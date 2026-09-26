use super::{
    mpv_err_str, mpv_load_opts, mpv_url_for_queue_item, reject_stale_jump, resolve_jump_target,
    shift_index_for_move, ExecSlot, Mpv, PlaybackRun, PlayerEvent, QueueSlotId,
};

impl PlaybackRun {
    /// Explicit jump to an owner-assigned slot. Resolves the slot to this
    /// run's mpv-local ordinal; a stale slot (gone here) is rejected, never
    /// repaired by position (design D6).
    pub(super) fn cmd_jump_to(
        &mut self,
        slot_id: QueueSlotId,
        request_id: crate::ctrl::PlaybackRequestId,
        generation: crate::ctrl::PlaybackGeneration,
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
        self.forced_transition = Some(crate::playback_transition::Transition::new(
            request_id, generation, slot_id,
        ));
        if self.active_file {
            self.cmd_jump_to_active_file(slot_id, mpv);
        } else {
            self.cmd_jump_to_playlist(slot_id, idx, resume_ticks, mpv);
        }
    }

    fn cmd_jump_to_active_file(&mut self, slot_id: QueueSlotId, mpv: &Mpv) {
        match self.select_active_slot(slot_id, mpv) {
            Ok(()) => {
                let _ = mpv.set_property("pause", false);
                // Active-file projection has no mpv playlist move to
                // observe, so the JumpTo emits its TrackChanged
                // observation here, shaped like the on_end_file settle
                // (design D1). The tag stays on `forced_transition` so
                // a duplicate settle path still carries it; the
                // pipeline treats the second attempt as Ignored.
                self.emit_track_changed(slot_id, self.forced_transition);
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
        self.forced_jump_from_idle = !self.status.lock().unwrap().active;
        if self.forced_jump_from_idle {
            self.tracks_initialized = false;
        }
        self.forced_slot_id = Some(slot_id);
        self.forced_resume_ticks = resume_ticks;
        if let Err(e) = mpv.set_property("playlist-pos", i64::try_from(idx).unwrap_or(i64::MAX)) {
            self.forced_slot_id = None;
            self.forced_jump_from_idle = false;
            self.forced_transition = None;
            self.forced_resume_ticks = None;
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
        if self.forced_jump_from_idle {
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

    /// Remove an owner-assigned slot; a stale slot (already gone here) is
    /// discarded.
    pub(super) fn cmd_queue_remove(&mut self, slot_id: QueueSlotId, mpv: &Mpv) {
        let Some(idx) = self.queue.slot_index(slot_id) else {
            return;
        };
        let active_slot_id = self.active_slot_id();
        if self.active_file {
            self.remove_active_file_slot(slot_id, idx, active_slot_id, mpv);
            return;
        }
        let _ = mpv.command("playlist-remove", &[&idx.to_string()]);
        if active_slot_id == Some(slot_id) {
            self.close_prepared_source();
            self.queue.remove_active_slot_confirmed(slot_id);
        } else {
            self.queue.remove_slot(slot_id);
        }
        self.sync_status_position();
        if self.forced_slot_id == Some(slot_id) {
            self.forced_slot_id = None;
            self.forced_transition = None;
            self.forced_resume_ticks = None;
        }
        if active_slot_id != Some(slot_id) {
            return;
        }
        // Currently playing track removed — clear reporter item_id to prevent
        // stale progress reports until on_end_file transitions to the next track.
        self.reporter.ids.lock().unwrap().0.clear();
    }

    fn remove_active_file_slot(
        &mut self,
        slot_id: QueueSlotId,
        idx: usize,
        active_slot_id: Option<QueueSlotId>,
        mpv: &Mpv,
    ) {
        if active_slot_id != Some(slot_id) {
            self.queue.remove_slot(slot_id);
            // active_file mode gets no mpv playlist-pos event to
            // self-correct the coordinate, so re-derive the
            // ordinal from the still-active slot's identity
            // (identity -> ordinal is permitted, design D2).
            self.current_idx = self
                .active_slot_id()
                .and_then(|s| self.queue.slot_index(s))
                .unwrap_or(self.current_idx);
            self.sync_status_position();
            return;
        }
        let Some(next) = self.remove_neighbor_slot(idx) else {
            self.close_prepared_source();
            self.queue.remove_active_slot_confirmed(slot_id);
            let _ = mpv.command("playlist-clear", &[]);
            self.sync_status_position();
            return;
        };
        if self.select_active_slot(next, mpv).is_ok() {
            self.queue.remove_slot(slot_id);
        }
        self.sync_status_position();
    }

    fn remove_neighbor_slot(&self, idx: usize) -> Option<QueueSlotId> {
        if idx + 1 < self.queue_len() {
            return self.slot_id_at(idx + 1);
        }
        if idx > 0 {
            return self.slot_id_at(idx - 1);
        }
        None
    }

    pub(super) fn cmd_queue_move(&mut self, slot_id: QueueSlotId, to: usize, mpv: &Mpv) {
        let Some(from) = self.queue.slot_index(slot_id) else {
            return;
        };
        if from >= self.queue_len() || to >= self.queue_len() || from == to {
            return;
        }
        // mpv's playlist-move index2 names the *pre-move* slot the
        // entry should end up next to, not its post-move index: for
        // from < to the entry actually lands at to - 1, not to (mpv
        // manual's own "paradox" note, confirmed against mpv 0.41).
        // Passing to + 1 (one past the end when to == n - 1, which
        // mpv also accepts as "move to end") makes mpv's result
        // match this struct's from/to bookkeeping below.
        if !self.active_file {
            let mpv_to = if from < to { to + 1 } else { to };
            let _ = mpv.command("playlist-move", &[&from.to_string(), &mpv_to.to_string()]);
        }
        let _ = self.queue.move_slot(slot_id, to);
        // `current_idx` is this run's mpv-local coordinate; adjust it
        // for the move directly (design D2) rather than recomputing
        // it from the observed active slot.
        self.current_idx = shift_index_for_move(self.current_idx, from, to);
        self.sync_status_position();
    }

    /// Relative single-step nav (`PlayerCommand::Next`/`Previous`) for an
    /// already-bounds-checked target ordinal. Mirrors the `JumpTo` move minus
    /// request identity: no `forced_transition` (design D4 — relative nav
    /// correlates like natural advancement). Still pins `forced_slot_id` so the
    /// resulting mpv observation is attributed to the right slot.
    pub(super) fn step_to_index(&mut self, idx: usize, mpv: &Mpv) {
        let Some(slot_id) = self.slot_id_at(idx) else {
            return;
        };
        if self.active_file {
            if let Err(error) = self.select_active_slot(slot_id, mpv) {
                log::warn!(target: "player", "active-file step to idx={idx} failed: {error}");
            } else {
                let _ = mpv.set_property("pause", false);
            }
            return;
        }
        // Unlike JumpTo (which gets the target's resume position from the
        // owner's canonical queue via the command itself), relative nav is
        // resolved entirely locally — this run's own queue mirror is now kept
        // current for exactly this (see `on_end_file`'s `apply_progress`
        // call), so the same per-kind gate can be evaluated straight off it.
        let resume_ticks = self
            .queue
            .slot(slot_id)
            .and_then(|slot| crate::player::resume_ticks_for_item(&slot.item));
        self.forced_slot_id = Some(slot_id);
        self.forced_resume_ticks = resume_ticks;
        if let Err(e) = mpv.set_property("playlist-pos", i64::try_from(idx).unwrap_or(i64::MAX)) {
            self.forced_slot_id = None;
            self.forced_resume_ticks = None;
            log::warn!(target: "player", "step to idx={idx} failed: {}", mpv_err_str(&e));
        } else {
            let _ = mpv.set_property("pause", false);
        }
    }

    pub(in crate::player) fn append_items_to_queue(&mut self, items: Vec<ExecSlot>) {
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
