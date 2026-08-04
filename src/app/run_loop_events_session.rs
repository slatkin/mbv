//! `SessionEvent` handling, split out of `run_loop_events.rs` to keep that
//! file within the repository's file-size limit.

use crate::app::types_playback::PlaylistMutation;
use crate::app::{App, PanelFocus, SessionEvent};
use std::time::{Duration, Instant};

impl App {
    /// Handle a single `SessionEvent` from the sessions-poll channel. Faithful
    /// transcription of the match arms previously inlined in `run()`'s
    /// `sessions_rx` drain loop (see `drain_session_events`).
    pub(in crate::app) fn handle_session_event(&mut self, ev: SessionEvent) {
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
                    self.set_panel_focus(PanelFocus::Queue);
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
}
