use crate::app::dispatch::notify::ToastSeverity;
use crate::app::infra::ui_util::is_playable;
use crate::app::state::queue_owner::LocalQueueOwner;
use crate::app::state::types::playback::PlaylistMutation;
use crate::app::{
    App, ConfirmAction, ConfirmModal, LibEvent, PanelFocus, PendingQueueAction, QueueScope,
    ReplacementExecutor, RoutedReplacementPrep, SessionEvent, SidebarId, UndoEntry,
};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use mbv_core::player::PlayerCommand;

mod playlist;
mod playlist_mutation;
mod replacement;

impl App {
    pub(in crate::app) fn remove_from_queue(&mut self, pos: usize) {
        let scope = self.viewed_queue_scope();
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let active = self.player.status.lock().unwrap().active;
        if pos >= self.queue_for_scope(scope).total_queue_len() {
            let queue = self.queue_for_scope_mut(scope);
            queue.clamp_cursor();
            return;
        }
        if self.queue_pos_needs_active_confirm(scope, pos) {
            self.ask_confirm(ConfirmModal {
                title: " Remove Item ".into(),
                message: "Remove now-playing item and stop playback?".into(),
                hint: "[y] Confirm    [Esc] Cancel".into(),
                on_confirm: ConfirmAction::RemoveActiveQueueItem(pos),
            });
            return;
        }
        let cursor_before = self.queue_for_scope(scope).queue_cursor;
        // Capture slot identity before removal so we can issue a
        // slot-based command for unified-capable remote peers.
        let slot_id = self.queue_for_scope(scope).slot_id_at(pos);
        let Some(item) = self.queue_for_scope_mut(scope).remove_slot_at(pos) else {
            return;
        };
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_scope_mut(scope)
            .push(UndoEntry::Remove(pos, item));
        self.persist_local_queue_state_if_needed(scope);
        let sent_queue_remove =
            controls_playback_queue && self.queue_edit_reaches_player(scope, active);
        if sent_queue_remove {
            // Slot identity was captured before the local removal above.
            // Prefer the unified remote path; the in-process command is now
            // slot-addressed too.
            if let Some(sid) = slot_id {
                if !self
                    .player
                    .queue_remove_slot(mbv_core::ctrl::slot_id_to_u64(sid))
                {
                    self.player.send_command(PlayerCommand::QueueRemove(sid));
                }
            }
            // Player thread adjusts current_idx when it processes the command.
            // No eager adjustment here — doing so races with the player thread
            // and can cause index mismatches during rapid removals.
        }
        let queue = self.queue_for_scope_mut(scope);
        if pos < cursor_before {
            // Set directly from the pre-removal cursor rather than decrementing
            // whatever `remove_slot_at`'s internal clamp left behind: that clamp
            // only enforces bounds (min(cursor, len-1)), and when `cursor_before`
            // was the last item, its bounds-clamp already shifts it down by one,
            // so decrementing on top of it would double-shift.
            queue.queue_cursor = cursor_before - 1;
        }
        queue.clamp_cursor();
        if sent_queue_remove {
            // The daemon's reply carries its own cursor, which tracks
            // *playback* position, not this selection — without this, the
            // round trip would snap the display cursor onto the now-playing
            // item instead of leaving it on the item that shifted into this
            // slot. See `PlayerEvent::UnifiedQueueUpdated`.
            self.pending_queue_edit_cursor = Some(queue.queue_cursor);
        }
        self.advance_queue_epoch();
    }

    /// Whether removing the slot at `pos` from `scope`'s playback queue needs
    /// the dedicated now-playing confirmation: the slot is the actively
    /// playing item, or the attached session reports it as the remote
    /// now-playing item. Non-playback scopes never need it.
    fn queue_pos_needs_active_confirm(&self, scope: QueueScope, pos: usize) -> bool {
        if !self.queue_scope_is_playback(scope) {
            return false;
        }
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if active {
            return current_idx == pos;
        }
        self.connected_session_state
            .as_ref()
            .and_then(|s| s.now_playing_item_id.as_ref())
            .zip(self.queue_for_scope(scope).emby_item_at(pos))
            .is_some_and(|(npid, item)| item.id == *npid)
    }

    /// Remove the slots named by `slot_ids` from `scope`'s queue as one edit.
    ///
    /// Every slot is removed locally before the playback owner is told
    /// anything, the owner receives one batch command, and the queue snapshot
    /// is persisted once. The cursor comes to rest on the item that preceded
    /// the deleted range; when the range started the queue there is nothing
    /// before it, so the first item that followed takes the cursor instead.
    ///
    /// A selection that includes the now-playing row keeps the dedicated
    /// per-row confirmation flow, because stopping playback is a decision the
    /// user has to answer, not a plain edit.
    pub(in crate::app) fn remove_slots_from_queue(
        &mut self,
        scope: QueueScope,
        slot_ids: &[mbv_core::playback_queue::QueueSlotId],
    ) {
        let needs_confirm = slot_ids.iter().any(|slot_id| {
            self.slot_index(scope, *slot_id)
                .is_some_and(|pos| self.queue_pos_needs_active_confirm(scope, pos))
        });
        if needs_confirm {
            for slot_id in slot_ids {
                if let Some(pos) = self.slot_index(scope, *slot_id) {
                    self.remove_from_queue(pos);
                }
            }
            return;
        }

        let (active, _) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        let mut positions: Vec<usize> = slot_ids
            .iter()
            .filter_map(|slot_id| self.slot_index(scope, *slot_id))
            .collect();
        positions.sort_unstable();
        positions.dedup();
        if positions.is_empty() {
            return;
        }
        // The cursor rests on the item that preceded the range. When the range
        // started the queue there is nothing before it, so it rests on the
        // first item that followed — which, after the removal, sits at index 0.
        // Only slots at or after `positions[0]` are removed, so the preceding
        // item keeps its index.
        let cursor_after = positions[0].saturating_sub(1);

        // Descending order keeps the remaining positions valid as slots go;
        // the recorded undo positions are the pre-removal indices.
        let mut removed_slots: Vec<mbv_core::playback_queue::QueueSlotId> = Vec::new();
        for pos in positions.iter().rev() {
            let Some(slot_id) = self.queue_for_scope(scope).slot_id_at(*pos) else {
                continue;
            };
            let Some(item) = self.queue_for_scope_mut(scope).remove_slot_at(*pos) else {
                continue;
            };
            self.undo_stack_for_scope_mut(scope)
                .push(UndoEntry::Remove(*pos, item));
            removed_slots.push(slot_id);
        }
        if removed_slots.is_empty() {
            return;
        }
        // Hand the owner the range in its original queue order.
        removed_slots.reverse();
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        {
            let queue = self.queue_for_scope_mut(scope);
            queue.queue_cursor = cursor_after;
            queue.clamp_cursor();
        };
        // The mounted component owns the painted cursor and only adopts
        // `App`'s value when a re-anchor is armed; without this it would keep
        // the row the deleted range used to occupy and clamp there.
        self.pending_queue_cursor_reanchor = Some(scope);
        self.persist_local_queue_state_if_needed(scope);

        let sent_queue_remove =
            self.queue_scope_is_playback(scope) && self.queue_edit_reaches_player(scope, active);
        if sent_queue_remove {
            let raw_slot_ids: Vec<u64> = removed_slots
                .iter()
                .map(|slot_id| mbv_core::ctrl::slot_id_to_u64(*slot_id))
                .collect();
            // A remote owner takes the whole range in one edit. A local owner
            // has no batch command and keeps one player command per slot; its
            // internal queue edits are never published, so they stay atomic
            // on screen either way.
            if !self.player.queue_remove_slots(raw_slot_ids) {
                for slot_id in &removed_slots {
                    self.player
                        .send_command(PlayerCommand::QueueRemove(*slot_id));
                }
            }
            self.pending_queue_edit_cursor = Some(self.queue_for_scope(scope).queue_cursor);
        }
        self.advance_queue_epoch();
    }

    /// Moves the item at `from` one position earlier. No-op at the start of
    /// the queue.
    pub(in crate::app) fn move_queue_item_up(&mut self, from: usize) {
        self.move_queue_item_by(from, -1);
    }

    /// Moves the item at `from` one position later. No-op at the end of the
    /// queue.
    pub(in crate::app) fn move_queue_item_down(&mut self, from: usize) {
        self.move_queue_item_by(from, 1);
    }

    /// Moves the item at `from` by `delta` within the displayed queue's
    /// scope. The target is passed explicitly (split-queue-cursor-ownership
    /// D2): the shell resolves the slot the user selected and hands it over
    /// rather than this function re-reading `queue.queue_cursor` as an
    /// ambient argument channel.
    fn move_queue_item_by(&mut self, from: usize, delta: isize) {
        let scope = self.viewed_queue_scope();
        let queue = self.queue_for_scope(scope);
        let len = queue.total_queue_len();
        let to = if delta < 0 {
            match from.checked_sub(1) {
                Some(t) => t,
                None => return,
            }
        } else {
            let t = from + 1;
            if t >= len {
                return;
            }
            t
        };
        self.move_queue_item_to(scope, from, to);
    }

    /// Moves the item at `from` to `to` within `scope`'s queue and records the
    /// undo entry. `scope` is passed explicitly (D2) rather than re-read from
    /// `viewed_queue_scope()` as an ambient channel.
    pub(in crate::app) fn move_queue_item_to(&mut self, scope: QueueScope, from: usize, to: usize) {
        let Some(slot_id) = self.queue_for_scope_mut(scope).slot_id_at(from) else {
            return;
        };
        if self.apply_queue_move_by_slot(scope, slot_id, from, to) {
            self.advance_queue_epoch();
            if scope == QueueScope::Remote {
                self.pending_remote_move_cursor = Some(to);
            }
            self.undo_stack_for_scope_mut(scope)
                .push(UndoEntry::Move { from, to, slot_id });
        }
    }

    /// Swaps the item at `from` to `to` within `scope`'s queue, moves the
    /// cursor to follow it, and — if this queue is also the live playback
    /// queue — tells the player to make the same move in its own internal
    /// queue copy (mirroring how active-playback removals keep that copy in
    /// sync). Returns
    /// `false` (no-op) if `from`/`to` are out of bounds or equal.
    pub(in crate::app) fn apply_queue_move(
        &mut self,
        scope: QueueScope,
        from: usize,
        to: usize,
    ) -> bool {
        let Some(slot_id) = self.queue_for_scope_mut(scope).slot_id_at(from) else {
            return false;
        };
        self.apply_queue_move_by_slot(scope, slot_id, from, to)
    }

    fn apply_queue_move_by_slot(
        &mut self,
        scope: QueueScope,
        slot_id: mbv_core::playback_queue::QueueSlotId,
        from: usize,
        to: usize,
    ) -> bool {
        let len = self.queue_for_scope(scope).total_queue_len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let active = self.player.status.lock().unwrap().active;
        if !self.queue_for_scope_mut(scope).move_slot(slot_id, to) {
            return false;
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && self.queue_edit_reaches_player(scope, active) {
            // Prefer the unified remote path; the in-process command is now
            // slot-addressed (source) with an ordinal destination.
            let sent_unified = self
                .player
                .queue_move_slot(mbv_core::ctrl::slot_id_to_u64(slot_id), to);
            if !sent_unified {
                self.player
                    .send_command(PlayerCommand::QueueMove(slot_id, to));
            }
        }
        true
    }

    /// Pops and reverses the most recent undoable edit in `scope`'s queue —
    /// re-inserting a removed item, or swapping a moved item back to where it
    /// came from. No-op if the undo stack for that scope is empty.
    pub(in crate::app) fn undo_last_queue_edit(&mut self, scope: QueueScope) {
        let Some(entry) = self.undo_stack_for_scope_mut(scope).pop() else {
            return;
        };
        match entry {
            UndoEntry::Remove(idx, item) => {
                let queue = self.queue_for_scope_mut(scope);
                let idx = idx.min(queue.total_queue_len());
                queue.insert_item_at(idx, item);
                if self.local_queue_metadata_applies(scope) {
                    self.queue_dirty = true;
                }
                self.persist_local_queue_state_if_needed(scope);
                self.advance_queue_epoch();
            }
            UndoEntry::Move { from, to, slot_id } => {
                let still_in_place = self.queue_for_scope(scope).slot_id_matches_at(to, slot_id);
                if !still_in_place || !self.apply_queue_move(scope, to, from) {
                    self.flash(
                        "Can't undo move: queue changed since then".into(),
                        ToastSeverity::Error,
                    );
                    return;
                }
                self.advance_queue_epoch();
            }
        }
        self.set_queue_scope(scope);
    }
}
