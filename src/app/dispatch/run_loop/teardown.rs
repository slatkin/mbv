//! Shutdown/teardown handling, split out of `run_loop_events.rs` to keep that
//! file within the repository's file-size limit.

use crate::app::shell::Model;
use crate::app::{App, QUIT_REQUESTED};
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::Duration;

fn join_visualizer_capture(handle: Option<JoinHandle<()>>) {
    if let Some(handle) = handle {
        mbv_visualizer::join_worker(handle);
    }
}

impl Model {
    /// Finish an orderly TUI teardown after taking the selected destination's
    /// bounded launch snapshot. The App remains the persistence authority;
    /// this shell query is the only reverse read from the mounted owner.
    pub(in crate::app) fn teardown(&mut self, quit_timeout: Duration) {
        let launch_state = self.launch_state_snapshot();
        self.app.teardown(quit_timeout, Some(launch_state));
    }
}

impl App {
    /// Persist exit state, apply the daemon lifetime policy, and stop the
    /// visualizer during either signal-triggered or in-app teardown.
    fn current_auto_reconnect_target(&self) -> Option<mbv_config::LastRemoteConnection> {
        if let Some(library) = self.active_route.clone() {
            Some(mbv_config::LastRemoteConnection::LibraryRoute { library })
        } else if let Some(sess) = self.connected_session_state.as_ref() {
            Some(mbv_config::LastRemoteConnection::DirectSession {
                device_name: sess.device_name.clone(),
            })
        } else {
            self.remote.direct_remote_label.as_ref().map(|device_name| {
                mbv_config::LastRemoteConnection::DirectSession {
                    device_name: device_name.clone(),
                }
            })
        }
    }

    pub(in crate::app) fn persist_current_auto_reconnect_target(&mut self) {
        let Some(last) = self.current_auto_reconnect_target() else {
            return;
        };
        if let Err(e) = mbv_config::save_last_remote_connection(Some(&last)) {
            tracing::warn!(name: "auto_reconnect.target_persist.failed", target: "auto_reconnect", error = %e, "current target persistence failed");
        }
    }

    /// Persist the launch snapshot supplied by the shell at this discrete
    /// orderly-exit boundary, then run the normal teardown. App never mirrors
    /// component-owned state while the TUI is running.
    pub(in crate::app) fn teardown(
        &mut self,
        quit_timeout: Duration,
        launch_state: Option<mbv_config::TuiLaunchState>,
    ) {
        if let Some(state) = launch_state
            && let Err(error) = mbv_config::save_tui_launch_state(&state)
        {
            tracing::warn!(name: "launch_state.save.failed", target: "launch_state", error = %error, "TUI launch state save failed");
        }
        self.teardown_inner(quit_timeout);
    }

    fn teardown_inner(&mut self, quit_timeout: Duration) {
        // Stop the visualizer before requesting daemon shutdown so its worker
        // can exit concurrently with the remote request.
        let visualizer_handle = self.visualizer.take().and_then(|mut worker| {
            let handle = worker.signal_stop();
            self.visualizer_window = mbv_visualizer::StereoSampleWindow::default();
            handle
        });
        // Advance the queue lineage so any late work from this process cannot
        // be applied after teardown.
        self.advance_queue_epoch();
        // #236: persist the active remote connection before anything below or
        // in the caller's cleanup path clears route identity. The full gating
        // rationale lives on `persist_auto_reconnect_target_on_teardown`.
        self.persist_auto_reconnect_target_on_teardown();
        let quit_requested = QUIT_REQUESTED.load(Ordering::Relaxed);
        // Preserve the remote owner's playback unless the daemon lifetime
        // policy below requests its shutdown.
        let (was_playing, current_idx, position_ticks, last_valid_pos) = {
            let st = self.player.status.lock().unwrap();
            (
                st.active,
                st.current_idx,
                st.position_ticks,
                st.last_valid_pos,
            )
        };
        tracing::info!(name: "player.quit.started", target: "player", requested = quit_requested, was_playing, current_index = current_idx, position_ticks, last_valid_position_ticks = last_valid_pos, timeout_seconds = quit_timeout.as_secs(), "player quit started");
        self.flush_playing_position_on_teardown(was_playing, current_idx, last_valid_pos);
        // Coordinated shutdown always uses the home link, even when it is
        // suspended behind a routed remote player. Flush settings first: the
        // daemon reads the current lifetime policy when it handles the request.
        let stay_alive = {
            let config = self.config.lock().unwrap();
            config.stay_alive
        };
        let should_request_shutdown = self.home_is_local_daemon && !stay_alive;
        tracing::info!(name: "daemon_shutdown.teardown.evaluated", target: "daemon_shutdown", home_is_local_daemon = self.home_is_local_daemon, stay_alive, should_request_shutdown, "daemon shutdown policy evaluated");
        self.flush_settings_save();
        let shutdown_response =
            self.request_teardown_shutdown(quit_timeout, should_request_shutdown);
        join_visualizer_capture(visualizer_handle);
        // After a failed shutdown request (Rejected, Disconnected,
        // TimedOut, or failure to connect Local), set a post-terminal message
        // that the local daemon may still be running and names `mbv -q`.
        if should_request_shutdown && let Some(response) = shutdown_response {
            self.record_shutdown_failure(Some(response));
        }
    }

    /// Persist whichever remote connection (if any) is active at teardown, so
    /// the next launch's `App::new` can restore it (#236). Mutually exclusive
    /// by construction: library routing and Sessions-panel direct-remote are
    /// two independent ways to end up thin-client, and #223's
    /// `restore_local_mode` / `connect_to_session` never let both be set at
    /// once. Gated on `auto_reconnect` so the file is never written (or read)
    /// at all when the feature is off, and on
    /// `launched_as_remote && !home_is_local_daemon`: keyed off
    /// `home_is_local_daemon` (the immutable launch-time snapshot) rather than
    /// the mutable `is_local_daemon`, because a local-daemon-launched session
    /// now routinely calls `try_auto_reconnect()` on attach (`App::new_remote`)
    /// and may reconnect to a genuinely remote target mid-session, flipping
    /// `is_local_daemon` to `false` while still needing its connection
    /// persisted at teardown. A genuinely remote launch (`--connect-daemon`)
    /// never flips `home_is_local_daemon`, so running this for it would always
    /// compute `None` and wipe out a real record saved by a different
    /// `App::new` session (per ADR 0010, `new_remote`'s path is unaffected by
    /// #236). A same-host local daemon is meant to behave exactly like a local
    /// session (see the `new_remote` doc comment), so it must not be skipped.
    fn persist_auto_reconnect_target_on_teardown(&mut self) {
        if self.launched_as_remote && !self.home_is_local_daemon {
            tracing::info!(name: "auto_reconnect.target_persist.skipped", target: "auto_reconnect", reason = "launched_as_remote", "auto-reconnect target persistence skipped");
        } else if !self.config.lock().unwrap().auto_reconnect {
            tracing::info!(name: "auto_reconnect.target_persist.skipped", target: "auto_reconnect", reason = "disabled", "auto-reconnect target persistence skipped");
        } else {
            let last = self.current_auto_reconnect_target();
            tracing::info!(name: "auto_reconnect.target_persist.decided", target: "auto_reconnect", target_state = ?last, "auto-reconnect persistence decision made");
            match mbv_config::save_last_remote_connection(last.as_ref()) {
                Ok(()) => {
                    tracing::info!(name: "auto_reconnect.target_persist.succeeded", target: "auto_reconnect", "auto-reconnect state persistence succeeded");
                }
                Err(e) => {
                    tracing::warn!(name: "auto_reconnect.target_persist.failed", target: "auto_reconnect", error = %e, "auto-reconnect state persistence failed");
                }
            }
        }
    }

    /// Update the playing item's position before saving: the
    /// `PlayerEvent::Stopped` that carries this update is never processed after
    /// the event loop breaks, and `last_valid_pos` (never zeroed during track
    /// transitions) is preferred over `position_ticks` (transiently 0 when
    /// `QueueSession` advances to the next track).
    fn flush_playing_position_on_teardown(
        &mut self,
        was_playing: bool,
        current_idx: usize,
        last_valid_pos: i64,
    ) {
        if !was_playing || self.has_direct_remote_queue() {
            return;
        }
        let Some(slot) = self.player_tab.queue.slots().get(current_idx) else {
            return;
        };
        let slot_id = slot.slot_id;
        let Some(item) = slot.item.as_emby() else {
            return;
        };
        let mut item = item.clone();
        if last_valid_pos > 0 && !item.is_audio() {
            item.playback_position_ticks = last_valid_pos;
        }
        let last_id = item.id.clone();
        let _ = self
            .player_tab
            .queue
            .update_slot_item(slot_id, mbv_queue::QueueItem::Emby(Box::new(item)));
        self.last_played_item_id = Some(last_id);
    }

    /// Invoke coordinated shutdown over the home link (current or suspended).
    /// `None` means no request was made or the home link is unavailable.
    fn request_teardown_shutdown(
        &self,
        quit_timeout: Duration,
        should_request_shutdown: bool,
    ) -> Option<mbv_remote_player::ShutdownResponse> {
        if !should_request_shutdown {
            return None;
        }
        let home_link = self
            .suspended_local
            .as_ref()
            .filter(|home| !home.player.is_remote_disconnected())
            .map(|home| &home.player)
            .or_else(|| {
                self.player_endpoint
                    .as_ref()
                    .is_some_and(mbv_remote_player::DaemonEndpoint::is_local)
                    .then_some(&self.player)
            });
        let Some(home_link) = home_link
            .filter(|player| player.as_remote().is_some() && !player.is_remote_disconnected())
        else {
            tracing::info!(name: "daemon_shutdown.connection.unavailable", target: "daemon_shutdown", reason = "home_link_unavailable", "home link unavailable for shutdown request");
            return None;
        };
        let remote = home_link.as_remote()?;
        tracing::info!(name: "daemon_shutdown.request.started", target: "daemon_shutdown", connection = if self.suspended_local.is_some() { "suspended_home" } else { "current_home" }, "invoking shutdown request through home link");
        Some(remote.request_shutdown(quit_timeout))
    }

    /// After a failed shutdown request (Rejected, Disconnected, `TimedOut`,
    /// Unsupported, or no response at all), set a post-terminal message that
    /// the local daemon may still be running and names `mbv -q`.
    fn record_shutdown_failure(&mut self, response: Option<mbv_remote_player::ShutdownResponse>) {
        use mbv_remote_player::ShutdownResponse;
        let Some(response) = response else {
            // Failed to connect or invoke the request.
            tracing::warn!(name: "daemon_shutdown.request.failed", target: "daemon_shutdown", reason = "no_response", "failed to invoke shutdown request via Local connection");
            self.pending_exit_message = Some(
                "Local daemon may still be running (failed to connect). Use `mbv -q` to stop it."
                    .to_string(),
            );
            return;
        };
        match response {
            ShutdownResponse::Accepted => {
                tracing::info!(name: "daemon_shutdown.request.accepted", target: "daemon_shutdown", "daemon accepted shutdown request");
            }
            ShutdownResponse::Rejected { reason } => {
                tracing::warn!(name: "daemon_shutdown.request.rejected", target: "daemon_shutdown", reason = %reason, "daemon rejected shutdown request");
                self.pending_exit_message = Some(format!(
                    "Local daemon may still be running (shutdown rejected: {reason}). Use `mbv -q` to stop it."
                ));
            }
            ShutdownResponse::Disconnected => {
                tracing::warn!(name: "daemon_shutdown.request.disconnected", target: "daemon_shutdown", "daemon disconnected before responding to shutdown request");
                self.pending_exit_message = Some(
                    "Local daemon may still be running (disconnected before responding). Use `mbv -q` to stop it.".to_string(),
                );
            }
            ShutdownResponse::TimedOut => {
                tracing::warn!(name: "daemon_shutdown.request.timed_out", target: "daemon_shutdown", "daemon shutdown request timed out");
                self.pending_exit_message = Some(
                    "Local daemon may still be running (did not respond within timeout). Use `mbv -q` to stop it.".to_string(),
                );
            }
            ShutdownResponse::Unsupported => {
                tracing::warn!(name: "daemon_shutdown.request.unsupported", target: "daemon_shutdown", "peer daemon does not support lifecycle shutdown");
                self.pending_exit_message = Some(
                    "Local daemon is an older version and cannot be stopped remotely. Use `mbv -q` to stop it.".to_string(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SuspendedLocalSession;
    use crate::app::tests::make_app_stub;
    use mbv_player::PlayerProxy;

    #[test]
    fn quitting_with_stay_alive_off_stops_this_machines_local_daemon() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_app_stub();
        app.home_is_local_daemon = true;
        app.config.lock().unwrap().stay_alive = false;

        let (home_remote, home_rx, home_peer) =
            mbv_remote_player::connect_stub_daemon_pair().unwrap();
        app.suspended_local = Some(SuspendedLocalSession {
            player: PlayerProxy::remote(home_remote, false),
            player_rx: home_rx,
        });
        let (routed_remote, routed_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
        app.player = PlayerProxy::remote(routed_remote, false);
        app.player_rx = routed_rx;
        app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Tcp(
            "127.0.0.1:1234".parse().unwrap(),
        ));
        app.active_route = Some("music".to_string());

        app.teardown(Duration::ZERO, None);

        assert!(
            app.pending_exit_message
                .as_deref()
                .is_some_and(|message| message.contains("did not respond within timeout"))
        );
        app.suspended_local
            .as_ref()
            .unwrap()
            .player
            .as_remote()
            .unwrap()
            .disconnect();
        home_peer.join().unwrap();
    }

    // Owns daemon-lifecycle "Quitting with Stay Alive off stops this machine's local daemon".
    #[test]
    fn teardown_after_local_restart_uses_the_live_home_link() {
        let mut app = make_app_stub();
        app.home_is_local_daemon = true;
        app.config.lock().unwrap().stay_alive = false;

        let (dead_home, dead_rx, dead_peer) =
            mbv_remote_player::connect_stub_daemon_pair().unwrap();
        dead_home.disconnect();
        dead_peer.join().unwrap();
        app.suspended_local = Some(SuspendedLocalSession {
            player: PlayerProxy::remote(dead_home, false),
            player_rx: dead_rx,
        });

        let (restarted_home, restarted_rx, restarted_peer) =
            mbv_remote_player::connect_stub_daemon_pair().unwrap();
        app.player = PlayerProxy::remote(restarted_home, false);
        app.player_rx = restarted_rx;
        app.player_endpoint = Some(mbv_remote_player::DaemonEndpoint::Local);

        let response = app.request_teardown_shutdown(Duration::ZERO, true);

        assert!(matches!(
            response,
            Some(mbv_remote_player::ShutdownResponse::TimedOut)
        ));
        assert!(app.pending_exit_message.is_none());
        app.player.disconnect_remote();
        restarted_peer.join().unwrap();
    }
}
