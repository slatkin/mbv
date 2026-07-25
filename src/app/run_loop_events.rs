use super::{App, SessionEvent, QUIT_REQUESTED};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

impl App {
    /// Handle a single `SessionEvent` from the sessions-poll channel. Faithful
    /// transcription of the match arms previously inlined in `run()`'s
    /// `sessions_rx` drain loop (see `drain_session_events`).
    pub(super) fn handle_session_event(&mut self, ev: SessionEvent) {
        match ev {
            SessionEvent::Loaded(sessions) => {
                let old_id = self
                    .sessions
                    .get(self.sessions_cursor)
                    .map(|s| s.id.clone());
                self.sessions = sessions;
                self.sessions_loading = false;
                self.last_session_poll = Instant::now();
                if let Some(id) = old_id {
                    if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
                        self.sessions_cursor = pos;
                    } else {
                        self.sessions_cursor = self
                            .sessions_cursor
                            .min(self.sessions.len().saturating_sub(1));
                        if !self.sessions.is_empty() {
                            log::warn!(target: "sessions", "selected session gone; cursor clamped");
                        }
                    }
                }
                // Update connected session state; auto-disconnect if gone
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    if let Some(s) = self.sessions.iter().find(|s| &s.id == conn_id) {
                        // Maintain a monotonic position estimate within a single video.
                        // Reset the anchor only when the playing item ID changes.
                        // Avoid keying on runtime or title — the API occasionally returns
                        // missing RunTimeTicks (as_i64 returns None → 0) or a slightly
                        // different name, which would spuriously reset the position anchor
                        // every poll and prevent smooth interpolation.
                        let now = Instant::now();
                        let prev_item_id = self
                            .connected_session_state
                            .as_ref()
                            .and_then(|p| p.now_playing_item_id.as_deref());
                        let item_changed = s.now_playing_item_id.as_deref() != prev_item_id;
                        if item_changed {
                            // Refresh the previous item so played/progress reflects
                            // what the remote client reported to the server.
                            if let Some(prev_id) = self
                                .connected_session_state
                                .as_ref()
                                .and_then(|p| p.now_playing_item_id.clone())
                            {
                                let client = self.client.lock().unwrap().clone();
                                let tx = self.sessions_tx.clone();
                                std::thread::spawn(move || {
                                    if let Ok(mut items) =
                                        client.get_items_by_ids(std::slice::from_ref(&prev_id))
                                    {
                                        if let Some(fresh) = items.pop() {
                                            let _ = tx.send(SessionEvent::ItemRefreshed(
                                                prev_id,
                                                Box::new(fresh),
                                            ));
                                        }
                                    }
                                });
                            }
                        }
                        // Detect playback via API position advancing, not IsPaused.
                        // Some Emby clients always report IsPaused=true even while playing;
                        // the only reliable signal is that PositionTicks keeps moving.
                        let prev_api_pos = self
                            .connected_session_state
                            .as_ref()
                            .map_or(0, |p| p.position_s);
                        if s.position_s > prev_api_pos {
                            self.remote_api_pos_advanced_at = now;
                        }
                        // Extrapolate if API advanced recently (within 2× the ~11s report
                        // interval). After that window lapses we treat it as paused/stopped.
                        let api_active = self.remote_api_pos_advanced_at.elapsed().as_secs() < 22;
                        let seek_pending = now < self.remote_seek_pending_until;
                        if seek_pending && !item_changed {
                            // A seek was just dispatched; hold the optimistic position until
                            // the API catches up. Once the API reports the new position (or
                            // the window expires) we fall through to normal reconciliation.
                            log::debug!(target: "sessions",
                                "pos hold (seek pending): api={}s remote_pos_s={}s",
                                s.position_s, self.remote_pos_s);
                        } else if item_changed {
                            log::debug!(target: "sessions",
                                "pos reset (item change): api_pos={}s → remote_pos_s {}s→{}s",
                                s.position_s, self.remote_pos_s, s.position_s);
                            self.remote_pos_s = s.position_s;
                            self.remote_api_pos_advanced_at = now;
                            self.remote_seek_pending_until = now - Duration::from_secs(1);
                        } else if api_active {
                            let elapsed = self.remote_pos_at.elapsed().as_secs_f64();
                            let extrapolated = Self::extrapolated_remote_position(
                                self.remote_pos_s,
                                self.remote_pos_at.elapsed(),
                            );
                            let new_pos = s.position_s.max(extrapolated);
                            log::debug!(target: "sessions",
                                "pos extrap: api={}s paused={} elapsed={:.2}s → remote_pos_s {}s→{}s",
                                s.position_s, s.is_paused, elapsed, self.remote_pos_s, new_pos);
                            self.remote_pos_s = new_pos;
                        } else {
                            log::debug!(target: "sessions",
                                "pos idle (no api advance in 22s): api_pos={}s → remote_pos_s {}s→{}s",
                                s.position_s, self.remote_pos_s, s.position_s);
                            self.remote_pos_s = s.position_s;
                        }
                        if !seek_pending || item_changed {
                            self.remote_pos_at = now;
                        }
                        if item_changed {
                            if let Some(new_idx) = s.now_playing_item_id.as_ref().and_then(|id| {
                                self.player_tab.items.iter().position(|it| &it.id == id)
                            }) {
                                self.player_tab.queue_cursor = new_idx;
                            }
                            self.runtime_zero_since = None;
                        }
                        self.connected_session_state = Some(s.clone());
                        self.session_miss_count = 0;
                        // Remote hasn't started playing yet — repoll sooner.
                        // Cap fast-poll at 30 s: if runtime stays 0 that long the
                        // remote client likely won't report it and we stop hammering.
                        if s.runtime_s == 0 {
                            let since = self.runtime_zero_since.get_or_insert_with(Instant::now);
                            if since.elapsed() < Duration::from_secs(30) {
                                self.last_session_poll =
                                    Instant::now() - Duration::from_millis(500);
                            }
                        } else {
                            self.runtime_zero_since = None;
                        }
                    } else {
                        self.session_miss_count += 1;
                        if self.session_miss_count >= 3 {
                            log::warn!(target: "sessions", "connected session gone; disconnecting");
                            self.flash_status_high(
                                "Remote session ended; disconnected".to_string(),
                            );
                            self.connected_session_id = None;
                            self.connected_session_state = None;
                            self.session_miss_count = 0;
                            self.remote_pos_s = 0;
                        } else {
                            log::warn!(target: "sessions", "connected session not in poll ({}/3); holding", self.session_miss_count);
                        }
                    }
                }
            }
            SessionEvent::ItemRefreshed(item_id, fresh) => {
                if let Some(slot) = self.player_tab.items.iter_mut().find(|i| i.id == item_id) {
                    *slot = *fresh;
                }
            }
            SessionEvent::Error(e) => {
                self.sessions_loading = false;
                self.flash_status_high(format!("Sessions error: {e}"));
            }
        }
    }

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
    pub(super) fn teardown(&mut self, quit_timeout: Duration) {
        // #236: persist whichever remote connection (if any) is active
        // right now, before anything below or in the caller's cleanup
        // path clears `active_route` / direct-session identity -- so the
        // next launch's `App::new` can restore it. Mutually exclusive by
        // construction (library routing and Sessions-panel direct-remote
        // are two independent ways to end up thin-client; #223's
        // `restore_local_mode` and `connect_to_session` never let both be
        // set at once). Gated on `auto_reconnect` so the file is
        // never written (or read) at all when the feature is off. Also
        // gated on `!launched_as_remote`: `App::new_remote` instances never
        // populate `active_route`/`connected_session_state` (those are set
        // only by `App::new`'s runtime library-route-switch / session-attach
        // mechanisms), so running this block for them would always compute
        // `None` and wipe out a real record saved by a different `App::new`
        // session (per ADR 0010, `new_remote`'s path is unaffected by #236).
        if self.launched_as_remote {
            log::info!(target: "auto_reconnect", "teardown persistence skipped: launched as remote");
        } else if !self.client.lock().unwrap().config.auto_reconnect {
            log::info!(target: "auto_reconnect", "teardown persistence skipped: auto-reconnect disabled");
        } else {
            let last = if let Some(library) = self.active_route.clone() {
                log::info!(target: "auto_reconnect", "teardown decision=save-library-route library={library:?}");
                Some(mbv_core::config::LastRemoteConnection::LibraryRoute { library })
            } else if let Some(sess) = self.connected_session_state.as_ref() {
                log::info!(target: "auto_reconnect", "teardown decision=save-direct-session device={:?}", sess.device_name);
                Some(mbv_core::config::LastRemoteConnection::DirectSession {
                    device_name: sess.device_name.clone(),
                })
            } else {
                log::info!(target: "auto_reconnect", "teardown decision={}", if self.direct_remote_label.is_some() { "save-direct-session" } else { "clear" });
                self.direct_remote_label.as_ref().map(|device_name| {
                    mbv_core::config::LastRemoteConnection::DirectSession {
                        device_name: device_name.clone(),
                    }
                })
            };
            match mbv_core::config::save_last_remote_connection(last.as_ref()) {
                Ok(()) => log::info!(target: "auto_reconnect", "state persistence succeeded"),
                Err(e) => log::warn!(target: "auto_reconnect", "state persistence failed: {e}"),
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
            if let Some(item) = self.player_tab.items.get_mut(current_idx) {
                if last_valid_pos > 0 && !item.is_audio() {
                    item.playback_position_ticks = last_valid_pos;
                }
                self.last_played_item_id = Some(item.id.clone());
            }
        }
        self.save_queue_state_no_clear();
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
    }
}
