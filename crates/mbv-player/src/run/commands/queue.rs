use super::{
    ExecSlot, Mpv, PlaybackRun, PlayerEvent, ProgressGuard, QueueItem, QueueSlotId, mpv_err_str,
    mpv_load_opts, mpv_url_for_queue_item, reject_stale_jump, resolve_jump_target,
    spawn_progress_reporter,
};
use crate::run::{ForcedJump, StopReport};
use crate::{EMBY_RESUME_RETRY_DELAYS, refresh_emby_resume};

impl PlaybackRun {
    /// Explicit jump to an owner-assigned slot. Resolves the slot to this
    /// run's mpv-local ordinal; a stale slot (gone here) is rejected, never
    /// repaired by position (design D6).
    pub(super) fn cmd_jump_to(
        &mut self,
        slot_id: QueueSlotId,
        request_id: mbv_ctrl::PlaybackRequestId,
        generation: mbv_ctrl::PlaybackGeneration,
        resume_ticks: Option<i64>,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        let slot_ids = self
            .queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>();
        let Some(idx) = resolve_jump_target(&slot_ids, slot_id) else {
            tracing::info!(name: "player.jump.rejected", target: "transition", slot = ?slot_id, "jump target is unresolvable");
            reject_stale_jump(&self.event_tx, slot_id);
            return;
        };
        let transition = crate::transition::Transition::new(request_id, generation, slot_id);
        // Design D1/1.3: an Emby video's resume position comes from Emby at
        // jump time, not from the owner's load-time value. Feed and
        // Audiobookshelf slots keep the owner's ticks.
        let resume_ticks = match self.queue.slot(slot_id).map(|slot| &slot.item) {
            Some(QueueItem::Emby(emby)) if !emby.is_audio() => {
                let refreshed = refresh_emby_resume(
                    vec![QueueItem::Emby(emby.clone())],
                    |ids| self.reporter.client.get_items_by_ids(ids),
                    &EMBY_RESUME_RETRY_DELAYS,
                );
                crate::resume_ticks_for_item(&refreshed[0])
            }
            _ => resume_ticks,
        };
        self.set_pending_playlist_jump(ForcedJump {
            slot_id,
            transition: Some(transition),
            resume_ticks,
            from_idle: false,
        });
        if self.active_file {
            self.cmd_jump_to_active_file(slot_id, resume_ticks, Some(transition), mpv, progress);
        } else {
            self.cmd_jump_to_playlist(slot_id, idx, resume_ticks, mpv);
        }
    }

    pub(crate) fn set_pending_playlist_jump(&mut self, jump: ForcedJump) {
        self.forced_jump = (!self.active_file).then_some(jump);
    }

    fn cmd_jump_to_active_file(
        &mut self,
        slot_id: QueueSlotId,
        resume_ticks: Option<i64>,
        transition: Option<crate::transition::Transition>,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        let outgoing_pos = self.last_valid_pos;
        match self.select_active_slot_with_resume(slot_id, resume_ticks, mpv) {
            Ok(()) => {
                let _ = mpv.set_property("pause", false);
                self.report_jumped_item(outgoing_pos, progress);
                // Active-file projection has no mpv playlist move to
                // observe, so emit the JumpTo observation here.
                self.emit_track_changed(slot_id, transition);
            }
            Err(error) => {
                tracing::warn!(name: "player.active_file.selection_failed", target: "player", error = %error, "active-file selection failed");
            }
        }
    }

    /// An active-file jump swallows the replaced file's `EndFile`, so it does
    /// the reporting `on_end_file` does for a playlist move: stop the outgoing
    /// item, then point Emby reporting and the status mirror at the new one.
    pub(crate) fn report_jumped_item(&mut self, outgoing_pos: i64, progress: &mut ProgressGuard) {
        let Some(item) = self.active_item().cloned() else {
            return;
        };
        progress.stop_and_join(Self::progress_join_budget());
        let _ = self.reporter.report_stopped(outgoing_pos);
        self.tracks_initialized = false;
        self.set_next_item_status(&item);
        self.start_next_item_reporting(&item);
        self.mark_reported(StopReport::NotSent);
        *progress = spawn_progress_reporter(self.reporter.clone());
    }

    fn cmd_jump_to_playlist(
        &mut self,
        slot_id: QueueSlotId,
        idx: usize,
        resume_ticks: Option<i64>,
        mpv: &Mpv,
    ) {
        // mpv playlist indices are adapter coordinates; pin the
        // target slot identity before asking mpv to move. Idle jumps
        // settle on PlaybackRestart because no outgoing EndFile exists.
        let from_idle = !self.status.lock().unwrap().active;
        if from_idle {
            self.tracks_initialized = false;
        }
        let transition = self.forced_jump.and_then(|jump| jump.transition);
        self.forced_jump = Some(super::super::ForcedJump {
            slot_id,
            transition,
            resume_ticks,
            from_idle,
        });
        if let Err(e) = mpv.set_property("playlist-pos", i64::try_from(idx).unwrap_or(i64::MAX)) {
            self.forced_jump = None;
            tracing::warn!(name: "player.jump.failed", target: "player", index = idx, error = %mpv_err_str(&e), "jump failed");
            return;
        }
        tracing::info!(name: "player.jump.succeeded", target: "transition", slot = ?slot_id, index = idx, current_index = self.current_idx, queue_length = self.queue_len(), "playlist position updated");
        if from_idle {
            Self::play_from_idle_playlist(idx, mpv);
        }
        // Selecting a track should always start it playing, even if
        // mpv was paused on the previous track — otherwise the new
        // track loads silently "stuck" paused (see issue: Enter on a
        // queue item, or a remote Next/Previous command, while paused).
        let _ = mpv.set_property("pause", false);
    }

    fn play_from_idle_playlist(idx: usize, mpv: &Mpv) {
        if let Err(error) = mpv.command("playlist-play-index", &[&idx.to_string()]) {
            tracing::warn!(name: "player.jump.play_from_idle_failed", target: "player", index = idx, error = %mpv_err_str(&error), "failed to play playlist index from idle");
        }
    }

    pub(crate) fn append_items_to_queue(&mut self, items: Vec<ExecSlot>) {
        for slot in items {
            self.queue.append_with_id(slot.slot_id, slot.item);
        }
        self.status.lock().unwrap().queue_len = self.queue_len();
    }

    pub(super) fn cmd_append_queue(&mut self, new_items: Vec<ExecSlot>, mpv: &Mpv) {
        if new_items.is_empty() {
            return;
        }

        if self.active_file {
            self.append_items_to_queue(new_items);
            return;
        }
        if new_items.iter().any(|slot| slot.item.is_audiobookshelf()) {
            let Some(active_item) = self.active_item().cloned() else {
                return;
            };
            let prepared = match self.prepare_item(&active_item) {
                Ok(prepared) => prepared,
                Err(error) => {
                    tracing::warn!(name: "player.active_file.transition_failed", target: "player", error = %error, "active-file transition failed");
                    return;
                }
            };
            if let Err(error) = self.install_active_projection(mpv, prepared, &active_item) {
                tracing::warn!(name: "player.active_file.transition_failed", target: "player", error = %error, "active-file transition failed");
                return;
            }
            self.append_items_to_queue(new_items);
            self.active_file = true;
            return;
        }
        // Design D1: every Emby video start position in the appended set is
        // overwritten with Emby's current value before its `start=` option is
        // baked. ponytail: a natural mpv advance still uses this load-time
        // value; the upgrade path is fetch-and-seek on entry activation.
        let slot_ids: Vec<QueueSlotId> = new_items.iter().map(|slot| slot.slot_id).collect();
        let items = refresh_emby_resume(
            new_items.into_iter().map(|slot| slot.item).collect(),
            |ids| self.reporter.client.get_items_by_ids(ids),
            &EMBY_RESUME_RETRY_DELAYS,
        );
        let Some(sources) = items
            .iter()
            .map(QueueItem::mpv_url_source)
            .collect::<Option<Vec<_>>>()
        else {
            let reason = "Queue append rejected: item has no direct mpv URL source".to_string();
            tracing::warn!(name: "player.queue_append.rejected", target: "player", reason = %reason, "queue append rejected");
            let _ = self.event_tx.send(PlayerEvent::CommandRejected(reason));
            return;
        };
        for (item, source) in items.iter().zip(sources) {
            let url = mpv_url_for_queue_item(source, &self.server_url, &self.token);
            let opts = mpv_load_opts(item);
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "append-play", "-1", opts.as_str()],
            ) {
                tracing::warn!(name: "player.queue_append.load_failed", target: "player", error = %mpv_err_str(&e), "mpv loadfile failed while appending");
            }
        }

        self.append_items_to_queue(
            slot_ids
                .into_iter()
                .zip(items)
                .map(|(slot_id, item)| ExecSlot { slot_id, item })
                .collect(),
        );
    }
}
