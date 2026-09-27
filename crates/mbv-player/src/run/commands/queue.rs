use super::{
    mpv_err_str, mpv_load_opts, mpv_url_for_queue_item, reject_stale_jump, resolve_jump_target,
    ExecSlot, Mpv, PlaybackRun, PlayerEvent, QueueSlotId,
};

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
    ) {
        let slot_ids = self
            .queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>();
        let Some(idx) = resolve_jump_target(&slot_ids, slot_id) else {
            log::info!(
                target: "transition",
                "jump-to reject_stale_jump: slot_id={slot_id:?} unresolvable",
            );
            reject_stale_jump(&self.event_tx, slot_id);
            return;
        };
        let transition = crate::transition::Transition::new(request_id, generation, slot_id);
        self.forced_jump = Some(super::super::ForcedJump {
            slot_id,
            transition: Some(transition),
            resume_ticks: if self.active_file { None } else { resume_ticks },
            from_idle: false,
        });
        if self.active_file {
            self.cmd_jump_to_active_file(slot_id, resume_ticks, mpv);
        } else {
            self.cmd_jump_to_playlist(slot_id, idx, resume_ticks, mpv);
        }
    }

    fn cmd_jump_to_active_file(
        &mut self,
        slot_id: QueueSlotId,
        resume_ticks: Option<i64>,
        mpv: &Mpv,
    ) {
        match self.select_active_slot_with_resume(slot_id, resume_ticks, mpv) {
            Ok(()) => {
                let _ = mpv.set_property("pause", false);
                // Active-file projection has no mpv playlist move to
                // observe, so the JumpTo emits its TrackChanged
                // observation here, shaped like the on_end_file settle
                // (design D1). The tag stays on the forced jump so
                // a duplicate settle path still carries it; the
                // pipeline treats the second attempt as Ignored.
                self.emit_track_changed(slot_id, self.forced_jump.and_then(|jump| jump.transition));
            }
            Err(error) => {
                log::warn!(target: "player", "active-file selection failed: {error}");
            }
        }
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
            log::warn!(target: "player", "jump-to idx={idx} failed: {}", mpv_err_str(&e));
            return;
        }
        log::info!(
            target: "transition",
            "jump-to: playlist-pos ok slot_id={:?} idx={} current_idx={} queue_len={}",
            slot_id,
            idx,
            self.current_idx,
            self.queue_len(),
        );
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
            log::warn!(target: "player", "jump-to playlist-play-index={idx} failed: {}", mpv_err_str(&error));
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
                    log::warn!(target: "player", "active-file transition failed: {error}");
                    return;
                }
            };
            if let Err(error) = self.install_active_projection(mpv, prepared, &active_item) {
                log::warn!(target: "player", "active-file transition failed: {error}");
                return;
            }
            self.append_items_to_queue(new_items);
            self.active_file = true;
            return;
        }
        let Some(sources) = new_items
            .iter()
            .map(|slot| slot.item.mpv_url_source())
            .collect::<Option<Vec<_>>>()
        else {
            let reason = "Queue append rejected: item has no direct mpv URL source".to_string();
            log::warn!(target: "player", "{reason}");
            let _ = self.event_tx.send(PlayerEvent::CommandRejected(reason));
            return;
        };
        for (slot, source) in new_items.iter().zip(sources) {
            let url = mpv_url_for_queue_item(source, &self.server_url, &self.token);
            let opts = mpv_load_opts(&slot.item);
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "append-play", "-1", opts.as_str()],
            ) {
                log::warn!(target: "player", "QueueAppend loadfile error: {}", mpv_err_str(&e));
            }
        }

        self.append_items_to_queue(new_items);
    }
}
