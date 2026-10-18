use super::{Mpv, PlaybackRun, QueueSlotId};
use crate::shift_index_for_move;

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

    pub(crate) fn cmd_queue_move(&mut self, slot_id: QueueSlotId, to: usize, mpv: &Mpv) {
        let Some(from) = self.queue.slot_index(slot_id) else {
            return;
        };
        if from >= self.queue_len() || to >= self.queue_len() || from == to {
            return;
        }
        if !self.active_file {
            let (mpv_from, mpv_to) = playlist_move_args(from, to);
            let _ = mpv.command(
                "playlist-move",
                &[&mpv_from.to_string(), &mpv_to.to_string()],
            );
        }
        self.apply_queue_move(slot_id, from, to);
    }

    /// The bookkeeping one queue move applies to the run's queue mirror and
    /// its tracked mpv index. Split from [`Self::cmd_queue_move`] so the
    /// Shuffle reorder (queue-content-shuffle task 3.3) can reuse the seams
    /// inside its one handler; the mpv playlist command itself is pinned by
    /// the `playlist_move_args` test.
    pub(crate) fn apply_queue_move(&mut self, slot_id: QueueSlotId, from: usize, to: usize) {
        let _ = self.queue.move_slot(slot_id, to);
        // `current_idx` is this run's mpv-local coordinate; adjust it
        // for the move directly (design D2) rather than recomputing
        // it from the observed active slot.
        self.current_idx = shift_index_for_move(self.current_idx, from, to);
        self.sync_status_position();
    }

    /// `PlayerCommand::QueueReorder`: apply the owner's full new order inside
    /// one command handler (queue-content-shuffle task 3.3). The run drains
    /// its command channel per tick, so a move list leaves a window where an
    /// mpv `EndFile` is processed against a partially applied order; one
    /// command closes it. The playlist surgery reuses the move seams: the
    /// incremental plan from `reorder_move_plan` drives `playlist_move_args`
    /// and `apply_queue_move`, and the active slot moves first, so
    /// `current_idx` ends at 0. An order that does not name exactly this
    /// run's slots (a stale reorder) is discarded untouched.
    pub(crate) fn cmd_queue_reorder(&mut self, order: &[QueueSlotId], mpv: &Mpv) {
        let before: Vec<QueueSlotId> = (0..self.queue_len())
            .filter_map(|idx| self.slot_id_at(idx))
            .collect();
        if !is_slot_permutation(&before, order) {
            tracing::warn!(name: "player.queue_reorder.stale", target: "player", held = before.len(), requested = order.len(), "queue reorder discarded: the order does not name this run's slots");
            return;
        }
        for (slot_id, to) in mbv_queue::reorder_move_plan(&before, order) {
            let Some(from) = self.queue.slot_index(slot_id) else {
                continue;
            };
            if !self.active_file {
                let (mpv_from, mpv_to) = playlist_move_args(from, to);
                let _ = mpv.command(
                    "playlist-move",
                    &[&mpv_from.to_string(), &mpv_to.to_string()],
                );
            }
            self.apply_queue_move(slot_id, from, to);
        }
    }
}

/// True when `order` names exactly the slots in `current`, each once.
fn is_slot_permutation(current: &[QueueSlotId], order: &[QueueSlotId]) -> bool {
    if current.len() != order.len() {
        return false;
    }
    let mut current = current.to_vec();
    let mut order = order.to_vec();
    current.sort_unstable();
    order.sort_unstable();
    current == order
}

/// mpv's `playlist-move` arguments for moving the entry at `from` to `to`.
///
/// mpv's index2 names the *pre-move* slot the entry should end up next to,
/// not its post-move index: for `from < to` the entry actually lands at
/// `to - 1`, not `to` (mpv manual's own "paradox" note, confirmed against
/// mpv 0.41). Passing `to + 1` (one past the end when `to == n - 1`, which
/// mpv also accepts as "move to end") makes mpv's result match the run's
/// from/to bookkeeping in [`PlaybackRun::apply_queue_move`].
pub(crate) fn playlist_move_args(from: usize, to: usize) -> (usize, usize) {
    (from, if from < to { to + 1 } else { to })
}
