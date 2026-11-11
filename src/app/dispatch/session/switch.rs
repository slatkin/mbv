use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, QueueView, SuspendedLocalSession};
use mbv_ctrl::player::PlayerEvent;
use mbv_player::PlayerProxy;
use mbv_ui_msg::{Key, KeyEvent};
use std::sync::mpsc;

fn replacement_view(snapshot: Option<&mbv_ctrl::UnifiedQueueStateData>) -> QueueView {
    snapshot.map_or_else(QueueView::default, QueueView::from_snapshot)
}

/// The Session's display name: the trimmed device name, or the device id
/// when the name is blank.
fn session_display_name(sess: &mbv_emby::SessionInfo) -> String {
    let name = sess.device_name.trim();
    if name.is_empty() {
        sess.device_id.clone()
    } else {
        name.to_string()
    }
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
        // The incoming owner's output is unknown until its cold-seed
        // `PlaybackOutput` event arrives (cast-owner-output design D13 reset
        // rule); the replaced owner's state must not leak into it.
        self.playback_output = None;
        if current_is_home {
            // The suspended home link stays playing while suspended: it is
            // drained every tick (`drain_suspended_home_events`), so route
            // switching must not stop it, whatever Stay-alive is set to
            // (stay-alive-is-lifetime-only, design D1).
            let remote_rx = self.waking_player_rx(remote_rx);
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
            self.player_rx = self.waking_player_rx(remote_rx);
        }
        self.sync_subtitle_prefs_to_player();
        self.rebind_mpris_to_current_player();

        let daemon_view = replacement_view(initial_unified_state.as_ref());
        let shown_before = self.playing_queue_link();
        if endpoint.is_local() {
            self.adopt_local_daemon_queue(daemon_view);
        } else {
            self.remote_view = Some(daemon_view);
        }
        self.reanchor_queue_cursor_after_link_change(shown_before);
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
        let current_is_home = self.is_local_daemon();
        self.stop_visualizer_capture();
        if !self.attach_remote_owner(remote, remote_rx, endpoint, current_is_home) {
            return;
        }
        self.direct_remote_connected = true;
        // A new restore-able target is held again: a prior explicit leave no
        // longer applies (the `left_remote_this_run` window to the next one).
        self.left_remote_this_run = false;
        self.direct_remote_label = {
            let name = sess.device_name.trim();
            (!name.is_empty()).then(|| name.to_string())
        };
        // The control socket replaces the device's Session control, but the
        // device the user connected to stays the Sessions-sidebar row. The
        // Stay-alive process (Local endpoint) is not a remote session and is
        // never marked.
        self.direct_remote_session_id = (!endpoint.is_local()).then(|| sess.id.clone());
        self.next_up_item = None;
        self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
        self.flash(
            format!("Connected directly to {}", sess.device_name),
            ToastSeverity::Success,
        );
    }

    /// Sibling to `switch_to_direct_remote` for library-scoped daemon
    /// routing (#223): same suspend-local/connect-remote shape, but
    /// targets a device name resolved live from `library_routes` instead
    /// of a discovered `SessionInfo`, and tracks `active_route` instead of
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
        // `connect_to_session`'s sever. The switch stops no playback, exactly
        // as a session connect does not (session-switch playback-continuity
        // decision): the outgoing owner's remote output keeps playing
        // whatever it plays, and `attach_remote_owner` suspends or
        // disconnects the outgoing link itself.
        let current_is_home = self.is_local_daemon();
        self.stop_visualizer_capture();
        let previous_route = self.active_route.clone();
        if !self.attach_remote_owner(remote, remote_rx, endpoint, current_is_home) {
            return;
        }
        self.direct_remote_connected = false;
        self.direct_remote_session_id = None;
        self.active_route = Some(library_name.to_string());
        // A new restore-able target is held again: a prior explicit leave no
        // longer applies (the `left_remote_this_run` window to the next one).
        self.left_remote_this_run = false;
        self.next_up_item = None;
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
        if self.is_local_daemon() {
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
            let lock_path = mbv_config::owner_lock_path();
            match crate::single_instance::resolve(&socket_path, &lock_path)? {
                crate::single_instance::Resolution::Fresh(guard) => {
                    drop(guard);
                    self.start_local_daemon()?;
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
            "owner process",
        )?;
        let always_play_next = self.config.lock().unwrap().always_play_next;
        Ok(Some(SuspendedLocalSession {
            player: PlayerProxy::from_remote(remote, always_play_next),
            player_rx: self.waking_player_rx(player_rx),
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
        // The previously attached owner's output does not carry over
        // (cast-owner-output design D13 reset rule).
        self.playback_output = None;
        self.player = suspended.player;
        self.player_rx = suspended.player_rx;
        self.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);
    }

    pub(in crate::app) fn rebind_mpris_to_current_player(&self) {
        if let Some(handle) = &self.mpris {
            let status_player = self.player.clone();
            let send_player = self.player.clone();
            let disconnect_player = self.player.clone();
            mbv_desktop::mpris::rebind(
                handle,
                move || status_player.status_snapshot(),
                move |transport| send_player.send_transport(transport),
                Some(move || disconnect_player.is_remote_disconnected()),
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
        let shown_before = self.playing_queue_link();
        self.remote_view = None;
        self.reanchor_queue_cursor_after_link_change(shown_before);
        self.advance_queue_epoch();
        self.direct_remote_connected = false;
        self.direct_remote_label = None;
        self.direct_remote_session_id = None;
        self.active_route = None;
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
        // The user explicitly confirmed playing locally, leaving whatever
        // restore-able remote target this Client held: teardown may then
        // clear the saved auto-reconnect record (see `left_remote_this_run`).
        if self.direct_remote_connected || self.active_route.is_some() {
            self.left_remote_this_run = true;
        }
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
        // An accepted fall-through ends the owner's Emby Session output before
        // local playback starts: `Stop`, then `SelectPlaybackOutput(Local)`
        // (emby-session-owner-output design D12).
        if self.output_is_emby_session() {
            self.hand_remote_output_back_to_local();
        }
        if !already_local {
            self.player.stop();
            self.player.disconnect_remote();
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
        if self.is_local_daemon() && self.suspended_local.is_none() {
            self.finish_local_mode(status.to_string(), None);
            return;
        }
        self.player.disconnect_remote();
        // The disconnected owner's output is no longer the active one; a
        // reconnect or restored suspension re-seeds it (cast-owner-output
        // design D13 reset rule).
        self.playback_output = None;
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
        } else {
            // This app's baseline is a connection to the local daemon, and
            // nothing was suspended above. Reconnect to the local daemon
            // directly so "restore local mode" actually lands back on this
            // app's real baseline instead of leaving the player
            // disconnected.
            match Self::try_daemon_route_connect(
                &mbv_remote_player::DaemonEndpoint::Local,
                "owner process",
            ) {
                Ok((remote, remote_rx)) => {
                    let initial_unified_state = remote.unified_queue_state();
                    let initial_tab = replacement_view(initial_unified_state.as_ref());
                    let always_play_next = self.config.lock().unwrap().always_play_next;
                    self.player = PlayerProxy::from_remote(remote, always_play_next);
                    self.player_rx = self.waking_player_rx(remote_rx);
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
        match self.resolve_route_for_play(item) {
            Some((name, endpoint)) => self.apply_resolved_playback_route(item, &name, &endpoint),
            None => self.clear_unresolved_playback_route(item),
        }
    }

    /// Applies a resolved playback route: keep an already-active one, otherwise
    /// connect it.
    fn apply_resolved_playback_route(
        &mut self,
        item: &mbv_emby_model::EmbyItem,
        name: &str,
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) {
        if self.active_route.as_deref() == Some(name) {
            tracing::info!(name: "library_route.playback_route.already_active", target: "library_route", route = %name, item = %item.id, "playback route already active");
            return;
        }
        let was_routed = self.active_route.is_some();
        self.connect_playback_route(name, endpoint, was_routed);
    }

    /// Connects the resolved library route, falling back to local playback when
    /// it is local or unreachable.
    fn connect_playback_route(
        &mut self,
        name: &str,
        endpoint: &mbv_remote_player::DaemonEndpoint,
        was_routed: bool,
    ) {
        if endpoint.is_local() && (self.suspended_local.is_some() || self.is_local_daemon()) {
            self.restore_local_mode("Local playback restored");
            return;
        }
        match Self::try_daemon_route_connect(endpoint, name) {
            Ok((remote, remote_rx)) => {
                self.switch_to_library_route(name, remote, remote_rx, endpoint);
            }
            Err(error) => {
                tracing::warn!(name: "library_route.connect.failed", target: "library_route", route = %name, endpoint = %endpoint, error = %error, "library route connection failed");
                let warning =
                    format!("\u{26a0} {name} route unreachable, using local playback (mbv.log)");
                if was_routed {
                    self.restore_local_mode(&warning);
                } else {
                    self.flash(warning, ToastSeverity::Warning);
                }
            }
        }
    }

    /// No route resolved: restore local playback when one was active, otherwise
    /// stay local.
    fn clear_unresolved_playback_route(&mut self, item: &mbv_emby_model::EmbyItem) {
        match self.active_route.clone() {
            Some(current) => {
                tracing::info!(name: "library_route.playback_route.unresolved", target: "library_route", current_route = %current, item = %item.id, "no route resolved while routed; restoring local");
                self.restore_local_mode("Local playback restored");
            }
            None => {
                tracing::info!(name: "library_route.playback_route.unresolved", target: "library_route", item = %item.id, "no route resolved while local; staying local");
            }
        }
    }

    /// Attempt the Direct remote control path for `sess`, when it advertises
    /// an endpoint. `Ok(true)` means the client now targets that endpoint (or
    /// restored local playback), `Ok(false)` means the Session has no direct
    /// endpoint, and `Err` means the endpoint was advertised but the connect
    /// failed. The caller severs the previous target first, because the direct
    /// path can replace a Cast output or a library route.
    ///
    /// The current attachment's locality does not gate this: a Client already
    /// attached to a remote owner (an mbv peer or `mbvd`) switches to another
    /// Session's direct endpoint by severing that link first, exactly as it
    /// does from the Local owner. Only a Session with no direct endpoint falls
    /// through to the owner-output path.
    fn connect_direct_session(
        &mut self,
        sess: &mbv_emby::SessionInfo,
    ) -> Result<bool, mbv_remote_player::RemotePlayerError> {
        let Some(endpoint) = self.session_direct_endpoint(sess) else {
            return Ok(false);
        };
        if endpoint.is_local() && (self.suspended_local.is_some() || self.is_local_daemon()) {
            self.restore_local_mode("Local playback restored");
            return Ok(true);
        }
        match Self::connect_direct_endpoint(&endpoint) {
            Ok((remote, remote_rx)) => {
                self.switch_to_direct_remote(sess, remote, remote_rx, &endpoint);
                Ok(true)
            }
            Err(e) => {
                tracing::warn!(name: "sessions.direct_daemon_upgrade.failed", target: "sessions", device = %sess.device_name, endpoint = %endpoint, error = %e, "direct daemon upgrade failed");
                Err(e)
            }
        }
    }

    pub(in crate::app) fn connect_to_session(&mut self, sess: &mbv_emby::SessionInfo) {
        // Connecting to a new target severs the current one (targets are
        // mutually exclusive): tears down an active library route before this
        // connect rather than holding both. A no-op when nothing is
        // connected. The sever stops no playback: the prior target's owner
        // keeps playing whatever it plays.
        self.sever_connections_without_output_change();
        match self.connect_direct_session(sess) {
            Ok(true) => {}
            Ok(false) => {
                tracing::info!(name: "sessions.connect.no_direct_endpoint", target: "sessions", device = %sess.device_name, "session offers no direct endpoint; staying local");
            }
            Err(error) => self.flash(
                format!("Direct mbv control failed: {error}"),
                ToastSeverity::Warning,
            ),
        }
    }

    /// Activate an Emby Session row from the F3 panel (emby-session-owner-output
    /// design D12). A Session that advertises a direct endpoint keeps the
    /// Direct remote control path; when that connect fails the Client reports
    /// the failure and, if the owner advertises `emby-session-output`, selects
    /// the Session as the owner's playback output instead. A Session without a
    /// direct endpoint is selected as the output only when the owner advertises
    /// that capability; otherwise the Client warns and does nothing. The
    /// existing connection is severed without handing a Cast output back to
    /// Local, because the owner performs the Cast-to-Session hand-off itself
    /// (spec scenario "An Emby Session is selected while a cast target is
    /// attached").
    ///
    /// A switch never stops the prior playback as a side effect: severing
    /// hands no output back to Local, and when the owner could move the
    /// active slot to the Session instead, the hand-off fork asks first
    /// (user decision: switching sessions must present the hand-off option;
    /// without it the prior playback continues).
    pub(in crate::app) fn activate_session_target(&mut self, sess: &mbv_emby::SessionInfo) {
        let has_direct_endpoint = self.session_direct_endpoint(sess).is_some();
        if has_direct_endpoint && self.hand_off_fork_applies(sess) {
            self.ask_hand_off_playback_to_session(sess);
            return;
        }
        self.activate_session_target_without_hand_off(sess);
    }

    /// Whether activating `sess` can offer the hand-off fork: this Client is
    /// attached to the local Owner, the Owner advertises
    /// `emby-session-output`, and a slot is active on its current output —
    /// selecting `sess` as the output then moves that slot to the Session
    /// (spec "Selecting an output hands off the active slot"), while a direct
    /// connect instead thin-clients the Session's own Owner. The Session's
    /// device already being the output skips the fork: that selection is the
    /// Owner's no-op hand-off. The decision reads the connection's retained
    /// output state, the record the handshake fills at the cold seed
    /// (cast-owner-output 8.2 fix).
    fn hand_off_fork_applies(&self, sess: &mbv_emby::SessionInfo) -> bool {
        if !self.is_local_daemon()
            || !self.player.remote().supports_emby_session_output()
            || !self.player.status_snapshot().active
        {
            return false;
        }
        self.player.remote().playback_output_state().is_none_or(|output| {
            !matches!(
                output,
                mbv_ctrl::OutputState::EmbySession { device_id, .. } if *device_id == sess.device_id
            )
        })
    }

    /// Raise the shared hand-off fork: Enter selects the Session as the
    /// local Owner's playback output (the Owner performs the hand-off), Esc
    /// switches to the Session's direct endpoint and leaves the prior
    /// playback running. The Session resolves against the current snapshot
    /// at answer time.
    fn ask_hand_off_playback_to_session(&mut self, sess: &mbv_emby::SessionInfo) {
        self.ask_confirm(mbv_ui_model::confirm::ConfirmModal::two_button(
            format!("Hand off playback to \"{}\"?", session_display_name(sess)),
            "Hand off",
            "Just switch",
            mbv_ui_model::confirm::ConfirmAction::HandOffPlaybackToSession {
                device_id: sess.device_id.clone(),
                name: session_display_name(sess),
            },
        ));
    }

    /// The confirm fork's answer: Enter hands the active slot to the
    /// Session as the local Owner's output; Esc runs the plain switch, whose
    /// sever stops nothing and whose direct connect leaves the prior
    /// playback running. The Session resolves against the displayed target
    /// snapshot; a target the snapshot no longer holds acts on nothing.
    pub(in crate::app) fn confirm_hand_off_playback_to_session(
        &mut self,
        device_id: &str,
        name: &str,
        key: KeyEvent,
    ) {
        let resolve = |panel_targets: &[crate::app::state::panel_targets::PanelTarget]| {
            panel_targets.iter().find_map(|target| match target {
                crate::app::state::panel_targets::PanelTarget::Emby(sess)
                    if sess.device_id == device_id =>
                {
                    Some((**sess).clone())
                }
                _ => None,
            })
        };
        match key.code {
            Key::Enter => {
                let Some(sess) = resolve(&self.panel_targets) else {
                    self.flash(
                        format!("\"{name}\" is no longer available"),
                        ToastSeverity::Warning,
                    );
                    return;
                };
                let name = session_display_name(&sess);
                tracing::info!(name: "sessions.output.handoff", target: "sessions", device = %name, device_id = %device_id, "handing the active slot off to the Session as the owner's playback output");
                self.flash(format!("Handed off to {name}"), ToastSeverity::Success);
                self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
                self.select_playback_output(mbv_ctrl::OutputSelection::EmbySession {
                    device_id: sess.device_id.clone(),
                    name: sess.device_name.clone(),
                });
            }
            Key::Esc => {
                let Some(sess) = resolve(&self.panel_targets) else {
                    self.flash(
                        format!("\"{name}\" is no longer available"),
                        ToastSeverity::Warning,
                    );
                    return;
                };
                self.activate_session_target_without_hand_off(&sess);
            }
            _ => {}
        }
    }

    /// Activate a Session with no hand-off fork: a direct endpoint keeps the
    /// Direct remote control path and severs only the control links — the
    /// prior playback continues (suspended behind the new attachment when
    /// this Client held the local Owner); without a direct endpoint the
    /// owner's own session-to-Session selection is the hand-off.
    fn activate_session_target_without_hand_off(&mut self, sess: &mbv_emby::SessionInfo) {
        let has_direct_endpoint = self.session_direct_endpoint(sess).is_some();
        if has_direct_endpoint {
            self.sever_connections_without_output_change();
        }
        let direct_upgrade_error = match self.connect_direct_session(sess) {
            Ok(true) => return,
            Ok(false) => None,
            Err(e) => Some(e),
        };

        let name = session_display_name(sess);
        if !self.player.remote().supports_emby_session_output() {
            let warning = match direct_upgrade_error {
                Some(error) => format!(
                    "Direct mbv control failed: {error}; this Player owner cannot select an Emby Session"
                ),
                None => "This Player owner cannot select an Emby Session as its playback output"
                    .to_string(),
            };
            self.flash(warning, ToastSeverity::Warning);
            return;
        }
        if let Some(error) = direct_upgrade_error {
            self.flash(
                format!(
                    "Direct mbv control failed: {error}; selecting {name} as the playback output"
                ),
                ToastSeverity::Warning,
            );
        }
        tracing::info!(name: "sessions.output.select", target: "sessions", device = %name, device_id = %sess.device_id, "selecting an Emby Session as the owner's playback output");
        self.sever_connections_without_output_change();
        self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
        self.select_playback_output(mbv_ctrl::OutputSelection::EmbySession {
            device_id: sess.device_id.clone(),
            name: sess.device_name.clone(),
        });
    }
}
