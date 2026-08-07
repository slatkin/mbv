use super::{App, PlayerTab, QueueScope, SuspendedLocalSession};
use mbv_core::api::parse_mbv_direct_tcp_port;
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::ws::WsEvent;
use std::sync::mpsc;
use std::time::{Duration, Instant};

impl App {
    pub(super) fn session_direct_endpoint(
        &self,
        sess: &mbv_core::api::SessionInfo,
    ) -> Option<mbv_core::remote_player::DaemonEndpoint> {
        if !sess.client.eq_ignore_ascii_case("mbv") {
            return None;
        }
        if let Some(port) = parse_mbv_direct_tcp_port(&sess.supported_commands) {
            if let Ok(ip) = sess.host.parse::<std::net::Ipv4Addr>() {
                return Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
                    std::net::SocketAddr::from((ip, port)),
                ));
            }
            log::warn!(
                target: "sessions",
                "mbv session {:?} advertised direct tcp port {} but host {:?} was not an IPv4 address",
                sess.device_name,
                port,
                sess.host
            );
        }
        let client = self.client.lock().unwrap();
        sess.device_name
            .eq_ignore_ascii_case(&client.device_name)
            .then_some(mbv_core::remote_player::DaemonEndpoint::Local)
    }

    /// Blocking `GET /Sessions` (unfiltered), factored out only so tests
    /// can override it via `SESSIONS_LOAD_OVERRIDE` -- mirrors
    /// `connect_daemon_route_endpoint`'s `#[cfg(test)]` seam. Callers:
    /// `try_auto_reconnect`'s `DirectSession` case (#236) and the F2
    /// "Library Routes" device picker (`enter_device_stage`, #256) --
    /// library-route *resolution* itself no longer calls this (#256).
    pub(super) fn fetch_sessions_blocking(
        &self,
    ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        #[cfg(test)]
        if let Some(f) = *super::SESSIONS_LOAD_OVERRIDE.lock().unwrap() {
            return f(&self.client.lock().unwrap());
        }
        self.client.lock().unwrap().get_sessions_unfiltered()
    }

    fn connect_direct_endpoint(
        &self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        #[cfg(test)]
        if let Some(connect) = *super::DIRECT_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint, auth_token);
        }

        mbv_core::remote_player::RemotePlayer::connect_endpoint(endpoint, auth_token)
    }

    /// Lazy, on-demand connect to a daemon route endpoint (issue #222's
    /// lifecycle primitive). Unlike `connect_direct_endpoint` (Sessions-panel
    /// "Direct Remote" upgrade, keyed off a discovered `SessionInfo`), this
    /// targets a statically configured `DaemonEndpoint` with no session
    /// discovery involved -- the shape #223's per-library routing needs.
    ///
    /// Under multi-connection (v4), connecting does NOT evict other ctrl
    /// clients. Authority is determined by command flow, not connection
    /// lifecycle (ADR 0014 supersedes ADR 0003).
    ///
    /// `#[allow(dead_code)]`: this repo's convention (`mem:conventions`) is
    /// "fix all compile warnings -- delete unused code, never
    /// `#[allow(unused)]`" -- but this primitive is a deliberate exception,
    /// not a suppressed mistake: issue #222's brief requires it to ship with
    /// *zero* production call sites (the trigger is #223's job, see
    /// Architecture above), so a plain `cargo build --workspace` (which
    /// strips `#[cfg(test)]` code, its only current caller) would otherwise
    /// warn `associated function is never used`. Deleting the primitive to
    /// silence that would defeat the entire point of this plan -- shipping
    /// a complete, tested, reusable connect primitive ahead of the issue
    /// that wires it up. Remove this attribute in the same change that adds
    /// #223's first call site (`apply_route_for_playback` or equivalent).
    fn connect_daemon_route_endpoint(
        &self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        #[cfg(test)]
        if let Some(connect) = *super::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint, auth_token);
        }

        log::info!(
            target: "daemon_route",
            "connecting to daemon route endpoint {endpoint}; under multi-connection (v4) this does not evict other ctrl clients (see ADR 0014)"
        );
        mbv_core::remote_player::RemotePlayer::connect_endpoint(endpoint, auth_token)
    }

    /// Attempts a lazy connect to `endpoint` for the route named
    /// `route_label` (e.g. a library name from #239's `library_routes`, or a
    /// generic label for the wildcard "route everything" case). On success,
    /// returns `Ok` with the connected `RemotePlayer` and its event receiver
    /// for the caller to swap in (mirroring `switch_to_direct_remote`'s
    /// shape). On failure, per #222: falls back to (stays on) local
    /// playback and schedules no retry -- but this primitive does NOT flash
    /// the warning itself. It logs the raw connect error internally
    /// (`target: "daemon_route"`), then returns `Err(message)` where
    /// `message` is the fully-formatted, ready-to-display status-bar
    /// warning text. Flashing is left to the caller deliberately: #223's
    /// per-library swap function needs to choose *how* to fall back --
    /// `flash_status_high(message)` directly when it was already local, or
    /// threading `message` through a `restore_local_mode`-style teardown
    /// when swapping away from a previously active *different* route -- and
    /// having this primitive flash unconditionally would risk a second,
    /// conflicting flash on top of that teardown path's own flash. The
    /// caller is expected to try again only on its own next natural trigger
    /// (e.g. the next play/enqueue into this route), never from a
    /// background timer. See the same `#[allow(dead_code)]` rationale as
    /// `connect_daemon_route_endpoint` above -- remove both attributes
    /// together when #223 adds its first call site.
    pub(super) fn try_daemon_route_connect(
        &self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        route_label: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        log::info!(target: "daemon_route", "daemon route attempt start route={route_label:?} endpoint={endpoint}");
        let auth_token = self.client.lock().unwrap().token.clone();
        self.connect_daemon_route_endpoint(endpoint, &auth_token)
            .inspect(|_| {
                log::info!(target: "daemon_route", "daemon route attempt succeeded route={route_label:?} endpoint={endpoint}");
            })
            .map_err(|e| {
                log::warn!(
                    target: "daemon_route",
                    "daemon route connect failed for route={route_label:?} endpoint={endpoint}: {e}"
                );
                format!("\u{26a0} {route_label} route unreachable, using local playback (mbv.log)")
            })
    }

    /// Restores the remote connection active when mbv last exited (issue
    /// #236 -- #222's original "auto-reconnect" intent). Called once from
    /// `App::new` (never `App::new_remote`, whose `--connect-daemon`
    /// startup path is a separate, unaffected mechanism per ADR 0010). A
    /// no-op unless `auto_reconnect` is enabled and
    /// `load_last_remote_connection` has a record. One shot, no retry: a
    /// failed connect, a route no longer present in `library_routes`, or a
    /// device not found in the current session list all fall back to (stay
    /// on) local playback, exactly like #222's per-play lazy-connect
    /// fallback rule -- never a hard failure at startup.
    pub(super) fn try_auto_reconnect(&mut self) {
        if !self.client.lock().unwrap().config.auto_reconnect {
            log::info!(target: "auto_reconnect", "auto-reconnect disabled; staying local");
            return;
        }
        log::info!(target: "auto_reconnect", "auto-reconnect enabled; loading state");
        let last = match mbv_core::config::load_last_remote_connection() {
            Ok(Some(last)) => last,
            Ok(None) => {
                log::info!(target: "auto_reconnect", "state missing; staying local");
                return;
            }
            Err(e) => {
                log::warn!(target: "auto_reconnect", "state load failed; staying local: {e}");
                return;
            }
        };
        match last {
            mbv_core::config::LastRemoteConnection::LibraryRoute { library } => {
                log::info!(target: "auto_reconnect", "state loaded variant=library-route library={library:?}");
                let Some((name, endpoint)) = self.resolve_route_for_library(&library) else {
                    log::info!(
                        target: "auto_reconnect",
                        "persisted library route {library:?} no longer resolves; staying local"
                    );
                    return;
                };
                match self.try_daemon_route_connect(&endpoint, &name) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx, &endpoint)
                    }
                    Err(message) => self.flash_status_high(message),
                }
            }
            mbv_core::config::LastRemoteConnection::DirectSession { device_name } => {
                log::info!(target: "auto_reconnect", "state loaded variant=direct-session device={device_name:?}");
                let sessions = match self.fetch_sessions_blocking() {
                    Ok(sessions) => sessions,
                    Err(e) => {
                        log::warn!(target: "auto_reconnect", "failed to list sessions: {e}");
                        self.flash_status_high(format!(
                            "\u{26a0} Auto-reconnect couldn't list sessions ({e}), using local playback"
                        ));
                        return;
                    }
                };
                match sessions
                    .into_iter()
                    .find(|s| s.device_name.eq_ignore_ascii_case(&device_name))
                {
                    Some(sess) => {
                        log::info!(target: "auto_reconnect", "direct-session resolved device={device_name:?} session_id={:?}; connecting", sess.id);
                        self.connect_to_session(&sess);
                        if self.direct_remote_connected {
                            log::info!(target: "auto_reconnect", "direct-session connection succeeded device={device_name:?} outcome=direct-daemon-upgrade");
                        } else if self.connected_session_id.is_some() {
                            log::info!(target: "auto_reconnect", "direct-session connection initiated device={device_name:?} outcome=emby-session-control");
                        } else {
                            log::warn!(target: "auto_reconnect", "direct-session connection failed device={device_name:?}; staying local");
                        }
                    }
                    None => {
                        log::info!(
                            target: "auto_reconnect",
                            "device {device_name:?} not found in current sessions; staying local"
                        );
                        self.flash_status_high(format!(
                            "\u{26a0} {device_name} not found, using local playback"
                        ));
                    }
                }
            }
        }
    }

    pub(super) fn switch_to_direct_remote(
        &mut self,
        sess: &mbv_core::api::SessionInfo,
        remote: mbv_core::remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
    ) {
        self.stop_visualizer_worker();
        self.player_endpoint = Some(endpoint.clone());
        let initial_items = remote.items.lock().unwrap().clone();
        let has_initial_items = !initial_items.is_empty();
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.client.lock().unwrap().config.always_play_next;
        // Cloned before `remote` is moved into `PlayerProxy::remote` below:
        // MPRIS (if this session has a live registration) must follow this
        // new ctrl-owning target too, or it stays wired to whatever owned
        // playback before the takeover -- see #175.
        let mpris_remote = remote.clone();

        if !self.player.is_remote() {
            self.player.stop();
            self.player.join_or_timeout(Duration::from_secs(5));
            let (_dummy_ws_tx, dummy_ws_rx) = mpsc::channel::<WsEvent>();
            let suspended = SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
                ws_rx: std::mem::replace(&mut self.ws_rx, dummy_ws_rx),
                ws_send_tx: self.ws_send_tx.take(),
            };
            self.suspended_local = Some(suspended);
        } else {
            // #233: tear down the previous remote connection's socket
            // before dropping the old PlayerProxy, so its reader thread
            // observes the shutdown and exits instead of leaking.
            self.player.disconnect_remote();
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }
        debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
        self.sync_subtitle_prefs_to_player();

        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            crate::mpris::rebind(
                handle,
                mpris_remote.status.clone(),
                move |cmd| {
                    mpris_remote.send_command(cmd);
                },
                Some(disconnected),
            );
        }

        self.remote_player_tab = Some(PlayerTab::new(initial_items, initial_cursor));
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.retire_remote_tracking(true);
        self.direct_remote_connected = true;
        self.direct_remote_label = {
            let name = sess.device_name.trim();
            (!name.is_empty()).then(|| name.to_string())
        };
        self.session_miss_count = 0;
        self.remote_pos_s = 0;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now() - Duration::from_secs(60);
        self.remote_seek_pending_until = Instant::now() - Duration::from_secs(1);
        self.runtime_zero_since = None;
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        if has_initial_items {
            self.set_queue_scope(QueueScope::Remote);
        } else {
            self.set_queue_scope(QueueScope::Local);
        }
        self.show_sessions = false;
        self.flash_status(format!("Connected directly to {}", sess.device_name));
    }

    /// Sibling to `switch_to_direct_remote` for library-scoped daemon
    /// routing (#223): same suspend-local/connect-remote shape, but
    /// targets a device name resolved live from `library_routes` instead
    /// of a discovered `SessionInfo`, and tracks
    /// `active_route` instead of `connected_session_id`/
    /// `direct_remote_label` -- library routing and the Sessions-panel
    /// direct-remote flow are two independent ways to end up thin-client
    /// and must not be conflated in App state. This is a new sibling
    /// method, not a modification of `switch_to_direct_remote`.
    pub(super) fn switch_to_library_route(
        &mut self,
        library_name: &str,
        remote: mbv_core::remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
    ) {
        self.stop_visualizer_worker();
        self.player_endpoint = Some(endpoint.clone());
        let previous_route = self.active_route.clone();
        let initial_items = remote.items.lock().unwrap().clone();
        let has_initial_items = !initial_items.is_empty();
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.client.lock().unwrap().config.always_play_next;
        // Cloned before `remote` is moved into `PlayerProxy::remote` below,
        // mirroring `switch_to_direct_remote`'s #175 MPRIS rebind.
        let mpris_remote = remote.clone();

        if !self.player.is_remote() {
            self.player.stop();
            self.player.join_or_timeout(Duration::from_secs(5));
            let (_dummy_ws_tx, dummy_ws_rx) = mpsc::channel::<WsEvent>();
            let suspended = SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
                ws_rx: std::mem::replace(&mut self.ws_rx, dummy_ws_rx),
                ws_send_tx: self.ws_send_tx.take(),
            };
            self.suspended_local = Some(suspended);
        } else {
            // #233: tear down the previous remote connection's socket
            // before dropping the old PlayerProxy, so its reader thread
            // observes the shutdown and exits instead of leaking.
            self.player.disconnect_remote();
            self.player = PlayerProxy::remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }
        debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
        self.sync_subtitle_prefs_to_player();

        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            crate::mpris::rebind(
                handle,
                mpris_remote.status.clone(),
                move |cmd| {
                    mpris_remote.send_command(cmd);
                },
                Some(disconnected),
            );
        }

        self.remote_player_tab = Some(PlayerTab::new(initial_items, initial_cursor));
        self.direct_remote_connected = false;
        self.active_route = Some(library_name.to_string());
        self.remote_pos_s = 0;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now() - Duration::from_secs(60);
        self.remote_seek_pending_until = Instant::now() - Duration::from_secs(1);
        self.runtime_zero_since = None;
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        if has_initial_items {
            self.set_queue_scope(QueueScope::Remote);
        } else {
            self.set_queue_scope(QueueScope::Local);
        }
        log::info!(
            target: "library_route",
            "switched playback route previous={previous_route:?} next={library_name:?}"
        );
        self.flash_status(format!("Routed to {library_name} daemon"));
    }

    pub(super) fn restore_local_mode(&mut self, status: &str) {
        let previous_route = self.active_route.clone();
        log::info!(target: "library_route", "restoring local playback previous_route={previous_route:?} reason={status:?}");
        if !self.player.is_remote() {
            self.player.stop();
        }
        self.player.join();
        // `join()` is a documented no-op for a remote player (it doesn't tear
        // down the control socket), so without this the old remote's reader
        // thread would leak here exactly as it did at the two already-fixed
        // remote-to-remote swap sites (#233). No-op if `self.player` is
        // already local.
        self.player.disconnect_remote();
        let mut status = status.to_string();
        // Populated only when the local-daemon reconnect branch below
        // succeeds, so the tail can restore `player_tab` / queue source
        // from the reconnected route instead of the plain-local defaults.
        let mut reconnected_local_daemon = None;
        if let Some(suspended) = self.suspended_local.take() {
            self.player = suspended.player;
            self.player_rx = suspended.player_rx;
            self.ws_rx = suspended.ws_rx;
            self.ws_send_tx = suspended.ws_send_tx;
            self.player_endpoint = None;
            debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
            // Mirror `switch_to_direct_remote`'s rebind (#175): the suspended
            // local `Player` is being restored as the ctrl-owning target
            // again, so MPRIS (if registered) must follow it back rather
            // than staying wired to the just-abandoned remote session.
            if let Some(handle) = &self.mpris {
                let sender = self.player.command_sender();
                crate::mpris::rebind(
                    handle,
                    self.player.status.clone(),
                    move |cmd| sender(cmd),
                    self.player.disconnected_flag(),
                );
            }
        } else if self.home_is_local_daemon {
            // This app's baseline was never a genuinely local in-process
            // player -- it was an `App::new_remote` thin client attached to
            // the local daemon (`home_is_local_daemon`), so nothing was ever
            // suspended above. Reconnect to the local daemon directly so
            // "restore local mode" actually lands back on this app's real
            // baseline instead of leaving the player disconnected.
            match self.try_daemon_route_connect(
                &mbv_core::remote_player::DaemonEndpoint::Local,
                "local daemon",
            ) {
                Ok((remote, remote_rx)) => {
                    let initial_items = remote.items.lock().unwrap().clone();
                    let initial_cursor = remote.status.lock().unwrap().current_idx;
                    let remote_queue_source = remote.queue_source.lock().unwrap().clone();
                    let always_play_next = self.client.lock().unwrap().config.always_play_next;
                    // Cloned before `remote` is moved into `PlayerProxy::remote`
                    // below, mirroring `switch_to_library_route`'s #175 MPRIS
                    // rebind.
                    let mpris_remote = remote.clone();
                    self.player = PlayerProxy::remote(remote, always_play_next);
                    self.player_rx = remote_rx;
                    if let Some(handle) = &self.mpris {
                        let disconnected = mpris_remote.disconnected_flag();
                        crate::mpris::rebind(
                            handle,
                            mpris_remote.status.clone(),
                            move |cmd| {
                                mpris_remote.send_command(cmd);
                            },
                            Some(disconnected),
                        );
                    }
                    self.player_endpoint = Some(mbv_core::remote_player::DaemonEndpoint::Local);
                    debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
                    self.sync_subtitle_prefs_to_player();
                    reconnected_local_daemon =
                        Some((initial_items, initial_cursor, remote_queue_source));
                }
                Err(message) => {
                    status = format!("{status}; {message}");
                }
            }
        }
        if let Some((initial_items, initial_cursor, remote_queue_source)) = reconnected_local_daemon
        {
            // Mirror `App::new_remote`'s local-daemon baseline (`construct.rs`):
            // adopt the daemon's live queue into the single unified `player_tab`
            // -- no separate remote tab, no scope pill (#424).
            self.player_tab = PlayerTab::new(initial_items, initial_cursor);
            self.queue_source = remote_queue_source;
        }
        self.remote_player_tab = None;
        self.set_queue_scope(QueueScope::Local);
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.retire_remote_tracking(true);
        self.direct_remote_connected = false;
        self.direct_remote_label = None;
        self.active_route = None;
        self.session_miss_count = 0;
        self.remote_pos_s = 0;
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        self.flash_status_high(status);
    }

    /// Applies the route resolved by `resolve_route_for_play` before a
    /// queue replace (#223): swaps to the routed daemon, restores local,
    /// or leaves the current target alone if it already matches.
    /// Connection failure falls back to local playback -- never a hard
    /// error -- per #222's fallback rule, via `try_daemon_route_connect`
    /// (#222's plan, Task 1), which already logs the raw failure and
    /// returns a ready-to-display warning string as its `Err` payload;
    /// this method decides *where* to surface that string -- a direct
    /// flash when we were already local, or threaded through
    /// `restore_local_mode` when we were already on a *different* route
    /// and must actually swap the player back to local, not just show a
    /// warning while silently staying connected to the old route.
    pub(super) fn apply_route_for_playback(&mut self, item: &mbv_core::api::MediaItem) {
        let resolved = self.resolve_route_for_play(item);
        match (resolved, self.active_route.clone()) {
            (Some((name, _)), Some(current)) if name == current => {
                log::info!(target: "library_route", "already-active route no-op route={name:?} item_id={:?}", item.id);
            }
            (Some((name, endpoint)), was_routed) => {
                match self.try_daemon_route_connect(&endpoint, &name) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx, &endpoint);
                    }
                    Err(message) => {
                        log::warn!(
                            target: "library_route",
                            "connect to library route {name:?} endpoint {endpoint} failed: {message}"
                        );
                        if was_routed.is_some() {
                            self.restore_local_mode(&message);
                        } else {
                            self.flash_status_high(message);
                        }
                    }
                }
            }
            (None, Some(current)) => {
                log::info!(target: "library_route", "no route resolved while routed current={current:?}; restoring local item_id={:?}", item.id);
                self.restore_local_mode("Local playback restored");
            }
            (None, None) => {
                log::info!(target: "library_route", "no route resolved while local item_id={:?}; staying local", item.id)
            }
        }
    }

    pub(super) fn connect_to_session(&mut self, sess: &mbv_core::api::SessionInfo) {
        // Tear down an active library route (#223) before a Sessions-panel
        // connect, rather than simply clearing `active_route` inline
        // further down: `restore_local_mode` runs the real teardown
        // (restores the suspended local `Player`, clears `active_route`,
        // MPRIS rebind) so the Sessions-panel path always starts from a
        // clean slate, regardless of which internal branch this function or
        // `switch_to_direct_remote` takes next. A no-op when no library
        // route is active.
        if self.active_route.is_some() {
            self.restore_local_mode("Local playback restored before connecting to session");
        }
        let mut direct_upgrade_error = None;
        // `player.is_remote()` alone can't gate this: a stay-alive thin
        // client attached to its own local daemon (is_local_daemon()) is
        // already `is_remote() == true` despite never having left home
        // base, which used to skip the direct-upgrade attempt entirely and
        // strand the connection on the plain `AttachedSession` path with no
        // queue management. Only a genuinely different remote target
        // should skip this.
        if self.player_owner_is_on_this_machine() {
            if let Some(endpoint) = self.session_direct_endpoint(sess) {
                let auth_token = self.client.lock().unwrap().token.clone();
                match self.connect_direct_endpoint(&endpoint, &auth_token) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_direct_remote(sess, remote, remote_rx, &endpoint);
                        return;
                    }
                    Err(e) => {
                        log::warn!(
                            target: "sessions",
                            "direct daemon upgrade failed for device={:?} endpoint={endpoint}: {}",
                            sess.device_name,
                            e
                        );
                        direct_upgrade_error = Some(e);
                    }
                }
            }
        }

        let id = sess.id.clone();
        let name = sess.device_name.clone();
        log::info!(
            target: "sessions",
            "connect: device={name:?} pos={}s runtime={}s",
            sess.position_s,
            sess.runtime_s
        );
        self.connected_session_id = Some(id);
        self.connected_session_state = Some(sess.clone());
        self.retire_remote_tracking(true);
        self.session_miss_count = 0;
        self.remote_pos_s = sess.position_s;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now();
        self.show_sessions = false;
        if let Some(error) = direct_upgrade_error {
            self.flash_status_high(format!(
                "Direct mbv control failed: {error}; using attached session {name}"
            ));
        } else {
            self.flash_status(format!("Connected to {name}"));
        }
        self.spawn_sessions_load();
    }
}
