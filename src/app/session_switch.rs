use super::notify_actions::ToastSeverity;
use super::{App, PlayerTab, QueueScope, SuspendedLocalSession};
use mbv_core::player::{PlayerEvent, PlayerProxy};
use mbv_core::ws::WsEvent;
use std::sync::mpsc;
use std::time::{Duration, Instant};

impl App {
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
        let initial_unified_state = remote.unified_queue_state();
        let has_initial_items = initial_unified_state
            .as_ref()
            .map_or(!initial_items.is_empty(), |state| !state.slots.is_empty());
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.config.lock().unwrap().always_play_next;
        // Cloned before `remote` is moved into `PlayerProxy::remote` below:
        // MPRIS (if this session has a live registration) must follow this
        // new ctrl-owning target too, or it stays wired to whatever owned
        // playback before the takeover -- see #175.
        let mpris_remote = remote.clone();

        if !self.player.is_remote() {
            self.player.stop();
            self.player.join_or_timeout(Duration::from_secs(5));
            let (_dummy_ws_tx, dummy_ws_rx) = mpsc::channel::<WsEvent>();
            let (_dummy_abs_tx, dummy_abs_rx) =
                mpsc::channel::<mbv_core::audiobookshelf_socket::SocketEvent>();
            let suspended = SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
                ws_rx: std::mem::replace(&mut self.ws_rx, dummy_ws_rx),
                ws_send_tx: self.ws_send_tx.take(),
                audiobookshelf_socket_rx: std::mem::replace(
                    &mut self.audiobookshelf_socket_rx,
                    dummy_abs_rx,
                ),
                audiobookshelf_socket_tx: self.audiobookshelf_socket_tx.take(),
                audiobookshelf_socket_generation: self.audiobookshelf_socket_generation.take(),
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

        self.remote_player_tab = Some(initial_unified_state.as_ref().map_or_else(
            || PlayerTab::from_emby_items(initial_items, initial_cursor),
            PlayerTab::from_unified_state,
        ));
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
        self.close_sidebar(super::SidebarId::Sessions);
        self.flash(
            format!("Connected directly to {}", sess.device_name),
            ToastSeverity::Success,
        );
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
        let initial_unified_state = remote.unified_queue_state();
        let has_initial_items = initial_unified_state
            .as_ref()
            .map_or(!initial_items.is_empty(), |state| !state.slots.is_empty());
        let initial_cursor = remote.status.lock().unwrap().current_idx;
        let always_play_next = self.config.lock().unwrap().always_play_next;
        // Cloned before `remote` is moved into `PlayerProxy::remote` below,
        // mirroring `switch_to_direct_remote`'s #175 MPRIS rebind.
        let mpris_remote = remote.clone();

        if !self.player.is_remote() {
            self.player.stop();
            self.player.join_or_timeout(Duration::from_secs(5));
            let (_dummy_ws_tx, dummy_ws_rx) = mpsc::channel::<WsEvent>();
            let (_dummy_abs_tx, dummy_abs_rx) =
                mpsc::channel::<mbv_core::audiobookshelf_socket::SocketEvent>();
            let suspended = SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
                ws_rx: std::mem::replace(&mut self.ws_rx, dummy_ws_rx),
                ws_send_tx: self.ws_send_tx.take(),
                audiobookshelf_socket_rx: std::mem::replace(
                    &mut self.audiobookshelf_socket_rx,
                    dummy_abs_rx,
                ),
                audiobookshelf_socket_tx: self.audiobookshelf_socket_tx.take(),
                audiobookshelf_socket_generation: self.audiobookshelf_socket_generation.take(),
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

        self.remote_player_tab = Some(initial_unified_state.as_ref().map_or_else(
            || PlayerTab::from_emby_items(initial_items, initial_cursor),
            PlayerTab::from_unified_state,
        ));
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
        self.flash(
            format!("Routed to {library_name} daemon"),
            ToastSeverity::Success,
        );
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
            self.audiobookshelf_socket_rx = suspended.audiobookshelf_socket_rx;
            self.audiobookshelf_socket_tx = suspended.audiobookshelf_socket_tx;
            self.audiobookshelf_socket_generation = suspended.audiobookshelf_socket_generation;
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
                    let initial_unified_state = remote.unified_queue_state();
                    let initial_cursor = remote.status.lock().unwrap().current_idx;
                    let remote_queue_source = remote.queue_source.lock().unwrap().clone();
                    let initial_tab = initial_unified_state.as_ref().map_or_else(
                        || PlayerTab::from_emby_items(initial_items, initial_cursor),
                        PlayerTab::from_unified_state,
                    );
                    let always_play_next = self.config.lock().unwrap().always_play_next;
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
                    reconnected_local_daemon = Some((initial_tab, remote_queue_source));
                }
                Err(_message) => {
                    // The generic message from try_daemon_route_connect
                    // claims "using local playback" but that is wrong here:
                    // there is no suspended local player and the Local
                    // daemon is unreachable, so local playback is not
                    // actually available. Strip any such claim from the
                    // route-failure message that was threaded through as
                    // `status` (the double-failure path from
                    // `apply_route_for_playback`), and always surface that
                    // the Local daemon is unavailable.
                    if status.contains("using local playback") {
                        status = status.replace("using local playback", "local daemon unavailable");
                    } else {
                        status = format!("{status}; local daemon unavailable");
                    }
                }
            }
        }
        if let Some((initial_tab, remote_queue_source)) = reconnected_local_daemon {
            // Mirror `App::new_remote`'s local-daemon baseline (`construct.rs`):
            // adopt the daemon's live queue into the single unified `player_tab`
            // -- no separate remote tab, no scope pill (#424).
            self.player_tab = initial_tab;
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
        self.flash(status, ToastSeverity::Warning);
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
    pub(super) fn apply_route_for_playback(&mut self, item: &mbv_core::api::EmbyItem) {
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
                            self.flash(message, ToastSeverity::Warning);
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
                match self.connect_direct_endpoint(&endpoint) {
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
        self.close_sidebar(super::SidebarId::Sessions);
        if let Some(error) = direct_upgrade_error {
            self.flash(
                format!("Direct mbv control failed: {error}; using attached session {name}"),
                ToastSeverity::Warning,
            );
        } else {
            self.flash(format!("Connected to {name}"), ToastSeverity::Success);
        }
        self.spawn_sessions_load();
    }
}
