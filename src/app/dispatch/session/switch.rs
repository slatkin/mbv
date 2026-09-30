use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, QueueScope, QueueView, SuspendedLocalSession};
use mbv_ctrl::player::PlayerEvent;
use mbv_player::PlayerProxy;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn replacement_view(snapshot: Option<&mbv_ctrl::UnifiedQueueStateData>) -> QueueView {
    snapshot.map_or_else(QueueView::default, QueueView::from_snapshot)
}

impl App {
    fn attach_remote_owner(
        &mut self,
        remote: mbv_remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
        current_is_home: bool,
    ) -> bool {
        let initial_unified_state = remote.unified_queue_state();
        let always_play_next = self.config.lock().unwrap().always_play_next;
        if endpoint.is_local() && (self.suspended_local.is_some() || current_is_home) {
            remote.disconnect();
            self.restore_local_mode("Local playback restored");
            return false;
        }
        self.player_endpoint = Some(endpoint.clone());
        if current_is_home {
            if !self.config.lock().unwrap().stay_alive {
                self.player.stop();
            }
            self.suspended_local = Some(SuspendedLocalSession {
                player: std::mem::replace(
                    &mut self.player,
                    PlayerProxy::from_remote(remote, always_play_next),
                ),
                player_rx: std::mem::replace(&mut self.player_rx, remote_rx),
            });
        } else {
            // #233: tear down the previous remote connection's socket
            // before dropping the old PlayerProxy, so its reader thread
            // observes the shutdown and exits instead of leaking.
            self.player.disconnect_remote();
            self.player = PlayerProxy::from_remote(remote, always_play_next);
            self.player_rx = remote_rx;
        }
        self.sync_subtitle_prefs_to_player();
        self.rebind_mpris_to_current_player();

        let daemon_view = replacement_view(initial_unified_state.as_ref());
        if endpoint.is_local() {
            self.adopt_local_daemon_queue(daemon_view);
        } else {
            self.remote_view = Some(daemon_view);
        }
        self.advance_queue_epoch();
        true
    }

    pub(in crate::app) fn switch_to_direct_remote(
        &mut self,
        sess: &mbv_emby::SessionInfo,
        remote: mbv_remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) {
        let current_is_home = self.home_is_local_daemon && self.is_local_daemon();
        self.stop_visualizer_capture();
        if !self.attach_remote_owner(remote, remote_rx, endpoint, current_is_home) {
            return;
        }
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.remote.direct_remote_connected = true;
        self.remote.direct_remote_label = {
            let name = sess.device_name.trim();
            (!name.is_empty()).then(|| name.to_string())
        };
        // The control socket replaces Session watch, but the device the user
        // connected to stays the Sessions-sidebar row. The Stay-alive process
        // (Local endpoint) is not a remote session and is never marked.
        self.remote.direct_remote_session_id = (!endpoint.is_local()).then(|| sess.id.clone());
        self.remote.session_miss_count = 0;
        self.remote.remote_pos_s = 0;
        self.remote.remote_pos_at = Instant::now();
        self.remote.remote_api_pos_advanced_at = Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now);
        self.remote.remote_seek_pending_until = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        self.remote.runtime_zero_since = None;
        self.next_up_item = None;
        self.display_peer_queue_on_connect();
        self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
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
    pub(in crate::app) fn switch_to_library_route(
        &mut self,
        library_name: &str,
        remote: mbv_remote_player::RemotePlayer,
        remote_rx: mpsc::Receiver<PlayerEvent>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) {
        // Attachment slots are mutually exclusive: a library-route switch can
        // be reached (via `apply_route_for_playback`) without going through
        // `connect_to_session`'s sever, so a cast attachment must be severed
        // here too. No-op when nothing is attached.
        self.cast_attachment = None;
        let current_is_home = self.home_is_local_daemon && self.is_local_daemon();
        self.stop_visualizer_capture();
        let previous_route = self.active_route.clone();
        if !self.attach_remote_owner(remote, remote_rx, endpoint, current_is_home) {
            return;
        }
        self.remote.direct_remote_connected = false;
        self.remote.direct_remote_session_id = None;
        self.active_route = Some(library_name.to_string());
        self.remote.remote_pos_s = 0;
        self.remote.remote_pos_at = Instant::now();
        self.remote.remote_api_pos_advanced_at = Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now);
        self.remote.remote_seek_pending_until = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        self.remote.runtime_zero_since = None;
        self.next_up_item = None;
        self.display_peer_queue_on_connect();
        tracing::info!(name: "library_route.playback_route.switched", target: "library_route", previous_route = ?previous_route, next_route = %library_name, "playback route switched");
        self.flash(
            format!("Routed to {library_name} daemon"),
            ToastSeverity::Success,
        );
    }

    /// Resolve this machine's local Player without changing the attachment.
    pub(in crate::app) fn prepare_local_player(
        &mut self,
    ) -> Result<Option<SuspendedLocalSession>, mbv_remote_player::RemotePlayerError> {
        if self.home_is_local_daemon && self.is_local_daemon() {
            return Ok(None);
        }
        if let Some(suspended) = self.suspended_local.take()
            && !suspended.player.is_remote_disconnected()
        {
            return Ok(Some(suspended));
        }
        #[cfg(not(test))]
        {
            let socket_path = crate::single_instance::socket_path();
            let lock_path = crate::single_instance::lock_path();
            match crate::single_instance::resolve(&socket_path, &lock_path)? {
                crate::single_instance::Resolution::Fresh(guard) => {
                    drop(guard);
                    crate::local_daemon::spawn_detached(&socket_path.to_string_lossy(), None)?;
                }
                crate::single_instance::Resolution::Attach => {}
                crate::single_instance::Resolution::Refuse => {
                    return Err(std::io::Error::other(
                        "the local Player owner is not accepting connections",
                    )
                    .into());
                }
            }
        }
        let (remote, player_rx) = Self::try_daemon_route_connect(
            &mbv_remote_player::DaemonEndpoint::Local,
            "local daemon",
        )?;
        let always_play_next = self.config.lock().unwrap().always_play_next;
        Ok(Some(SuspendedLocalSession {
            player: PlayerProxy::from_remote(remote, always_play_next),
            player_rx,
        }))
    }

    /// Install a prepared local session over the current attachment: swap
    /// the player and its channel plumbing back in and drop the remote
    /// endpoint baseline. Shared by the confirmed fall-through and ordinary
    /// restoration so no path can half-install a suspended local Player.
    /// Local daemon attach: one unified queue owned by the daemon
    /// (local-daemon-thin-client spec). Adopt the owner's Bound queue as the
    /// displayed Local queue, mirroring the App-construction attach path —
    /// parking it in `remote_view` leaves it unreachable, because the
    /// unified view never reads that tab.
    fn adopt_local_daemon_queue(&mut self, daemon_view: QueueView) {
        self.local_view = daemon_view;
        self.remote_view = None;
    }

    fn install_suspended_local(&mut self, suspended: SuspendedLocalSession) {
        self.player = suspended.player;
        self.player_rx = suspended.player_rx;
        self.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    }

    pub(in crate::app) fn rebind_mpris_to_current_player(&self) {
        if let Some(handle) = &self.mpris {
            let sender = self.player.transport_sender();
            mbv_desktop::mpris::rebind(
                handle,
                std::sync::Arc::clone(&self.player.status),
                move |transport| sender(transport),
                self.player.disconnected_flag(),
            );
        }
    }

    /// Clear attachment and route presentation after the local player is
    /// ready. Both ordinary restoration and confirmed fall-through use this
    /// tail so no path can leave the old owner presented or commandable.
    fn finish_local_mode(&mut self, status: String, reconnected_local_daemon: Option<QueueView>) {
        if let Some(local_view) = reconnected_local_daemon {
            self.local_view = local_view;
        }
        self.remote_view = None;
        self.set_queue_scope(QueueScope::Local);
        self.connected_session_id = None;
        self.connected_session_state = None;
        self.advance_queue_epoch();
        self.remote.direct_remote_connected = false;
        self.remote.direct_remote_label = None;
        self.remote.direct_remote_session_id = None;
        self.active_route = None;
        self.remote.session_miss_count = 0;
        self.remote.remote_pos_s = 0;
        self.next_up_item = None;
        self.rebind_mpris_to_current_player();
        self.flash(status, ToastSeverity::Warning);
    }

    /// Execute a confirmed local fall-through. Local preparation happens
    /// before the current owner is stopped or the attachment is changed.
    pub(in crate::app) fn play_pending_local_play(&mut self) {
        let Some(action) = self.queue_deferrals.take_confirmed_local_play() else {
            return;
        };
        let prepared = match self.prepare_local_player() {
            Ok(prepared) => prepared,
            Err(error) => {
                self.flash(
                    format!("Cannot prepare local playback: {error}"),
                    ToastSeverity::Warning,
                );
                return;
            }
        };
        let already_local = prepared.is_none();
        if !already_local {
            self.player.stop();
            self.player.disconnect_remote();
        }
        if self.connected_session_id.is_some() {
            self.playback_target().stop(self);
        }
        if let Some(suspended) = prepared {
            self.install_suspended_local(suspended);
            self.sync_subtitle_prefs_to_player();
        }
        self.finish_local_mode(
            if already_local {
                "Already local"
            } else {
                "Playing locally"
            }
            .into(),
            None,
        );
        self.execute_pending_queue_action(action);
    }

    fn strip_local_playback_claim(status: &str) -> String {
        // The generic message from try_daemon_route_connect claims "using local
        // playback" but that is wrong here: there is no suspended local player
        // and the Local daemon is unreachable, so local playback is not actually
        // available. Strip any such claim from the route-failure message that
        // was threaded through as `status` (the double-failure path from
        // `apply_route_for_playback`), and always surface that the Local daemon
        // is unavailable.
        if status.contains("using local playback") {
            status.replace("using local playback", "local daemon unavailable")
        } else {
            format!("{status}; local daemon unavailable")
        }
    }

    pub(in crate::app) fn restore_local_mode(&mut self, status: &str) {
        let previous_route = self.active_route.clone();
        tracing::info!(name: "library_route.playback_route.restoring_local", target: "library_route", previous_route = ?previous_route, reason = %status, "restoring local playback");
        if self.home_is_local_daemon && self.is_local_daemon() && self.suspended_local.is_none() {
            self.finish_local_mode(status.to_string(), None);
            return;
        }
        self.player.disconnect_remote();
        let mut status = status.to_string();
        // Populated only when the local-daemon reconnect branch below
        // succeeds, so the tail can restore `local_view` / queue source
        // from the reconnected route instead of the plain-local defaults.
        let mut reconnected_local_daemon = None;
        if let Some(suspended) = self
            .suspended_local
            .take()
            .filter(|suspended| !suspended.player.is_remote_disconnected())
        {
            self.install_suspended_local(suspended);
        } else if self.home_is_local_daemon {
            // This app's baseline was never a genuinely local in-process
            // player -- it was an `App::new_remote` thin client attached to
            // the local daemon (`home_is_local_daemon`), so nothing was ever
            // suspended above. Reconnect to the local daemon directly so
            // "restore local mode" actually lands back on this app's real
            // baseline instead of leaving the player disconnected.
            match Self::try_daemon_route_connect(
                &mbv_remote_player::DaemonEndpoint::Local,
                "local daemon",
            ) {
                Ok((remote, remote_rx)) => {
                    let initial_unified_state = remote.unified_queue_state();
                    let initial_tab = replacement_view(initial_unified_state.as_ref());
                    let always_play_next = self.config.lock().unwrap().always_play_next;
                    self.player = PlayerProxy::from_remote(remote, always_play_next);
                    self.player_rx = remote_rx;
                    self.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
                    self.sync_subtitle_prefs_to_player();
                    reconnected_local_daemon = Some(initial_tab);
                }
                Err(_message) => {
                    status = Self::strip_local_playback_claim(&status);
                }
            }
        }
        self.finish_local_mode(status, reconnected_local_daemon);
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
    pub(in crate::app) fn apply_route_for_playback(&mut self, item: &mbv_emby_model::EmbyItem) {
        let resolved = self.resolve_route_for_play(item);
        match (resolved, self.active_route.clone()) {
            (Some((name, _)), Some(current)) if name == current => {
                tracing::info!(name: "library_route.playback_route.already_active", target: "library_route", route = %name, item = %item.id, "playback route already active");
            }
            (Some((name, endpoint)), was_routed) => {
                if endpoint.is_local()
                    && (self.suspended_local.is_some()
                        || (self.home_is_local_daemon && self.is_local_daemon()))
                {
                    self.restore_local_mode("Local playback restored");
                    return;
                }
                match Self::try_daemon_route_connect(&endpoint, &name) {
                    Ok((remote, remote_rx)) => {
                        self.switch_to_library_route(&name, remote, remote_rx, &endpoint);
                    }
                    Err(error) => {
                        tracing::warn!(name: "library_route.connect.failed", target: "library_route", route = %name, endpoint = %endpoint, error = %error, "library route connection failed");
                        let warning = format!(
                            "\u{26a0} {name} route unreachable, using local playback (mbv.log)"
                        );
                        if was_routed.is_some() {
                            self.restore_local_mode(&warning);
                        } else {
                            self.flash(warning, ToastSeverity::Warning);
                        }
                    }
                }
            }
            (None, Some(current)) => {
                tracing::info!(name: "library_route.playback_route.unresolved", target: "library_route", current_route = %current, item = %item.id, "no route resolved while routed; restoring local");
                self.restore_local_mode("Local playback restored");
            }
            (None, None) => {
                tracing::info!(name: "library_route.playback_route.unresolved", target: "library_route", item = %item.id, "no route resolved while local; staying local");
            }
        }
    }

    pub(in crate::app) fn connect_to_session(&mut self, sess: &mbv_emby::SessionInfo) {
        // Connecting to a new target severs the current one (attachment
        // slots are mutually exclusive): tears down an active library
        // route, detaches any cast attachment, and clears a watched
        // session before this connect, rather than holding both. A no-op
        // when nothing is connected.
        self.sever_active_connection();
        let mut direct_upgrade_error = None;
        if self.player_owner_is_on_this_machine()
            && let Some(endpoint) = self.session_direct_endpoint(sess)
        {
            if endpoint.is_local()
                && (self.suspended_local.is_some()
                    || (self.home_is_local_daemon && self.is_local_daemon()))
            {
                self.restore_local_mode("Local playback restored");
                return;
            }
            match Self::connect_direct_endpoint(&endpoint) {
                Ok((remote, remote_rx)) => {
                    self.switch_to_direct_remote(sess, remote, remote_rx, &endpoint);
                    return;
                }
                Err(e) => {
                    tracing::warn!(name: "sessions.direct_daemon_upgrade.failed", target: "sessions", device = %sess.device_name, endpoint = %endpoint, error = %e, "direct daemon upgrade failed");
                    direct_upgrade_error = Some(e);
                }
            }
        }

        let id = sess.id.clone();
        let name = sess.device_name.clone();
        tracing::info!(name: "sessions.connect.started", target: "sessions", device = %name, position_seconds = sess.position_s, runtime_seconds = sess.runtime_s, "connecting to session");
        self.connected_session_id = Some(id);
        self.connected_session_state = Some(sess.clone());
        self.advance_queue_epoch();
        self.remote.session_miss_count = 0;
        self.remote.remote_pos_s = sess.position_s;
        self.remote.remote_pos_at = Instant::now();
        self.remote.remote_api_pos_advanced_at = Instant::now();
        self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
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
