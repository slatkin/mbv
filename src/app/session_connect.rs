use super::notify_actions::ToastSeverity;
use super::{App, PlayerTab, QueueScope};
use mbv_core::api::parse_mbv_direct_tcp_port;
use mbv_core::player::{PlayerEvent, PlayerProxy};
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
        let client = self.emby_client()?;
        let client = client.lock().unwrap();
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
            let Some(client) = self.emby_client() else {
                return Err("Emby is unavailable".into());
            };
            return f(&client.lock().unwrap());
        }
        let Some(client) = self.emby_client() else {
            return Err("Emby is unavailable".into());
        };
        let result = client.lock().unwrap().get_sessions_unfiltered();
        result
    }

    pub(super) fn connect_direct_endpoint(
        &self,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        #[cfg(test)]
        if let Some(connect) = *super::DIRECT_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint);
        }

        mbv_core::remote_player::RemotePlayer::connect_endpoint(endpoint)
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
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        #[cfg(test)]
        if let Some(connect) = *super::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() {
            return connect(endpoint);
        }

        log::info!(
            target: "daemon_route",
            "connecting to daemon route endpoint {endpoint}; under multi-connection (v4) this does not evict other ctrl clients (see ADR 0014)"
        );
        mbv_core::remote_player::RemotePlayer::connect_endpoint(endpoint)
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
    /// `flash(message, ToastSeverity::Warning)` directly when it was already local, or
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
        self.connect_daemon_route_endpoint(endpoint)
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

    /// Reattach to the same daemon endpoint after an unannounced drop when
    /// `auto_reconnect` is enabled. Runs before the local-restore fallback so
    /// a restarted daemon lands the TUI straight back on its canonical queue
    /// instead of dropping to local playback for the rest of the session.
    ///
    /// Returns `true` when the player has been swapped to a live reattach and
    /// the caller must skip `restore_local_mode`. Local daemons are excluded:
    /// those already have the modal / `home_is_local_daemon` reconnect paths.
    pub(super) fn try_reattach_remote_daemon(&mut self) -> bool {
        let Some(endpoint) = self.player_endpoint.clone() else {
            return false;
        };
        if matches!(endpoint, mbv_core::remote_player::DaemonEndpoint::Local) {
            return false;
        }
        if !self.config.lock().unwrap().auto_reconnect {
            return false;
        }
        log::info!(target: "auto_reconnect", "reconnect enabled; reattaching to {endpoint}");
        // The daemon may still be coming back up, so retry a few times with
        // backoff before falling through to the local-restore path. Bounded
        // and short so an unreachable daemon cannot wedge the UI.
        let mut backoff = Duration::from_millis(300);
        for attempt in 0..3 {
            match self.connect_daemon_route_endpoint(&endpoint) {
                Ok((remote, remote_rx)) => {
                    self.attach_reattached_daemon(remote, remote_rx, &endpoint, attempt);
                    return true;
                }
                Err(e) => {
                    log::warn!(
                        target: "auto_reconnect",
                        "reattach attempt {attempt} to {endpoint} failed: {e}"
                    );
                    std::thread::sleep(backoff);
                    backoff *= 2;
                }
            }
        }
        false
    }

    fn attach_reattached_daemon(
        &mut self,
        remote: mbv_core::remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_core::remote_player::DaemonEndpoint,
        attempt: usize,
    ) {
        let initial_items = remote.items.lock().unwrap().clone();
        let initial_unified_state = remote.unified_queue_state();
        let has_initial_items = initial_unified_state
            .as_ref()
            .map_or(!initial_items.is_empty(), |state| !state.slots.is_empty());
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.config.lock().unwrap().always_play_next;
        let mpris_remote = remote.clone();
        // #233: tear down the dead connection before replacing it so its
        // reader thread observes the shutdown and exits instead of leaking.
        self.player.disconnect_remote();
        self.player = PlayerProxy::remote(remote, always_play_next);
        self.player_rx = remote_rx;
        self.player_endpoint = Some(endpoint.clone());
        debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
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
        self.remote_player_tab = Some(initial_unified_state.as_ref().map_or_else(
            || PlayerTab::from_emby_items(initial_items, initial_cursor),
            PlayerTab::from_unified_state,
        ));
        self.direct_remote_connected = true;
        self.retire_remote_tracking(true);
        self.session_miss_count = 0;
        self.remote_pos_s = 0;
        self.remote_pos_at = Instant::now();
        self.remote_api_pos_advanced_at = Instant::now() - Duration::from_secs(60);
        self.remote_seek_pending_until = Instant::now() - Duration::from_secs(1);
        self.runtime_zero_since = None;
        self.next_up_item = None;
        if has_initial_items {
            self.set_queue_scope(QueueScope::Remote);
        } else {
            self.set_queue_scope(QueueScope::Local);
        }
        self.sync_subtitle_prefs_to_player();
        self.flash(
            format!("Reconnected to daemon (attempt {})", attempt + 1),
            ToastSeverity::Success,
        );
    }

    /// Restores the remote connection active when mbv last exited (issue
    /// #236 -- #222's original "auto-reconnect" intent). Called once per
    /// launch: synchronously from `App::new_remote`'s local-daemon-attach
    /// path (construct.rs) when the Emby client is already available at
    /// construction, or from `apply_emby_completion`
    /// (app_emby_service_completion.rs) once the async Emby startup used by
    /// `App::new_independent` completes. A genuinely remote
    /// `--connect-daemon` launch is a separate, unaffected mechanism per
    /// ADR 0010. A no-op unless `auto_reconnect` is enabled and
    /// `load_last_remote_connection` has a record. One shot, no retry: a
    /// failed connect, a route no longer present in `library_routes`, or a
    /// device not found in the current session list all fall back to (stay
    /// on) local playback, exactly like #222's per-play lazy-connect
    /// fallback rule -- never a hard failure at startup.
    pub(super) fn try_auto_reconnect(&mut self) {
        if !self.config.lock().unwrap().auto_reconnect {
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
                    Err(message) => self.flash(message, ToastSeverity::Warning),
                }
            }
            mbv_core::config::LastRemoteConnection::DirectSession { device_name } => {
                log::info!(target: "auto_reconnect", "state loaded variant=direct-session device={device_name:?}");
                let sessions = match self.fetch_sessions_blocking() {
                    Ok(sessions) => sessions,
                    Err(e) => {
                        log::warn!(target: "auto_reconnect", "failed to list sessions: {e}");
                        self.flash(format!(
                            "\u{26a0} Auto-reconnect couldn't list sessions ({e}), using local playback"
                        ), ToastSeverity::Warning);
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
                        self.flash(
                            format!("\u{26a0} {device_name} not found, using local playback"),
                            ToastSeverity::Warning,
                        );
                    }
                }
            }
        }
    }
}
