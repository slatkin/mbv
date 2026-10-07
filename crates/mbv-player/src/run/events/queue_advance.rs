use mbv_audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};

use super::super::{
    AdvanceDecisionInput, CompletedMedia, EndFileReason, FinishReason, Mpv, PlaybackOrigin,
    PlaybackRun, PlayerEvent, ProgressGuard, QueueItem, QueueSlotId, StopReport, advance_decision,
    is_near_end, mpv_end_file_reason, mpv_position_ticks, retry_mark_played, send_ep_info,
    spawn_progress_reporter,
};
use super::event_classifiers::{is_superseded_jump_end_file, provider_lifecycle_close_pos};
use mbv_ids::ItemId;
use std::sync::atomic::Ordering;

/// The completed item's outcome, past the end of the queue (`stop_at_queue_end`).
struct QueueEndStop<'a> {
    completed_slot_id: Option<QueueSlotId>,
    completed_item: &'a QueueItem,
    completed_pos: i64,
    played_out: bool,
    consume_track: bool,
    natural: bool,
    completed_runtime: i64,
}

impl PlaybackRun {
    /// Returns true if the event loop should `continue`.
    pub(crate) fn on_end_file(
        &mut self,
        reason: EndFileReason,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) -> bool {
        if self.quit_at.is_some() {
            return true;
        }
        if self.swallow_pending_load() {
            return true;
        }
        // Pin the completed occurrence's identity before QueueMove/QueueRemove
        // can mutate the sequence (design D2).
        let completed_slot_id = self.active_slot_id();
        if let Some(done) = self.fail_active_file_start(reason, progress, completed_slot_id) {
            return done;
        }
        if reason == mpv_end_file_reason::Error {
            tracing::warn!(name: "player.end_file.playback_error", target: "player", "playback error; file may be unreadable or unsupported");
        }

        let completed_is_audio = self.reporter.is_audio.load(Ordering::Relaxed);

        if let Some(done) =
            self.handle_queue_quit(reason, progress, completed_slot_id, completed_is_audio)
        {
            return done;
        }

        if self.origin == PlaybackOrigin::Standalone {
            return self.on_end_file_standalone(reason, progress);
        }

        self.on_end_file_queue(reason, mpv, progress, completed_slot_id, completed_is_audio)
    }

    /// Queue-origin end-of-file: advance to the next entry, or stop at the end
    /// of the queue.
    fn on_end_file_queue(
        &mut self,
        reason: EndFileReason,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
        completed_slot_id: Option<QueueSlotId>,
        completed_is_audio: bool,
    ) -> bool {
        let completed_idx = completed_slot_id.and_then(|slot_id| self.queue.slot_index(slot_id));
        tracing::warn!(name: "player.queue_advance.started", target: "player", reason = ?reason, last_valid_position = self.last_valid_pos, runtime = self.status.lock().unwrap().runtime_ticks, "advancing queue after end-file");
        // H11: bounds-check completed_idx — QueueRemove can shrink the list
        // while the current track is finishing.
        let Some(completed_item) = completed_slot_id
            .and_then(|slot_id| self.queue.slot(slot_id))
            .map(|slot| slot.item.clone())
        else {
            return self.stop_for_missing_completed(completed_slot_id, completed_idx, progress);
        };
        let completed_runtime = completed_item.runtime_ticks();
        let natural = reason == mpv_end_file_reason::Eof && completed_runtime > 0;
        let near_end = is_near_end(
            completed_is_audio,
            natural,
            self.last_valid_pos,
            completed_runtime,
        );
        let was_next_up = std::mem::replace(&mut self.next_up_jump, false);
        let (track_finished, played_out, consume_track, completed_pos) =
            advance_decision(&AdvanceDecisionInput {
                media: if completed_is_audio {
                    CompletedMedia::Audio
                } else {
                    CompletedMedia::Video
                },
                finish: if natural {
                    FinishReason::Natural
                } else if near_end {
                    FinishReason::NearEnd
                } else if was_next_up {
                    FinishReason::NextUp
                } else {
                    FinishReason::Unfinished
                },
                last_valid_pos: self.last_valid_pos,
            });
        if is_superseded_jump_end_file(
            reason,
            self.forced_jump.filter(|_| !self.active_file),
            track_finished,
        ) {
            return self.handle_superseded_jump(reason, completed_slot_id, mpv);
        }
        // Played stays video-only; consume is type-agnostic and gated per type
        // by the app layer.
        tracing::info!(name: "player.consume.decision", target: "consume", index = ?completed_idx, reason = ?reason, natural, near_end, was_next_up, is_audio = completed_is_audio, last_valid_position = self.last_valid_pos, runtime = completed_runtime, played = played_out, consume = consume_track, "computed completed-track outcome");
        let (next_idx, settling_transition) = self.settle_forced_jump();
        let end = QueueEndStop {
            completed_slot_id,
            completed_item: &completed_item,
            completed_pos,
            played_out,
            consume_track,
            natural,
            completed_runtime,
        };
        if next_idx >= self.queue_len() {
            return self.stop_at_queue_end(&end, progress);
        }

        self.advance_to_next_track(mpv, progress, &end, next_idx, settling_transition)
    }

    fn settle_forced_jump(&mut self) -> (usize, Option<crate::transition::Transition>) {
        let jump = self.forced_jump.take();
        let transition = jump.and_then(|jump| jump.transition);
        let slot_id = jump.map(|jump| jump.slot_id);
        let next_idx = slot_id
            .and_then(|slot_id| self.queue.slot_index(slot_id))
            .unwrap_or(self.current_idx + 1);
        tracing::info!(name: "player.transition.end_file_settled", target: "transition", slot = ?slot_id, next_index = next_idx, current_index = self.current_idx, queue_length = self.queue_len(), has_transition = transition.is_some(), "settled forced jump on end-file");
        if next_idx < self.queue_len() {
            self.forced_jump = jump.filter(|jump| jump.resume_ticks.is_some()).map(|jump| {
                crate::run::ForcedJump {
                    transition: None,
                    from_idle: false,
                    ..jump
                }
            });
        }
        (next_idx, transition)
    }

    /// Advance to the already-bounds-checked next track: select it, emit the
    /// transition, and report false (keep running). Extracted from
    /// `on_end_file` (issue #803); the `next_idx < queue_len()` check above
    /// still guarantees `set_active_index` cannot fail here.
    fn advance_to_next_track(
        &mut self,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
        end: &QueueEndStop<'_>,
        next_idx: usize,
        settling_transition: Option<crate::transition::Transition>,
    ) -> bool {
        // Update UI to the next track immediately, before slow network calls.
        // next_idx < queue_len() was already checked above, so set_active_index
        // (which only fails when the index is out of bounds) cannot fail here.
        let next_slot_id = self.slot_id_at(next_idx);
        let advanced = if self.active_file {
            self.close_prepared_source_at(provider_lifecycle_close_pos(
                end.completed_item,
                end.natural,
                end.completed_runtime,
                self.last_valid_pos,
            ));
            let selected =
                next_slot_id.is_some_and(|slot_id| self.select_active_slot(slot_id, mpv).is_ok());
            if selected {
                // The completed file already ended (this EndFile), so the
                // replacement the selection armed has nothing left to drain.
                // Left armed, it swallows the next track's real EndFile and
                // the one-entry playlist goes idle.
                let _ = self.on_drained();
            }
            selected
        } else {
            self.set_active_index(next_idx)
        };
        debug_assert!(
            advanced,
            "set_active_index({next_idx}) must succeed: already bounds-checked against queue_len={}",
            self.queue_len()
        );
        let next_item = self
            .active_item()
            .cloned()
            .expect("active item must exist after successful set_active_index");
        self.load_active_item_state();
        if self.active_file && !advanced {
            return self.stop_for_failed_advance(end.completed_slot_id, progress);
        }
        self.tracks_initialized = false;
        self.set_next_item_status(&next_item);

        let _ = self.reporter.report_stopped(end.completed_pos);
        self.mark_played_or_retry(end.played_out, end.completed_item);

        let _ = mpv.set_property("start", "0");
        self.queue_next_up.reset();
        if let Some(emby) = next_item.as_emby() {
            send_ep_info(mpv, emby);
        }
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);

        // Stop progress reporter during transition to prevent stale reports.
        progress.stop_and_join(Self::progress_join_budget());
        self.start_next_item_reporting(&next_item);
        *progress = spawn_progress_reporter(self.reporter.clone());

        tracing::info!(name: "player.track.transitioned", target: "player", index = self.current_idx, "playlist track transitioned");

        if let Some(completed_slot_id) = end.completed_slot_id {
            let _ = self.event_tx.send(PlayerEvent::TrackCompleted {
                slot_id: completed_slot_id,
                run_identity: self.run_identity,
                position_ticks: end.completed_pos,
                played: end.played_out,
                consume: end.consume_track,
            });
        }
        if let Some(next_slot_id) = self.active_slot_id() {
            self.emit_track_changed(next_slot_id, settling_transition);
        }
        false
    }

    /// Swallow `EndFiles` displaced by a queue submission while its drain is
    /// pending. Returns true when the caller must `continue`.
    fn swallow_pending_load(&mut self) -> bool {
        if self.load_is_ready() {
            return false;
        }
        let _ = self.on_drained();
        true
    }

    /// Active-file start failure: the outgoing file errored before playback
    /// began. Returns Some(false) when handled.
    fn fail_active_file_start(
        &mut self,
        reason: EndFileReason,
        progress: &mut ProgressGuard,
        completed_slot_id: Option<QueueSlotId>,
    ) -> Option<bool> {
        if !self.active_file || !self.active_file_starting {
            return None;
        }
        if reason != mpv_end_file_reason::Error {
            return None;
        }
        self.active_file_starting = false;
        self.close_prepared_source();
        progress.stop_and_join(Self::progress_join_budget());
        self.close_prepared_source_at(self.last_valid_pos);
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: 0,
            played: false,
            consume: false,
            error: Some("failed to start media".into()),
        });
        Some(false)
    }

    /// mpv-initiated quit mid-queue: report stop without blocking mpv's own
    /// shutdown, record the near-end decision for the deferred Shutdown emit.
    /// Returns Some(true) when handled.
    fn handle_queue_quit(
        &mut self,
        reason: EndFileReason,
        progress: &mut ProgressGuard,
        completed_slot_id: Option<QueueSlotId>,
        completed_is_audio: bool,
    ) -> Option<bool> {
        if self.origin != PlaybackOrigin::Queue || reason != mpv_end_file_reason::Quit {
            return None;
        }
        let completed_runtime = self.active_item().map_or(0, QueueItem::runtime_ticks);
        self.stop_runtime = Some(completed_runtime);
        let near_end = is_near_end(
            completed_is_audio,
            false,
            self.last_valid_pos,
            completed_runtime,
        );
        tracing::warn!(name: "player.queue.quit", target: "player", last_valid_position = self.last_valid_pos, runtime = completed_runtime, stop_report = ?self.stop_report(), "queue playback quit");
        if self.is_unreported() {
            // mpv-initiated quits (for example a compositor close request)
            // must not wait on Emby before mpv can finish its own shutdown.
            self.report_stop_now_or_background(progress);
        }
        if near_end && !completed_is_audio && self.reporter.has_session() {
            let id = self.reporter.ids.lock().unwrap().0.clone();
            self.mark_played_id_or_retry(id);
        }
        self.stopped_near_end = near_end;
        self.stop_slot = completed_slot_id;
        Some(true) // wait for Shutdown to fire PlayerEvent::Stopped
    }

    /// Completed slot vanished under a concurrent `QueueRemove`: stop rather
    /// than advancing from garbage (H11).
    fn stop_for_missing_completed(
        &mut self,
        completed_slot_id: Option<QueueSlotId>,
        completed_idx: Option<usize>,
        progress: &mut ProgressGuard,
    ) -> bool {
        tracing::warn!(name: "player.queue.completed_slot_missing", target: "player", index = ?completed_idx, queue_length = self.queue_len(), "completed item is out of bounds; stopping");
        progress.stop_and_join(Self::progress_join_budget());
        self.status.lock().unwrap().active = false;
        let _ = self.reporter.report_stopped(self.last_valid_pos);
        self.mark_reported(StopReport::Sent);
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: self.last_valid_pos,
            played: false,
            consume: false,
            error: None,
        });
        false
    }

    /// Debris `EndFile` from a superseded `JumpTo` vs a real abandonment where
    /// mpv left the entry by itself: mpv's current entry tells them apart.
    fn handle_superseded_jump(
        &mut self,
        reason: EndFileReason,
        completed_slot_id: Option<QueueSlotId>,
        mpv: &Mpv,
    ) -> bool {
        // A `Stop` with no jump in flight is usually debris from a
        // superseded jump, but it is also exactly what a real abandonment
        // looks like: mpv left the entry by itself. mpv's current entry is
        // the only thing that tells them apart, and dropping the real one
        // leaves the run reporting an item mpv is not playing.
        let (mpv_pos, divergent) = self.mpv_divergent_entry(mpv);
        let Some(index) = divergent else {
            // mpv is still on the entry the run believes in — the debris
            // this guard was written for — or it has no entry at all. The
            // idle case is left as-is deliberately: a transient idle is
            // indistinguishable from a real stop here, and stopping the
            // run on a transient one would cost playback. The logged
            // ordinal makes the difference visible in the field.
            tracing::info!(name: "player.jump.superseded_end_file_dropped", target: "player", reason = ?reason, mpv_position = ?mpv_pos, "dropping superseded jump end-file; mpv drives next start-file");
            return true;
        };
        tracing::warn!(name: "player.playlist_entry.abandoned", target: "player", index, previous_index = self.current_idx, "mpv abandoned the active entry; adopting current entry");
        let abandoned_pos = self.last_valid_pos;
        self.report_stopped_and_adopt_mpv_entry(
            completed_slot_id,
            abandoned_pos,
            index,
            mpv_position_ticks(mpv),
        );
        true
    }

    /// Past the end of the queue: report the completed item stopped and end
    /// the run (signals `run()` to return).
    fn stop_at_queue_end(&mut self, stop: &QueueEndStop<'_>, progress: &mut ProgressGuard) -> bool {
        progress.stop_and_join(Self::progress_join_budget());
        self.status.lock().unwrap().active = false;
        let _ = self.reporter.report_stopped(stop.completed_pos);
        self.mark_reported(StopReport::Sent);
        self.close_prepared_source_at(provider_lifecycle_close_pos(
            stop.completed_item,
            stop.natural,
            stop.completed_runtime,
            self.last_valid_pos,
        ));
        self.mark_played_or_retry(stop.played_out, stop.completed_item);
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: stop.completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: stop.completed_pos,
            played: stop.played_out,
            consume: stop.consume_track,
            error: None,
        });
        false // signals run() to return
    }

    fn stop_for_failed_advance(
        &mut self,
        completed_slot_id: Option<QueueSlotId>,
        progress: &mut ProgressGuard,
    ) -> bool {
        let error = AudiobookshelfError::from_class(AudiobookshelfFailureClass::Unavailable);
        progress.stop_and_join(Self::progress_join_budget());
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: 0,
            played: false,
            consume: false,
            error: Some(format!("failed to prepare media: {error}")),
        });
        false
    }

    pub(crate) fn set_next_item_status(&mut self, next_item: &QueueItem) {
        let mut s = self.status.lock().unwrap();
        s.position_ticks = 0;
        s.runtime_ticks = next_item.runtime_ticks();
        s.current_idx = self.current_idx;
        s.queue_len = self.queue_len();
        if let Some(emby) = next_item.as_emby() {
            s.set_current_item_metadata(emby);
        } else {
            s.title = next_item.title().to_string();
            s.art_item_id = next_item.id().to_string();
        }
    }

    /// Best-effort mark-played for a completed Emby item, with background
    /// retry on failure. No-op unless `played_out`.
    fn mark_played_or_retry(&self, played_out: bool, completed_item: &QueueItem) {
        if !played_out {
            return;
        }
        let Some(emby) = completed_item.as_emby() else {
            return;
        };
        self.mark_played_id_or_retry(ItemId::new(emby.id.clone()));
    }

    fn mark_played_id_or_retry(&self, id: ItemId) {
        if let Err(e) = self.reporter.client.mark_played(id.as_str()) {
            tracing::warn!(name: "player.mark_played.failed", target: "player", item = %id, error = %e, "mark played failed; scheduling retry");
            retry_mark_played(std::sync::Arc::clone(&self.reporter.client), id);
        }
    }

    /// Point Emby reporting at the item now playing — or clear the session
    /// for a non-Emby item so the reporter becomes a no-op. The outgoing
    /// item's `report_stopped` was already sent with the original IDs.
    pub(crate) fn start_next_item_reporting(&mut self, next_item: &QueueItem) {
        self.in_playback_span(|this| {
            if let Some(emby) = next_item.as_emby() {
                let (urls, ok) = this.reporter.start_item(emby, &this.playback_span);
                this.ext_sub_urls = urls;
                if !ok {
                    tracing::warn!(name: "player.report_start.failed", target: "player", phase = "playlist_transition", item = %emby.id, "start report failed for playlist track transition");
                }
                return;
            }
            this.ext_sub_urls = vec![];
            // Feed item: clear Emby session IDs so the progress reporter
            // (if any) becomes a no-op.  The old item's report_stopped has
            // already been sent above with the original IDs.
            this.reporter.clear_session();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{ExecutionSequence, ForcedJump};
    use crate::tests::{abs_item, make_queue_session_for_pos_tests};

    #[test]
    fn active_file_jump_then_natural_end_advances_after_target() {
        let (mut run, _) = make_queue_session_for_pos_tests(0);
        let (a, b, c) = (
            QueueSlotId::from_raw(1),
            QueueSlotId::from_raw(2),
            QueueSlotId::from_raw(3),
        );
        run.queue = ExecutionSequence::from_slot_items(
            vec![(a, abs_item()), (b, abs_item()), (c, abs_item())],
            Some(b),
        );
        run.current_idx = 1;
        run.active_file = true;
        run.set_pending_playlist_jump(ForcedJump {
            slot_id: b,
            transition: Some(crate::transition::Transition::new(42, 1, b)),
            resume_ticks: None,
            from_idle: false,
        });

        let (next_idx, transition) = run.settle_forced_jump();

        assert_eq!(run.slot_id_at(next_idx), Some(c));
        assert_eq!(transition, None);
    }
}
