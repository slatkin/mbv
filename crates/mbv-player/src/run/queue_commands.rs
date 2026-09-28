use super::{PlaybackRun, QueueSlotId};
use crate::shift_index_for_move;
use libmpv2::Mpv;

impl PlaybackRun {
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
        if self.forced_jump.is_some_and(|jump| jump.slot_id == slot_id) {
            self.forced_jump = None;
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
}
