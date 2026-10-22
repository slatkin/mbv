use super::super::{
    EndFileReason, FailurePhase, FailureVerdict, MpvFailure, PlaybackOrigin, PlaybackRun,
    PlayerEvent, ProgressGuard, QueueItem, QueueSlotId, RunPosition, StopOutcome, StopReport,
    is_near_end, retry_mark_played,
};
use super::event_classifiers::{is_clocked_audio_error, provider_lifecycle_close_pos};
use std::sync::{Arc, atomic::Ordering};

impl PlaybackRun {
    // An end-file error arrives as MpvEvent::Failed, not as
    // MpvEvent::EndFile, so every error-exit path lives here and the
    // end-of-file paths never see an error. Classify the output-specific
    // failure before the generic event logging.
    pub(crate) fn on_mpv_error(
        &mut self,
        failure: &MpvFailure,
        progress: &mut ProgressGuard,
    ) -> bool {
        if self.stop_for_process_exit(failure, progress) {
            return true;
        }
        if self.stop_for_unrecoverable_error(failure, progress) {
            return true;
        }
        if !is_clocked_audio_error(failure, self.config.audio_device.is_some()) {
            tracing::warn!(name: "player.event.failed", target: "player", error = %failure, "mpv event failed");
            return false;
        }

        let device = self.config.audio_device.clone().unwrap_or_default();
        let failed_slot = self.active_slot_id();
        self.stop_run_with_error(
            progress,
            failed_slot,
            0,
            format!("audio output failed to start (device: {device})"),
        )
    }

    /// The child exited with no quit request: end the run like an
    /// audio-output failure, but report the last observed position so
    /// the stop lands where playback stopped (design D7, S8).
    fn stop_for_process_exit(
        &mut self,
        failure: &MpvFailure,
        progress: &mut ProgressGuard,
    ) -> bool {
        if !matches!(failure, MpvFailure::ProcessExited) {
            return false;
        }
        tracing::error!(name: "player.mpv.process_exited", target: "player", "mpv process exited without a quit request");
        let failed_slot = self.active_slot_id();
        self.stop_run_with_error(
            progress,
            failed_slot,
            self.last_valid_pos,
            "mpv process exited unexpectedly".to_owned(),
        );
        true
    }

    /// A generic error — a missing file, an HTTP 404 from the stream
    /// endpoint, a decode error — is judged by the lifecycle machine (design
    /// D2): before any playback began it ends the run (#941), after it only
    /// when mpv has no entry after the failed one to skip to (#942, #943).
    /// With one, mpv skips on its own and the playlist-position drift
    /// machinery follows (probe S5), so the warn-and-continue path stays
    /// there. The judged slot names the failed entry: the armed jump target
    /// while a jump was in flight (design D3), otherwise the entry mpv is on.
    fn stop_for_unrecoverable_error(
        &mut self,
        failure: &MpvFailure,
        progress: &mut ProgressGuard,
    ) -> bool {
        if !matches!(failure, MpvFailure::Other(_)) {
            return false;
        }
        let FailureVerdict::EndRun { failed_slot, phase } =
            self.run_lifecycle.load_failed(self.run_position())
        else {
            // mpv holds an entry after the failed one. Fall through silently:
            // `on_mpv_error` fires the run's one `player.event.failed` warn
            // for a failure it does not stop on.
            return false;
        };
        let error = match phase {
            FailurePhase::BeforeStart => {
                tracing::error!(name: "player.event.start_failed", target: "player", error = %failure, "file failed to start before playback began");
                format!("failed to start media: {failure}")
            }
            FailurePhase::AfterStart => {
                tracing::error!(name: "player.event.playback_failed", target: "player", error = %failure, "playback error with no next entry; ending the run");
                format!("failed to play media: {failure}")
            }
        };
        self.stop_run_with_error(progress, failed_slot, self.last_valid_pos, error)
    }

    /// The plain position facts a load failure is judged from (design D1/D2):
    /// the machine holds no queue, so the run resolves the armed jump
    /// target's ordinal before the call, and names the entry mpv is on.
    fn run_position(&self) -> RunPosition {
        RunPosition {
            current_idx: self.current_idx,
            queue_len: self.queue_len(),
            current_slot: self.slot_id_at(self.current_idx),
            jump_target_idx: self
                .run_lifecycle
                .jump_target_slot()
                .and_then(|slot_id| self.queue.slot_index(slot_id)),
        }
    }

    /// The teardown shared by an audio-output failure and an unexpected mpv
    /// exit: close the prepared source, join progress, and report one
    /// `Stopped` with `error` and `position_ticks` through the single exit
    /// (design D4). The close per run identity — not the Service report
    /// state — decides that the event goes out exactly once: an adoption's
    /// stopped report no longer mutes a later error stop (defect 1), and an
    /// earlier end for the same identity still allows no second one.
    fn stop_run_with_error(
        &mut self,
        progress: &mut ProgressGuard,
        failed_slot: Option<QueueSlotId>,
        position_ticks: i64,
        error: String,
    ) -> bool {
        self.close_prepared_source();
        progress.stop_and_join(Self::progress_join_budget());
        self.close_prepared_source_at(self.last_valid_pos);
        self.end_run(StopOutcome {
            slot_id: failed_slot,
            position_ticks,
            played: false,
            consume: false,
            error: Some(error),
        });
        true
    }

    /// Standalone origin: exactly one file, so any `EndFile` ends playback. The
    /// run idles afterward (mpv stays up awaiting Shutdown), so the shared
    /// status snapshot must stop advertising `active=true` here — unlike the
    /// queue paths, no Shutdown or `TrackChanged` will refresh it. A daemon
    /// client attaching later would otherwise inherit a "still playing"
    /// now-playing panel frozen at the final position.
    fn report_for_end_file(&mut self, reason: EndFileReason, progress: &mut ProgressGuard) {
        if reason == EndFileReason::Quit {
            // Keep an external window close off the mpv event loop. Natural
            // EOF still reports synchronously before its completion event.
            self.report_stop_now_or_background(progress);
            return;
        }
        progress.stop_and_join(Self::progress_join_budget());
        let _ = self.report_stopped_for_end_file(reason);
        self.mark_reported(StopReport::Sent);
    }

    fn retry_natural_mark_played(&mut self, natural_end: bool, completed_is_audio: bool) {
        if !natural_end || completed_is_audio || !self.reporter.has_session() {
            return;
        }
        let id = self.reporter.ids.lock().unwrap().0.clone();
        match self.reporter.client.mark_played(id.as_str()) {
            Ok(()) => {
                tracing::info!(name: "player.mark_played.succeeded", target: "player", item = %id, "mark played succeeded");
            }
            Err(e) => {
                tracing::warn!(name: "player.mark_played.failed", target: "player", item = %id, error = %e, "mark played failed; will retry");
                self.mark_played_id = Some(id.clone());
            }
        }
    }

    pub(crate) fn on_end_file_standalone(
        &mut self,
        reason: EndFileReason,
        progress: &mut ProgressGuard,
    ) -> bool {
        tracing::info!(name: "player.end_file.standalone", target: "player", reason = ?reason, "standalone run end-file");
        if reason == EndFileReason::Quit {
            self.pin_stop_runtime();
        }
        let runtime = self.status.lock().unwrap().runtime_ticks;
        let natural_end = reason == EndFileReason::Eof && runtime > 0;
        let completed_is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
        let completed_slot_id = self.active_slot_id();

        self.report_for_end_file(reason, progress);

        let lifecycle_pos = self.active_item().map_or(self.last_valid_pos, |item| {
            provider_lifecycle_close_pos(item, natural_end, runtime, self.last_valid_pos)
        });
        self.close_prepared_source_at(lifecycle_pos);
        self.status.lock().unwrap().active = false;

        self.retry_natural_mark_played(natural_end, completed_is_audio);
        self.end_run(StopOutcome {
            slot_id: completed_slot_id,
            position_ticks: 0,
            played: natural_end && !completed_is_audio && self.reporter.has_session(),
            consume: false,
            error: None,
        });
        false
    }

    pub(crate) fn on_shutdown(&mut self, progress: &mut ProgressGuard) {
        // Prefer the slot captured when the stop/quit was first observed
        // (design D2); fall back to the currently observed active slot only
        // when mpv shut down with no prior end-file to capture from. This is
        // the observed active slot, not an mpv-index fallback.
        let stopped_slot = self.stop_slot.take().or_else(|| self.active_slot_id());
        self.close_prepared_source();
        tracing::warn!(name: "player.shutdown.started", target: "player", last_valid_position = self.last_valid_pos, stop_report = ?self.stop_report(), "mpv shutdown");
        if self.is_unreported() {
            self.report_stop_now_or_background(progress);
        }
        let client = Arc::clone(&self.reporter.client);
        if self.origin == PlaybackOrigin::Standalone {
            self.shutdown_standalone(stopped_slot, &client);
            return;
        }
        self.shutdown_queue(stopped_slot);
    }

    fn shutdown_standalone(
        &mut self,
        stopped_slot: Option<QueueSlotId>,
        client: &Arc<mbv_emby::EmbyClient>,
    ) {
        // Retry mark_played in a detached thread so Shutdown never blocks.
        if let Some(mid) = self.mark_played_id.take() {
            retry_mark_played(Arc::clone(client), mid);
        }
        let completed_runtime = self
            .stop_runtime
            .or_else(|| self.active_item().map(QueueItem::runtime_ticks))
            .unwrap_or(0);
        let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
        let near_end = self.reporter.has_session()
            && is_near_end(is_audio, false, self.last_valid_pos, completed_runtime);
        if near_end {
            let id = self.reporter.ids.lock().unwrap().0.clone();
            retry_mark_played(Arc::clone(client), id);
        }
        self.end_run(StopOutcome {
            slot_id: stopped_slot,
            position_ticks: self.last_valid_pos,
            played: near_end,
            consume: false,
            error: None,
        });
        // mpv exited on its own (not via our stop command, e.g. the user
        // closed the mpv window directly) — despite the event name,
        // App::handle_player_event's PlayerEvent::MpvQuit arm does not
        // quit the app; it just clears some UI state and returns false.
        if self.quit_at.is_none() {
            let _ = self.event_tx.send(PlayerEvent::MpvQuit);
        }
    }

    fn shutdown_queue(&mut self, stopped_slot: Option<QueueSlotId>) {
        // played and consume are deliberately the same value here: stopped_near_end
        // is already video-only (see is_near_end's !is_audio gate), so a quit/cancel
        // near the end of an audio item never sets either — consistent with on_end_file's
        // normal advance path, where only natural/next-up (not near-end) triggers audio consume.
        self.end_run(StopOutcome {
            slot_id: stopped_slot,
            position_ticks: self.last_valid_pos,
            played: self.stopped_near_end,
            consume: self.stopped_near_end,
            error: None,
        });
        // mpv exited on its own (not via our stop command) — tell the app to quit.
        if self.quit_at.is_none() {
            let _ = self.event_tx.send(PlayerEvent::MpvQuit);
        }
    }
}
