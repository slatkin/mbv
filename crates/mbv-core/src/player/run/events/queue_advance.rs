use crate::audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};

use super::super::*;
use super::event_classifiers::{is_superseded_jump_end_file, provider_lifecycle_close_pos};

impl PlaybackRun {
    /// Returns true if the event loop should `continue`.
    pub(in crate::player) fn on_end_file(
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
        // The completed occurrence's owner-assigned identity, resolved from the
        // observed active slot at the moment the end-file is seen and carried to
        // every emit/defer site below — never re-resolved against a sequence a
        // later QueueMove/QueueRemove may have mutated (design D2).
        let completed_slot_id = self.active_slot_id();
        if let Some(done) = self.fail_active_file_start(reason, progress, completed_slot_id) {
            return done;
        }
        if reason == mpv_end_file_reason::Error {
            log::warn!(target: "player", "EndFile: playback error (file may be unreadable or format unsupported)");
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

        let completed_idx = completed_slot_id.and_then(|slot_id| self.queue.slot_index(slot_id));
        log::warn!(target: "player", "advance path: reason={reason:?} last_valid_pos={} runtime={}",
            self.last_valid_pos, self.status.lock().unwrap().runtime_ticks);
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
        let track_finished = natural || near_end || was_next_up;
        if is_superseded_jump_end_file(reason, self.forced_slot_id.is_some(), track_finished) {
            return self.handle_superseded_jump(reason, completed_slot_id, mpv);
        }
        // played_out drives mark-played/Emby watched-status and stays video-only;
        // consume_track drives queue auto-removal and is type-agnostic — the app layer
        // gates it per-type against consume_videos/consume_audio.
        let played_out = track_finished && !completed_is_audio;
        let consume_track = track_finished;
        log::info!(target: "consume", "on_end_file decision: idx={completed_idx:?} reason={reason:?} \
            natural={natural} near_end={near_end} was_next_up={was_next_up} \
            completed_is_audio={completed_is_audio} last_valid_pos={} runtime={} \
            => played_out={played_out} consume_track={consume_track}",
            self.last_valid_pos, completed_runtime);
        let completed_pos =
            queue_completed_pos(completed_is_audio, natural, near_end, self.last_valid_pos);
        // Keep this run's own queue mirror current too: it is never refreshed
        // from the owner's canonical queue mid-session, so a later relative
        // Next/Previous step (which resolves resume position locally, not via
        // a dispatched command) would otherwise still see this slot's
        // submission-time position.
        if let Some(slot_id) = completed_slot_id {
            self.queue
                .apply_progress(slot_id, completed_pos, played_out);
        }

        // Consume the in-flight jump's identity alongside `forced_slot_id`
        // (same lifetime, design D4); it tags the `TrackChanged` emit below
        // only if this observation actually lands on its target slot.
        let settling_transition = self.forced_transition.take();
        let logged_forced_slot_id = self.forced_slot_id;
        self.forced_jump_from_idle = false;
        let next_idx = self
            .forced_slot_id
            .take()
            .and_then(|slot_id| self.queue.slot_index(slot_id))
            .unwrap_or(self.current_idx + 1);
        log::info!(
            target: "transition",
            "on_end_file settle: forced_slot_id={:?} next_idx={} current_idx={} queue_len={} settling_transition={}",
            logged_forced_slot_id,
            next_idx,
            self.current_idx,
            self.queue_len(),
            settling_transition.is_some(),
        );

        if next_idx >= self.queue_len() {
            return self.stop_at_queue_end(
                completed_slot_id,
                &completed_item,
                completed_pos,
                played_out,
                consume_track,
                natural,
                completed_runtime,
                progress,
            );
        }

        // Update UI to the next track immediately, before slow network calls.
        // next_idx < queue_len() was already checked above, so set_active_index
        // (which only fails when the index is out of bounds) cannot fail here.
        let next_slot_id = self.slot_id_at(next_idx);
        let advanced = if self.active_file {
            self.close_prepared_source_at(provider_lifecycle_close_pos(
                &completed_item,
                natural,
                completed_runtime,
                self.last_valid_pos,
            ));
            next_slot_id.is_some_and(|slot_id| self.select_active_slot(slot_id, mpv).is_ok())
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
            return self.stop_for_failed_advance(completed_slot_id, progress);
        }
        self.tracks_initialized = false;
        self.set_next_item_status(&next_item);

        let stop_report_accepted = self.reporter.report_stopped(completed_pos);
        self.mark_played_or_retry(played_out, &completed_item);

        let _ = mpv.set_property("start", "0");
        self.queue_next_up.reset();
        if let Some(emby) = next_item.as_emby() {
            send_ep_info(mpv, emby);
        }
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);

        // Stop progress reporter during transition to prevent stale reports.
        progress.stop_and_join(self.progress_join_budget());
        self.start_next_item_reporting(&next_item);
        *progress = spawn_progress_reporter(self.reporter.clone());

        log::info!(target: "player", "playlist track-transition idx={}", self.current_idx);

        if let Some(completed_slot_id) = completed_slot_id {
            let _ = self.event_tx.send(PlayerEvent::TrackCompleted {
                slot_id: completed_slot_id,
                run_identity: self.run_identity,
                position_ticks: completed_pos,
                played: played_out,
                consume: consume_track,
                progress_report_accepted: stop_report_accepted,
            });
        }
        if let Some(next_slot_id) = self.active_slot_id() {
            self.emit_track_changed(next_slot_id, settling_transition);
        }
        false
    }

    /// Swallow EndFiles displaced by a queue submission while its drain is
    /// pending. Returns true when the caller must `continue`.
    fn swallow_pending_load(&mut self) -> bool {
        if self.load_state.is_ready() {
            return false;
        }
        if self.load_state.drain() == Drained::HitZero {
            // Once all pending EndFiles from a queue submission are drained, the new item's
            // lifecycle begins — reset stop_report so on_end_file/on_shutdown can report it.
            self.stop_report.reset();
        }
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
        progress.stop_and_join(self.progress_join_budget());
        self.close_prepared_source_at(self.last_valid_pos);
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: 0,
            played: false,
            consume: false,
            progress_report_accepted: false,
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
        let completed_runtime = self.active_item().map_or(0, |item| item.runtime_ticks());
        self.stop_runtime = Some(completed_runtime);
        let near_end = is_near_end(
            completed_is_audio,
            false,
            self.last_valid_pos,
            completed_runtime,
        );
        log::warn!(target: "player", "quit path: last_valid_pos={} runtime={} stop_report={:?}",
            self.last_valid_pos, completed_runtime, self.stop_report);
        if self.stop_report == StopReport::NotSent {
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

    /// Completed slot vanished under a concurrent QueueRemove: stop rather
    /// than advancing from garbage (H11).
    fn stop_for_missing_completed(
        &mut self,
        completed_slot_id: Option<QueueSlotId>,
        completed_idx: Option<usize>,
        progress: &mut ProgressGuard,
    ) -> bool {
        log::warn!(target: "player", "on_end_file: completed_idx={completed_idx:?} out of bounds (len={}), stopping",
            self.queue_len());
        progress.stop_and_join(self.progress_join_budget());
        self.status.lock().unwrap().active = false;
        self.stop_report = StopReport::mark_sent(self.reporter.report_stopped(self.last_valid_pos));
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: self.last_valid_pos,
            played: false,
            consume: false,
            progress_report_accepted: self.stop_report.is_accepted(),
            error: None,
        });
        false
    }

    /// Debris EndFile from a superseded JumpTo vs a real abandonment where
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
            log::info!(
                target: "player",
                "on_end_file: dropping superseded-jump EndFile (reason={reason:?}); \
                 mpv drives the next start-file (mpv_pos={mpv_pos})",
            );
            return true;
        };
        log::warn!(
            target: "player",
            "on_end_file: EndFile(Stop) abandoned the active entry; mpv is on entry {index} \
             (was {}) — adopting it",
            self.current_idx,
        );
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
    /// the run (signals run() to return).
    #[allow(clippy::too_many_arguments)]
    fn stop_at_queue_end(
        &mut self,
        completed_slot_id: Option<QueueSlotId>,
        completed_item: &QueueItem,
        completed_pos: i64,
        played_out: bool,
        consume_track: bool,
        natural: bool,
        completed_runtime: i64,
        progress: &mut ProgressGuard,
    ) -> bool {
        progress.stop_and_join(self.progress_join_budget());
        self.status.lock().unwrap().active = false;
        self.stop_report = StopReport::mark_sent(self.reporter.report_stopped(completed_pos));
        self.close_prepared_source_at(provider_lifecycle_close_pos(
            completed_item,
            natural,
            completed_runtime,
            self.last_valid_pos,
        ));
        self.mark_played_or_retry(played_out, completed_item);
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: completed_pos,
            played: played_out,
            consume: consume_track,
            progress_report_accepted: self.stop_report.is_accepted(),
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
        progress.stop_and_join(self.progress_join_budget());
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: completed_slot_id,
            run_identity: self.run_identity,
            position_ticks: 0,
            played: false,
            consume: false,
            progress_report_accepted: false,
            error: Some(format!("failed to prepare media: {error}")),
        });
        false
    }

    fn set_next_item_status(&mut self, next_item: &QueueItem) {
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
            log::warn!(target: "player", "mark_played failed id={id}: {e}; scheduling retry");
            retry_mark_played(self.reporter.client.clone(), id);
        }
    }

    /// Point Emby reporting at the item now playing — or clear the session
    /// for a non-Emby item so the reporter becomes a no-op. The outgoing
    /// item's report_stopped was already sent with the original IDs.
    fn start_next_item_reporting(&mut self, next_item: &QueueItem) {
        if let Some(emby) = next_item.as_emby() {
            let (urls, ok) = self.reporter.start_item(emby);
            self.ext_sub_urls = urls;
            if !ok {
                log::warn!(target: "player", "start_item failed for playlist track-transition item={}", emby.id);
            }
            return;
        }
        self.ext_sub_urls = vec![];
        // Feed item: clear Emby session IDs so the progress reporter
        // (if any) becomes a no-op.  The old item's report_stopped has
        // already been sent above with the original IDs.
        self.reporter.clear_session();
    }
}
