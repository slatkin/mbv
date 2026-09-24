use super::super::*;

impl PlaybackRun {
    // libmpv2 returns MPV_EVENT_END_FILE failures as Err(Error::Raw(...)),
    // so classify the output-specific error before the generic event logging.
    pub(in crate::player) fn on_mpv_error(
        &mut self,
        error: libmpv2::Error,
        progress: &mut ProgressGuard,
    ) -> bool {
        if !is_clocked_audio_error(&error, self.config.audio_device.is_some()) {
            log::warn!(target: "player", "event error: {}", mpv_err_str(&error));
            return false;
        }

        let device = self.config.audio_device.clone().unwrap_or_default();
        self.close_prepared_source();
        progress.stop_and_join(self.progress_join_budget());
        self.close_prepared_source_at(self.last_valid_pos);
        let stopped_slot = self.active_slot_id();
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: stopped_slot,
            run_identity: self.run_identity,
            position_ticks: 0,
            played: false,
            consume: false,
            progress_report_accepted: false,
            error: Some(format!("audio output failed to start (device: {device})")),
        });
        true
    }

    /// Standalone origin: exactly one file, so any EndFile ends playback. The
    /// run idles afterward (mpv stays up awaiting Shutdown), so the shared
    /// status snapshot must stop advertising `active=true` here — unlike the
    /// queue paths, no Shutdown or TrackChanged will refresh it. A daemon
    /// client attaching later would otherwise inherit a "still playing"
    /// now-playing panel frozen at the final position.
    pub(in crate::player) fn on_end_file_standalone(
        &mut self,
        reason: EndFileReason,
        progress: &mut ProgressGuard,
    ) -> bool {
        if reason == mpv_end_file_reason::Quit {
            self.stop_runtime = Some(self.active_item().map_or(0, |item| item.runtime_ticks()));
        }
        let runtime = self.status.lock().unwrap().runtime_ticks;
        let natural_end = reason == mpv_end_file_reason::Eof && runtime > 0;
        let completed_is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
        let completed_slot_id = self.active_slot_id();

        if reason == mpv_end_file_reason::Quit {
            // Keep an external window close off the mpv event loop. Natural
            // EOF still reports synchronously before its completion event.
            self.report_stop_now_or_background(progress);
        } else {
            progress.stop_and_join(self.progress_join_budget());
            self.stop_report = StopReport::mark_sent(self.report_stopped_for_end_file(reason));
        }

        let lifecycle_pos = self.active_item().map_or(self.last_valid_pos, |item| {
            provider_lifecycle_close_pos(item, natural_end, runtime, self.last_valid_pos)
        });
        self.close_prepared_source_at(lifecycle_pos);
        self.status.lock().unwrap().active = false;

        if natural_end && self.reporter.has_session() {
            let id = self.reporter.ids.lock().unwrap().0.clone();
            if !completed_is_audio {
                match self.reporter.client.mark_played(id.as_str()) {
                    Ok(()) => log::info!(target: "player", "mark_played ok id={id}"),
                    Err(e) => {
                        log::warn!(target: "player", "mark_played failed id={id}: {e}; will retry");
                        self.mark_played_id = Some(id.clone());
                    }
                }
            }
        }
        if !self.stopped_event_sent {
            let _ = self.event_tx.send(PlayerEvent::Stopped {
                slot_id: completed_slot_id,
                run_identity: self.run_identity,
                position_ticks: 0,
                played: natural_end && !completed_is_audio && self.reporter.has_session(),
                consume: false,
                progress_report_accepted: self.stop_report.is_accepted(),
                error: None,
            });
            self.stopped_event_sent = true;
        }
        false
    }

    pub(in crate::player) fn on_shutdown(&mut self, progress: &mut ProgressGuard) {
        // Prefer the slot captured when the stop/quit was first observed
        // (design D2); fall back to the currently observed active slot only
        // when mpv shut down with no prior end-file to capture from. This is
        // the observed active slot, not an mpv-index fallback.
        let stopped_slot = self.stop_slot.take().or_else(|| self.active_slot_id());
        self.close_prepared_source();
        log::warn!(target: "player", "shutdown: last_valid_pos={} stop_report={:?}",
            self.last_valid_pos, self.stop_report);
        if self.stop_report == StopReport::NotSent {
            self.report_stop_now_or_background(progress);
        }
        let client = self.reporter.client.clone();
        if self.origin == PlaybackOrigin::Standalone {
            // Retry mark_played in a detached thread so Shutdown never blocks.
            if let Some(mid) = self.mark_played_id.take() {
                retry_mark_played(client.clone(), mid);
            }
            let completed_runtime = self
                .stop_runtime
                .or_else(|| self.active_item().map(|item| item.runtime_ticks()))
                .unwrap_or(0);
            let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
            let near_end = self.reporter.has_session()
                && is_near_end(is_audio, false, self.last_valid_pos, completed_runtime);
            if near_end {
                let id = self.reporter.ids.lock().unwrap().0.clone();
                retry_mark_played(client.clone(), id);
            }
            self.status.lock().unwrap().active = false;
            if !self.stopped_event_sent {
                let _ = self.event_tx.send(PlayerEvent::Stopped {
                    slot_id: stopped_slot,
                    run_identity: self.run_identity,
                    position_ticks: self.last_valid_pos,
                    played: near_end,
                    consume: false,
                    progress_report_accepted: self.stop_report.is_accepted(),
                    error: None,
                });
            }
            // mpv exited on its own (not via our stop command, e.g. the user
            // closed the mpv window directly) — despite the event name,
            // App::handle_player_event's PlayerEvent::MpvQuit arm does not
            // quit the app; it just clears some UI state and returns false.
            if self.quit_at.is_none() {
                let _ = self.event_tx.send(PlayerEvent::MpvQuit);
            }
            return;
        }
        self.status.lock().unwrap().active = false;
        // played and consume are deliberately the same value here: stopped_near_end
        // is already video-only (see is_near_end's !is_audio gate), so a quit/cancel
        // near the end of an audio item never sets either — consistent with on_end_file's
        // normal advance path, where only natural/next-up (not near-end) triggers audio consume.
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: stopped_slot,
            run_identity: self.run_identity,
            position_ticks: self.last_valid_pos,
            played: self.stopped_near_end,
            consume: self.stopped_near_end,
            progress_report_accepted: self.stop_report.is_accepted(),
            error: None,
        });
        // mpv exited on its own (not via our stop command) — tell the app to quit.
        if self.quit_at.is_none() {
            let _ = self.event_tx.send(PlayerEvent::MpvQuit);
        }
    }
}
