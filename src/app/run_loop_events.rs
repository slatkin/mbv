use super::types_playback::{PlaylistMutation, RemoteConsumeOperation};
use super::{App, SessionEvent, QUIT_REQUESTED};
use mbv_core::remote_reconciliation::{ReconciliationEffect, RemoteObservation};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub(super) fn validate_remote_playlist_entry(
    items: &[mbv_core::api::MediaItem],
    entry_id: &str,
    expected_media_id: &str,
) -> Result<bool, String> {
    match items.iter().find(|item| item.playlist_item_id == entry_id) {
        None => Ok(false),
        Some(item) if item.id == expected_media_id => Ok(true),
        Some(item) => Err(format!(
            "playlist entry {entry_id} now identifies media {} instead of {expected_media_id}",
            item.id
        )),
    }
}

impl App {
    fn apply_remote_observation(&mut self, session: &mbv_core::api::SessionInfo, generation: u64) {
        let Some(tracker) = self.remote_tracker.as_mut() else {
            return;
        };
        let observation = if let Some(media_id) = session.now_playing_item_id.clone() {
            RemoteObservation::playing(
                generation,
                session.id.clone(),
                media_id,
                session.position_ticks,
                session.runtime_ticks,
                Self::now_ms(),
            )
        } else {
            RemoteObservation::stopped(generation, session.id.clone(), Self::now_ms())
        };
        let effects = tracker.observe(observation);
        for effect in effects {
            match effect {
                ReconciliationEffect::Completion(item) => {
                    log::info!(
                        target: "remote_reconciliation",
                        "completed remote occurrence={} media={}",
                        item.occurrence_id,
                        item.media_id
                    );
                    self.begin_remote_consume(item);
                }
                ReconciliationEffect::StateChanged { state, reason } => {
                    log::debug!(
                        target: "remote_reconciliation",
                        "tracking state={state:?} reason={reason:?}"
                    );
                }
                _ => {}
            }
        }
    }

    fn unresolved_consume(&mut self, error: String) {
        self.remote_unresolved_outcomes = self.remote_unresolved_outcomes.saturating_add(1);
        log::warn!(target: "remote_reconciliation", "unresolved playlist consume: {error}");
    }

    fn apply_remote_consumed_occurrence(&mut self, operation: &RemoteConsumeOperation) {
        let Some(slot_id) = operation.queue_slot_id else {
            return;
        };
        if operation.queue_lineage != self.remote_queue_lineage {
            return;
        }

        self.player_tab.sync_queue_model_from_items_if_needed();
        let selected_slot = self
            .player_tab
            .queue
            .slots()
            .get(self.player_tab.queue_cursor)
            .map(|slot| slot.slot_id);
        if !matches!(
            self.player_tab.queue.consume_slot(slot_id),
            mbv_core::playback_queue::QueueMutationResult::Applied(_)
        ) {
            return;
        }
        self.player_tab.sync_items_from_queue_model();
        if let Some(index) =
            selected_slot.and_then(|selected| self.player_tab.queue.slot_index(selected))
        {
            self.player_tab.queue_cursor = index;
        }
        // An attached session must still persist an intentionally empty queue;
        // the ordinary empty-save guard protects unrelated remote-control UI.
        self.save_queue_state_after_remote_projection();
    }

    fn begin_remote_consume(
        &mut self,
        occurrence: mbv_core::remote_reconciliation::SubmittedOccurrence,
    ) {
        let Some(playlist_id) = occurrence.playlist_id().map(str::to_string) else {
            return;
        };
        let is_audio = self
            .player_tab
            .items
            .iter()
            .find(|item| item.id == occurrence.media_id)
            .is_some_and(|item| item.is_audio());
        let consume_enabled = {
            let config = &self.client.lock().unwrap().config;
            if is_audio {
                config.consume_audio
            } else {
                config.consume_videos
            }
        };
        if !consume_enabled {
            return;
        }
        let Some(entry_id) = occurrence.playlist_item_id.clone() else {
            return;
        };
        let Some(tracker) = self.remote_tracker.as_mut() else {
            return;
        };
        if self.remote_consume_operations.len() >= 128 {
            log::warn!(target: "remote_reconciliation", "consume operation limit reached; deferring new consume");
            return;
        }
        let session_id = tracker.session_id().to_string();
        let tracking_id = tracker.tracking_id();
        let epoch = tracker.epoch();
        if !tracker.mark_consumed(occurrence.occurrence_id) {
            return;
        }
        let media_id = occurrence.media_id.clone();
        let occurrence_id = occurrence.occurrence_id;
        let operation_id = self.next_remote_consume_operation;
        self.next_remote_consume_operation = self.next_remote_consume_operation.saturating_add(1);
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        let queue_slot_id = self
            .remote_queue_projection
            .as_ref()
            .and_then(|projection| {
                (projection.session_id == session_id
                    && projection.epoch == epoch
                    && projection.queue_lineage == self.remote_queue_lineage)
                    .then(|| projection.occurrence_slots.get(&occurrence_id).copied())
                    .flatten()
            });
        self.remote_consume_operations.push(RemoteConsumeOperation {
            operation_id,
            mutation_id,
            session_id: session_id.clone(),
            tracking_id,
            epoch,
            occurrence_id,
            playlist_id: playlist_id.clone(),
            entry_id: entry_id.clone(),
            media_id: media_id.clone(),
            queue_slot_id,
            queue_lineage: self.remote_queue_lineage,
        });
        self.enqueue_playlist_mutation(
            playlist_id,
            PlaylistMutation::ConsumeValidate {
                mutation_id,
                operation_id,
                session_id,
                tracking_id,
                epoch,
                occurrence_id,
                entry_id,
                media_id,
            },
        );
    }

    /// Handle a single `SessionEvent` from the sessions-poll channel. Faithful
    /// transcription of the match arms previously inlined in `run()`'s
    /// `sessions_rx` drain loop (see `drain_session_events`).
    pub(super) fn handle_session_event(&mut self, ev: SessionEvent) {
        match ev {
            SessionEvent::Loaded {
                sessions,
                generation,
            } => {
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
                    if let Some(s) = self.sessions.iter().find(|s| &s.id == conn_id).cloned() {
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
                        self.apply_remote_observation(&s, generation);
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
                        // A poll gap means the connected session is not
                        // currently observable, but the logical attachment is
                        // still held (capable of observing a return), so
                        // tracking suspends rather than staying confidently
                        // current or retiring early. Only the three-miss
                        // policy clears the attachment, and tracking retires
                        // in that same transition (below).
                        if let Some(tracker) = self.remote_tracker.as_mut() {
                            tracker.session_disappeared();
                        }
                        if self.session_miss_count >= 3 {
                            log::warn!(target: "sessions", "connected session gone; disconnecting");
                            self.flash_status_high(
                                "Remote session ended; disconnected".to_string(),
                            );
                            self.connected_session_id = None;
                            self.connected_session_state = None;
                            self.retire_remote_tracking(false);
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
            SessionEvent::CommandAcknowledged(command) => {
                if let Some(tracker) = self.remote_tracker.as_mut() {
                    if tracker.session_id() == command.session_id
                        && tracker.tracking_id() == command.tracking_id
                        && tracker.epoch() == command.tracker_epoch
                    {
                        tracker.acknowledge_command(command.generation);
                    }
                }
            }
            SessionEvent::CommandError {
                error,
                reconciliation,
            } => {
                if let (Some(command), Some(tracker)) =
                    (reconciliation, self.remote_tracker.as_mut())
                {
                    if tracker.session_id() == command.session_id
                        && tracker.tracking_id() == command.tracking_id
                        && tracker.epoch() == command.tracker_epoch
                        && tracker.command_generation_matches(command.generation)
                    {
                        tracker.command_failed();
                        self.retire_remote_tracking(false);
                    }
                }
                self.flash_status_high(format!("Remote command failed: {error}"));
            }
            SessionEvent::ConsumeValidated {
                mutation_id,
                operation_id,
                tracking_id,
                session_id,
                epoch,
                occurrence_id,
                playlist_id,
                entry_id,
                media_id,
                result,
            } => {
                let operation = self
                    .remote_consume_operations
                    .iter()
                    .find(|op| {
                        op.operation_id == operation_id
                            && op.mutation_id == mutation_id
                            && op.session_id == session_id
                            && op.tracking_id == tracking_id
                            && op.epoch == epoch
                            && op.occurrence_id == occurrence_id
                            && op.playlist_id == playlist_id
                            && op.entry_id == entry_id
                            && op.media_id == media_id
                    })
                    .cloned();
                let Some(operation) = operation else {
                    return;
                };
                if let Err(error) = result {
                    if self.remote_tracker.as_ref().is_some_and(|tracker| {
                        tracker.session_id() == operation.session_id
                            && tracker.tracking_id() == operation.tracking_id
                            && tracker.epoch() == operation.epoch
                    }) {
                        self.unresolved_consume(error);
                    }
                    self.remote_consume_operations
                        .retain(|op| op.operation_id != operation.operation_id);
                    self.finish_playlist_mutation(&operation.playlist_id, operation.mutation_id);
                } else {
                    log::debug!(
                        target: "remote_reconciliation",
                        "validated consume operation={} media={}",
                        operation.operation_id,
                        operation.media_id
                    );
                    let eligible = self.remote_tracker.as_ref().is_some_and(|tracker| {
                        tracker.session_id() == session_id
                            && tracker.tracking_id() == tracking_id
                            && tracker.consume_pending(epoch, occurrence_id)
                    });
                    if matches!(result, Ok(false)) {
                        self.remote_consume_operations
                            .retain(|op| op.operation_id != operation.operation_id);
                        self.apply_remote_consumed_occurrence(&operation);
                        self.finish_playlist_mutation(
                            &operation.playlist_id,
                            operation.mutation_id,
                        );
                    } else if !eligible {
                        self.remote_consume_operations
                            .retain(|op| op.operation_id != operation.operation_id);
                        self.finish_playlist_mutation(
                            &operation.playlist_id,
                            operation.mutation_id,
                        );
                    } else {
                        self.replace_active_playlist_mutation(
                            &operation.playlist_id,
                            operation.mutation_id,
                            PlaylistMutation::ConsumeDelete {
                                mutation_id: operation.mutation_id,
                                operation_id: operation.operation_id,
                                session_id: operation.session_id,
                                tracking_id: operation.tracking_id,
                                epoch: operation.epoch,
                                occurrence_id: operation.occurrence_id,
                                entry_id: operation.entry_id,
                                media_id: operation.media_id,
                            },
                        );
                    }
                }
            }
            SessionEvent::ConsumeOutcome {
                mutation_id,
                operation_id,
                tracking_id,
                session_id,
                epoch,
                occurrence_id,
                playlist_id,
                entry_id,
                media_id,
                result,
            } => {
                if let Some(index) = self.remote_consume_operations.iter().position(|op| {
                    op.operation_id == operation_id
                        && op.mutation_id == mutation_id
                        && op.session_id == session_id
                        && op.tracking_id == tracking_id
                        && op.epoch == epoch
                        && op.occurrence_id == occurrence_id
                        && op.playlist_id == playlist_id
                        && op.entry_id == entry_id
                        && op.media_id == media_id
                }) {
                    let operation = self.remote_consume_operations.remove(index);
                    if let Err(error) = result {
                        if self.remote_tracker.as_ref().is_some_and(|tracker| {
                            tracker.session_id() == session_id
                                && tracker.tracking_id() == tracking_id
                                && tracker.epoch() == epoch
                        }) {
                            self.unresolved_consume(error);
                        }
                    } else {
                        self.apply_remote_consumed_occurrence(&operation);
                    }
                    self.finish_playlist_mutation(&operation.playlist_id, operation.mutation_id);
                }
            }
            SessionEvent::PlaylistMutationComplete {
                mutation_id,
                playlist_id,
                queue_lineage,
                source_playlist_id,
                result,
            } => {
                let succeeded = result.is_ok();
                if let Err(error) = result {
                    self.flash_status_high(format!("Playlist save failed: {error}"));
                } else if queue_lineage == self.remote_queue_lineage
                    && self.queue_playlist_id() == Some(source_playlist_id.as_str())
                {
                    self.queue_dirty = false;
                    // A successful Save recreated server entry identities
                    // (cleared locally at the mutation boundary); persist that
                    // cleared state so stale identities cannot survive restart.
                    self.save_queue_state();
                }
                self.finish_playlist_mutation(&playlist_id, mutation_id);
                if succeeded
                    && queue_lineage == self.remote_queue_lineage
                    && self.queue_playlist_id() == Some(playlist_id.as_str())
                    && self.pending_queue_action.is_some()
                {
                    if let Some(action) = self.pending_queue_action.take() {
                        self.execute_pending_queue_action(action);
                    }
                    self.show_playlists = false;
                    self.set_panel_focus(super::PanelFocus::Queue);
                }
            }
            SessionEvent::PlaylistReplacementComplete {
                mutation_id,
                playlist_id,
                queue_lineage,
                source_playlist_id,
                name,
                result,
            } => {
                match result {
                    Ok(id) if queue_lineage == self.remote_queue_lineage => {
                        if self.remote_tracking_source_is(&source_playlist_id) {
                            self.retire_remote_tracking(true);
                        }
                        self.queue_source =
                            crate::config::QueueSource::Playlist { id: Some(id), name };
                        self.queue_dirty = false;
                        // The queue now identifies the replacement playlist; its
                        // items must not retain entry identities from a previously
                        // current source. Persist before reporting the overwrite
                        // clean so stale identities cannot survive restart.
                        self.clear_local_playlist_entry_ids();
                        self.save_queue_state();
                    }
                    Ok(_) => {
                        log::debug!(target: "playlist", "discarding stale playlist replacement completion")
                    }
                    Err(error) => {
                        self.flash_status_high(format!("Playlist overwrite failed: {error}"))
                    }
                }
                self.finish_playlist_mutation(&playlist_id, mutation_id);
            }
            SessionEvent::PlaylistCreateComplete {
                mutation_id,
                coordinator_key,
                name,
                queue_lineage,
                source_playlist_id,
                result,
            } => {
                match result {
                    Ok(id)
                        if queue_lineage == self.remote_queue_lineage
                            && self.queue_playlist_id() == source_playlist_id.as_deref() =>
                    {
                        self.queue_source = crate::config::QueueSource::Playlist {
                            id: Some(id),
                            name: name.clone(),
                        };
                        self.queue_dirty = false;
                        // The new source must never retain entry identities
                        // from the old playlist.
                        self.clear_local_playlist_entry_ids();
                        self.save_queue_state();
                        self.flash_status(format!("Saved as playlist \"{name}\""));
                    }
                    Ok(_) => {
                        log::debug!(target: "playlist", "discarding stale Save As completion");
                    }
                    Err(error) => self.flash_status_high(format!("Playlist save failed: {error}")),
                }
                self.finish_playlist_mutation(&coordinator_key, mutation_id);
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

    pub(super) fn persist_current_auto_reconnect_target(&mut self) {
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

    pub(super) fn teardown(&mut self, quit_timeout: Duration) {
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
        } else if !self.client.lock().unwrap().config.auto_reconnect {
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
            if let Some(item) = self.player_tab.items.get_mut(current_idx) {
                if last_valid_pos > 0 && !item.is_audio() {
                    item.playback_position_ticks = last_valid_pos;
                }
                self.last_played_item_id = Some(item.id.clone());
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
            let client = self.client.lock().unwrap();
            (client.config.stay_alive, client.token.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;

    #[test]
    fn playlist_entry_validation_distinguishes_absent_match_and_mismatch() {
        let mut item = make_item("Track", "Audio");
        item.id = "media-1".into();
        item.playlist_item_id = "entry-1".into();
        let items = vec![item];

        assert_eq!(
            validate_remote_playlist_entry(&items, "missing", "media-1"),
            Ok(false)
        );
        assert_eq!(
            validate_remote_playlist_entry(&items, "entry-1", "media-1"),
            Ok(true)
        );
        assert!(validate_remote_playlist_entry(&items, "entry-1", "media-2").is_err());
    }
}
