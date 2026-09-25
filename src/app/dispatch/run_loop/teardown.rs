//! Shutdown/teardown handling, split out of `run_loop_events.rs` to keep that
//! file within the repository's file-size limit.

use crate::app::shell::Model;
use crate::app::{App, QUIT_REQUESTED};
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn player_join_outer_bound(quit_timeout: Duration) -> Duration {
    quit_timeout + Duration::from_millis(200) + Duration::from_secs(1)
}

fn join_visualizer_worker(handle: Option<JoinHandle<()>>) {
    if let Some(handle) = handle {
        crate::app::infra::visualizer_worker::join_worker(handle);
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
    /// Shared local-player teardown sequence for both the signal-triggered
    /// quit-watchdog path (SIGHUP/SIGTERM) and the normal in-app quit-key
    /// path (both now break out of `run()`'s event loop the same way) —
    /// these two used to diverge, one bounded and one not, which is #202:
    /// an unbounded join on a hung `report_stopped` call during shutdown
    /// could hold the single-instance flock indefinitely. The player thread's
    /// stopped report derives its own budget from `quit_timeout` via
    /// `Player::stop_for_shutdown`, while the visualizer join remains bounded
    /// independently by its worker shutdown timeout.
    ///
    /// Extracted from `run()`'s tail so it's callable directly against a
    /// stubbed `App` in tests without a real tty — `run()` itself remains
    /// untested end-to-end (unchanged status quo, not a regression; it has
    /// never had test coverage since it unconditionally calls
    /// `enable_raw_mode()`).
    fn current_auto_reconnect_target(&self) -> Option<mbv_core::config::LastRemoteConnection> {
        if let Some(library) = self.active_route.clone() {
            Some(mbv_core::config::LastRemoteConnection::LibraryRoute { library })
        } else if let Some(sess) = self.connected_session_state.as_ref() {
            Some(mbv_core::config::LastRemoteConnection::DirectSession {
                device_name: sess.device_name.clone(),
            })
        } else {
            self.direct_remote_label.as_ref().map(|device_name| {
                mbv_core::config::LastRemoteConnection::DirectSession {
                    device_name: device_name.clone(),
                }
            })
        }
    }

    pub(in crate::app) fn persist_current_auto_reconnect_target(&mut self) {
        let Some(last) = self.current_auto_reconnect_target() else {
            return;
        };
        if let Err(e) = mbv_core::config::save_last_remote_connection(Some(&last)) {
            log::warn!(target: "auto_reconnect", "current target persistence failed: {e}");
        }
    }

    /// Persist the launch snapshot supplied by the shell at this discrete
    /// orderly-exit boundary, then run the normal teardown. App never mirrors
    /// component-owned state while the TUI is running.
    pub(in crate::app) fn teardown(
        &mut self,
        quit_timeout: Duration,
        launch_state: Option<mbv_core::config::TuiLaunchState>,
    ) {
        if let Some(state) = launch_state {
            if let Err(error) = mbv_core::config::save_tui_launch_state(&state) {
                log::warn!(target: "launch_state", "failed to save TUI launch state: {error}");
            }
        }
        self.teardown_inner(quit_timeout);
    }

    fn teardown_inner(&mut self, quit_timeout: Duration) {
        // Signal the visualizer before starting player shutdown so its worker
        // can stop concurrently with the player thread.
        let visualizer_handle = self.visualizer.take().and_then(|mut worker| {
            let handle = worker.signal_stop();
            self.visualizer_window = Default::default();
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
        // Leave the daemon's player running when the TUI disconnects; only stop
        // and join the player when we own it locally. Both signal-triggered and
        // in-app quit paths share the same bounded local teardown.
        let (was_playing, current_idx, position_ticks, last_valid_pos) = {
            let st = self.player.status.lock().unwrap();
            (
                st.active,
                st.current_idx,
                st.position_ticks,
                st.last_valid_pos,
            )
        };
        log::info!(target: "player", "quit: requested={quit_requested} was_playing={was_playing} idx={current_idx} position_ticks={position_ticks} last_valid_pos={last_valid_pos} timeout={}s", quit_timeout.as_secs());
        self.flush_playing_position_on_teardown(was_playing, current_idx, last_valid_pos);
        if self.home_is_local_daemon {
            log::info!(
                target: "queue",
                "teardown persistence skipped: local daemon owns the authoritative queue"
            );
        } else {
            self.save_queue_state_no_clear();
        }
        // Coordinated daemon shutdown: when the policy gate
        // is true (launched against the local daemon and stay_alive is off),
        // send a bounded RequestShutdown to the daemon. The daemon owns
        // queue persistence (persist-before-acceptance); this client only
        // invokes the request. When the current player is a live Local
        // connection, use it directly; otherwise create a short-lived
        // DaemonEndpoint::Local connection without mutating self.player or
        // any route/queue-scope/MPRIS/auto-reconnect state.
        let stay_alive = {
            let config = self.config.lock().unwrap();
            config.stay_alive
        };
        let should_request_shutdown = self.home_is_local_daemon && !stay_alive;
        log::info!(
            target: "daemon_shutdown",
            "teardown: home_is_local_daemon={} stay_alive={} should_request_shutdown={}",
            self.home_is_local_daemon, stay_alive, should_request_shutdown
        );
        let shutdown_response =
            self.request_teardown_shutdown(quit_timeout, should_request_shutdown);
        if !self.player.is_remote() {
            self.player.stop_for_shutdown(quit_timeout);
            // During quit shutdown there is no progress-thread join and no WS
            // flush. The player thread's worst case is the bounded stopped
            // report (`quit_timeout`) plus the 200ms mpv quit fallback. The
            // one-second cushion makes the outer join bound
            // `quit_timeout + 200ms + 1s`. Join the visualizer while the
            // player is still shutting down, using its own bounded join.
            join_visualizer_worker(visualizer_handle);
            let outer_bound = player_join_outer_bound(quit_timeout);
            let started = Instant::now();
            self.player.join_or_timeout(outer_bound);
            let elapsed = started.elapsed();
            log::info!(target: "player", "quit: player join finished in {}ms (bound={}ms)",
                elapsed.as_millis(), outer_bound.as_millis());
        } else {
            join_visualizer_worker(visualizer_handle);
        }
        // After a failed shutdown request (Rejected, Disconnected,
        // TimedOut, or failure to connect Local), set a post-terminal message
        // that the local daemon may still be running and names `mbv -q`.
        if should_request_shutdown {
            self.record_shutdown_failure(shutdown_response);
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
            log::info!(target: "auto_reconnect", "teardown persistence skipped: launched as remote");
        } else if !self.config.lock().unwrap().auto_reconnect {
            log::info!(target: "auto_reconnect", "teardown persistence skipped: auto-reconnect disabled");
        } else {
            let last = self.current_auto_reconnect_target();
            log::info!(
                target: "auto_reconnect",
                "teardown decision={}",
                match &last {
                    Some(mbv_core::config::LastRemoteConnection::LibraryRoute { library }) =>
                        format!("save-library-route library={library:?}"),
                    Some(mbv_core::config::LastRemoteConnection::DirectSession { device_name }) =>
                        format!("save-direct-session device={device_name:?}"),
                    None => "clear".to_string(),
                }
            );
            match mbv_core::config::save_last_remote_connection(last.as_ref()) {
                Ok(()) => log::info!(target: "auto_reconnect", "state persistence succeeded"),
                Err(e) => log::warn!(target: "auto_reconnect", "state persistence failed: {e}"),
            }
        }
    }

    /// Update the playing item's position before saving: the
    /// `PlayerEvent::Stopped` that carries this update is never processed after
    /// the event loop breaks, and `last_valid_pos` (never zeroed during track
    /// transitions) is preferred over `position_ticks` (transiently 0 when
    /// QueueSession advances to the next track).
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
        let _ = self.player_tab.queue.update_slot_item(
            slot_id,
            mbv_core::playback_queue::QueueItem::Emby(Box::new(item)),
        );
        self.last_played_item_id = Some(last_id);
    }

    /// Invoke the coordinated daemon shutdown when the policy gate is true
    /// (launched against the local daemon with stay-alive off). Uses the live
    /// Local connection when present, else a short-lived Local connection that
    /// does not mutate `self.player` or any route/queue-scope/MPRIS/
    /// auto-reconnect state. `None` means no request was made.
    fn request_teardown_shutdown(
        &self,
        quit_timeout: Duration,
        should_request_shutdown: bool,
    ) -> Option<mbv_core::remote_player::ShutdownResponse> {
        if !should_request_shutdown {
            return None;
        }
        let current_is_local = matches!(
            self.player_endpoint,
            Some(mbv_core::remote_player::DaemonEndpoint::Local)
        );
        let current_connected = self.player.is_remote() && !self.player.is_remote_disconnected();
        if current_is_local && current_connected {
            // Invoke through the current live Local connection.
            if let Some(remote) = self.player.as_remote() {
                log::info!(target: "daemon_shutdown", "invoking request_shutdown through current Local connection");
                return Some(remote.request_shutdown(quit_timeout));
            }
            log::warn!(target: "daemon_shutdown", "current player_endpoint is Local but as_remote() returned None; falling back to short-lived connection");
        } else {
            // Create a short-lived Local connection.
            log::info!(target: "daemon_shutdown", "current target is not a live Local connection; creating short-lived Local connection");
        }
        Self::invoke_shutdown_via_short_lived_local(quit_timeout)
    }

    /// After a failed shutdown request (Rejected, Disconnected, TimedOut,
    /// Unsupported, or no response at all), set a post-terminal message that
    /// the local daemon may still be running and names `mbv -q`.
    fn record_shutdown_failure(
        &mut self,
        response: Option<mbv_core::remote_player::ShutdownResponse>,
    ) {
        use mbv_core::remote_player::ShutdownResponse;
        let Some(response) = response else {
            // Failed to connect or invoke the request.
            log::warn!(target: "daemon_shutdown", "failed to invoke shutdown request via Local connection");
            self.pending_exit_message = Some(
                "Local daemon may still be running (failed to connect). Use `mbv -q` to stop it."
                    .to_string(),
            );
            return;
        };
        match response {
            ShutdownResponse::Accepted => {
                log::info!(target: "daemon_shutdown", "daemon accepted shutdown request");
            }
            ShutdownResponse::Rejected { reason } => {
                log::warn!(target: "daemon_shutdown", "daemon rejected shutdown request: {reason}");
                self.pending_exit_message = Some(format!(
                    "Local daemon may still be running (shutdown rejected: {}). Use `mbv -q` to stop it.",
                    reason
                ));
            }
            ShutdownResponse::Disconnected => {
                log::warn!(target: "daemon_shutdown", "daemon disconnected before responding to shutdown request");
                self.pending_exit_message = Some(
                    "Local daemon may still be running (disconnected before responding). Use `mbv -q` to stop it.".to_string(),
                );
            }
            ShutdownResponse::TimedOut => {
                log::warn!(target: "daemon_shutdown", "daemon did not respond to shutdown request within timeout");
                self.pending_exit_message = Some(
                    "Local daemon may still be running (did not respond within timeout). Use `mbv -q` to stop it.".to_string(),
                );
            }
            ShutdownResponse::Unsupported => {
                log::warn!(target: "daemon_shutdown", "peer daemon does not support lifecycle-shutdown");
                self.pending_exit_message = Some(
                    "Local daemon is an older version and cannot be stopped remotely. Use `mbv -q` to stop it.".to_string(),
                );
            }
        }
    }

    /// Creates a short-lived DaemonEndpoint::Local connection and
    /// invoke request_shutdown through it without replacing self.player or
    /// mutating route, queue-scope, MPRIS, or auto-reconnect state. Returns
    /// None if the connection cannot be established.
    fn invoke_shutdown_via_short_lived_local(
        quit_timeout: Duration,
    ) -> Option<mbv_core::remote_player::ShutdownResponse> {
        use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
        match RemotePlayer::connect_endpoint(&DaemonEndpoint::Local) {
            Ok((remote, _event_rx)) => {
                log::info!(target: "daemon_shutdown", "short-lived Local connection established");
                let response = remote.request_shutdown(quit_timeout);
                // Disconnect the short-lived connection after the request.
                remote.disconnect();
                Some(response)
            }
            Err(e) => {
                log::warn!(target: "daemon_shutdown", "failed to establish short-lived Local connection: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::player_join_outer_bound;
    use std::time::Duration;

    #[test]
    fn player_join_outer_bound_includes_quit_fallback_and_cushion() {
        assert_eq!(
            player_join_outer_bound(Duration::from_secs(5)),
            Duration::from_millis(6_200)
        );
    }
}
