//! Shutdown/teardown handling, split out of `run_loop_events.rs` to keep that
//! file within the repository's file-size limit.

use crate::app::{App, QUIT_REQUESTED};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

impl App {
    /// Shared local-player teardown sequence for both the signal-triggered
    /// quit-watchdog path (SIGHUP/SIGTERM) and the normal in-app quit-key
    /// path (both now break out of `run()`'s event loop the same way) —
    /// these two used to diverge, one bounded and one not, which is #202:
    /// an unbounded join on a hung `report_stopped` call during shutdown
    /// could hold the single-instance flock indefinitely. `quit_timeout`
    /// bounds every blocking step below; the player thread's own nested
    /// bounded calls (`ProgressGuard::stop_and_join`,
    /// `SessionReporter::report_stopped_for_shutdown`) each derive their
    /// own budget from the same value via `Player::stop_for_shutdown` —
    /// see the `outer_bound` comment below for why the outer join needs
    /// real headroom over those, not an identical `Duration`.
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
        if let Ok(value) = serde_json::to_value(&last) {
            let _ = self.persist_shared_document(
                mbv_core::shared_state::SharedDocumentKind::LastRemoteConnection,
                value,
            );
        }
    }

    pub(in crate::app) fn teardown(&mut self, quit_timeout: Duration) {
        // A position saved just before quitting is still only in memory --
        // `save_default_library_position` defers the disk write (see its
        // doc comment) -- so flush it now rather than waiting for the
        // run loop's idle check, which won't run again.
        self.flush_library_position_now();
        self.stop_visualizer_worker();
        // Process-local tracking must not outlive the process: retire the
        // session, its projection, and its unresolved presentation through the
        // same helper every other lifecycle boundary uses. Late consume
        // outcomes and stale unresolved counts are discarded with the exit.
        self.retire_remote_tracking(true);
        // #236: persist whichever remote connection (if any) is active
        // right now, before anything below or in the caller's cleanup
        // path clears `active_route` / direct-session identity -- so the
        // next launch's `App::new` can restore it. Mutually exclusive by
        // construction (library routing and Sessions-panel direct-remote
        // are two independent ways to end up thin-client; #223's
        // `restore_local_mode` and `connect_to_session` never let both be
        // set at once). Gated on `auto_reconnect` so the file is
        // never written (or read) at all when the feature is off. Also
        // gated on `launched_as_remote && !home_is_local_daemon`: keyed off
        // `home_is_local_daemon` (the immutable launch-time snapshot) rather
        // than the mutable `is_local_daemon`, because a local-daemon-launched
        // session now routinely calls `try_auto_reconnect()` on attach
        // (`App::new_remote`) and may reconnect to a genuinely remote
        // target mid-session, flipping `is_local_daemon` to `false` while
        // still needing its connection persisted at teardown. A genuinely
        // remote launch (`--connect-daemon`) never flips `home_is_local_daemon`,
        // so running this block for it would always compute `None` and wipe
        // out a real record saved by a different `App::new` session (per
        // ADR 0010, `new_remote`'s path is unaffected by #236). A same-host
        // local daemon is meant to behave exactly like a local session (see
        // the `new_remote` doc comment), so it must not be skipped here.
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
            if let Ok(value) = serde_json::to_value(last.as_ref()) {
                let _ = self.persist_shared_document(
                    mbv_core::shared_state::SharedDocumentKind::LastRemoteConnection,
                    value,
                );
            }
        }
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
        // Update the playing item's position before saving — the PlayerEvent::Stopped
        // that carries this update is never processed after we break out of the event loop.
        // Use last_valid_pos (never zeroed during track transitions) rather than
        // position_ticks (transiently 0 when QueueSession advances to the next track).
        if was_playing && !self.has_direct_remote_queue() {
            if let Some(slot) = self.player_tab.queue.slots().get(current_idx) {
                let slot_id = slot.slot_id;
                if let Some(item) = slot.item.as_emby() {
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
            }
        }
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
        let (stay_alive, auth_token) = {
            let config = self.config.lock().unwrap();
            let token = self
                .emby_client()
                .map(|client| client.lock().unwrap().token.clone())
                .unwrap_or_default();
            (config.stay_alive, token)
        };
        let should_request_shutdown = self.home_is_local_daemon && !stay_alive;
        let mut shutdown_response: Option<mbv_core::remote_player::ShutdownResponse> = None;
        if should_request_shutdown {
            let current_is_local = matches!(
                self.player_endpoint,
                Some(mbv_core::remote_player::DaemonEndpoint::Local)
            );
            let current_connected =
                self.player.is_remote() && !self.player.is_remote_disconnected();
            if current_is_local && current_connected {
                // Invoke through the current live Local connection.
                if let Some(remote) = self.player.as_remote() {
                    log::info!(target: "daemon_shutdown", "invoking request_shutdown through current Local connection");
                    shutdown_response = Some(remote.request_shutdown(quit_timeout));
                } else {
                    log::warn!(target: "daemon_shutdown", "current player_endpoint is Local but as_remote() returned None; falling back to short-lived connection");
                    shutdown_response =
                        Self::invoke_shutdown_via_short_lived_local(&auth_token, quit_timeout);
                }
            } else {
                // Create a short-lived Local connection.
                log::info!(target: "daemon_shutdown", "current target is not a live Local connection; creating short-lived Local connection");
                shutdown_response =
                    Self::invoke_shutdown_via_short_lived_local(&auth_token, quit_timeout);
            }
        }
        if !self.player.is_remote() {
            self.player.stop_for_shutdown(quit_timeout);
            // The two nested bounded calls inside the player thread's own
            // shutdown path (on_shutdown, run sequentially) do NOT share an
            // identical budget: PlaybackSession::progress_join_budget gives
            // ProgressGuard::stop_and_join only quit_timeout/2 (it's a
            // secondary, non-network-critical join), while
            // report_stopped_for_shutdown keeps the full quit_timeout as its
            // own budget (the session-terminating call, worth protecting
            // most — see progress_join_budget's doc comment). Worst case the
            // two together take quit_timeout/2 + quit_timeout =
            // 1.5*quit_timeout, so the outer bound below is that plus a 3s
            // cushion — a real, explicit margin for the remaining
            // bookkeeping and fixed overhead (thread-spawn cost, contended
            // locks, drop cleanup; mark_played retry is fire-and-forget on a
            // detached thread and the PlayerEvent::Stopped send is a cheap
            // channel op), not just "the same Duration racing every layer of
            // the timeout composition" as an earlier version of this
            // function did.
            let outer_bound = quit_timeout + quit_timeout / 2 + Duration::from_secs(3);
            let started = Instant::now();
            self.player.join_or_timeout(outer_bound);
            let elapsed = started.elapsed();
            log::info!(target: "player", "quit: player join finished in {}ms (bound={}ms)",
                elapsed.as_millis(), outer_bound.as_millis());
        }
        // After a failed shutdown request (Rejected, Disconnected,
        // TimedOut, or failure to connect Local), set a post-terminal message
        // that the local daemon may still be running and names `mbv -q`.
        if should_request_shutdown {
            if let Some(response) = shutdown_response {
                use mbv_core::remote_player::ShutdownResponse;
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
            } else {
                // Failed to connect or invoke the request.
                log::warn!(target: "daemon_shutdown", "failed to invoke shutdown request via Local connection");
                self.pending_exit_message = Some(
                    "Local daemon may still be running (failed to connect). Use `mbv -q` to stop it.".to_string(),
                );
            }
        }
    }

    /// Creates a short-lived DaemonEndpoint::Local connection and
    /// invoke request_shutdown through it without replacing self.player or
    /// mutating route, queue-scope, MPRIS, or auto-reconnect state. Returns
    /// None if the connection cannot be established.
    fn invoke_shutdown_via_short_lived_local(
        auth_token: &str,
        quit_timeout: Duration,
    ) -> Option<mbv_core::remote_player::ShutdownResponse> {
        use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
        match RemotePlayer::connect_endpoint(&DaemonEndpoint::Local, auth_token) {
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
