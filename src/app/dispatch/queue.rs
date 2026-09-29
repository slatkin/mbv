use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::playback::PlaylistMutation;
use crate::app::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, QueueScope,
    ReplacementExecutor, RoutedReplacementPrep, SessionEvent, UndoEntry,
};
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use mbv_ui_model::ui_util::is_playable;

pub(in crate::app) use self::queue_op::QueueOpEdit;

mod pending_playback;
mod playlist;
mod playlist_mutation;
mod queue_op;
mod replacement;

impl App {
    pub(in crate::app) fn remove_from_queue(&mut self, pos: usize) {
        let scope = self.viewed_queue_scope();
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
        // Slot identity and the undo payload are resolved against the
        // owner's last accepted state before the edit is sent: the Client
        // holds no editable queue, so the view changes only when the
        // owner's answer is adopted.
        let cursor_before = self.queue_for_scope(scope).queue_cursor;
        let Some(slot_id) = self.queue_for_scope(scope).slot_id_at(pos) else {
            return;
        };
        let Some(item) = self.queue_for_scope(scope).item_at(pos).cloned() else {
            return;
        };
        let sent = self.queue_op(
            scope,
            mbv_remote_player::QueueOp::RemoveSlot {
                slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
            },
        );
        match sent {
            QueueOpEdit::Applied => {
                // The selected slot is gone; the selection moves to the
                // entry now at the removed entry's former position — the
                // follower when one followed it, the selection itself when
                // it sat below the removed row.
                let len = self.queue_for_scope(scope).total_queue_len();
                let cursor = if pos < cursor_before {
                    cursor_before - 1
                } else {
                    cursor_before
                };
                self.queue_for_scope_mut(scope).queue_cursor = cursor.min(len.saturating_sub(1));
                self.record_undoable_queue_edit(scope, UndoEntry::Remove { item, index: pos });
            }
            QueueOpEdit::SentLegacy => {
                // The edit was delivered; the displayed queue follows the
                // legacy owner's later snapshots.
                self.record_undoable_queue_edit(scope, UndoEntry::Remove { item, index: pos });
            }
            QueueOpEdit::NotApplied => {}
        }
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

    /// Remove the slots named by `slot_ids` from `scope`'s queue as one
    /// owner edit (row 5.2): the owner applies the whole range before
    /// publishing the queue snapshot the Client adopts, so the list never
    /// shrinks one row per removal.
    ///
    /// The cursor comes to rest on the item that preceded the deleted range;
    /// when the range started the queue there is nothing before it, so the
    /// first item that followed takes the cursor instead. A selection that
    /// includes the now-playing row keeps the dedicated per-row confirmation
    /// flow, because stopping playback is a decision the user has to answer,
    /// not a plain edit.
    pub(in crate::app) fn remove_slots_from_queue(
        &mut self,
        scope: QueueScope,
        slot_ids: &[mbv_queue::QueueSlotId],
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

        let mut targets: Vec<(mbv_queue::QueueSlotId, usize)> = slot_ids
            .iter()
            .filter_map(|slot_id| self.slot_index(scope, *slot_id).map(|pos| (*slot_id, pos)))
            .collect();
        targets.sort_unstable_by_key(|(_, pos)| *pos);
        targets.dedup_by_key(|(_, pos)| *pos);
        if targets.is_empty() {
            return;
        }
        // The cursor rests on the item that preceded the range. When the
        // range started the queue there is nothing before it, so it rests on
        // the first item that followed — which, after the removal, sits at
        // index 0. Only slots at or after the range are removed, so the
        // preceding item keeps its index.
        let cursor_after = targets[0].1.saturating_sub(1);
        // The undo payload is resolved against the owner's last accepted
        // state before the edit is sent: the view changes only when the
        // owner's answer is adopted.
        let removed: Vec<(usize, QueueItem)> = targets
            .iter()
            .filter_map(|(_, pos)| {
                let item = self.queue_for_scope(scope).item_at(*pos)?.clone();
                Some((*pos, item))
            })
            .collect();
        let raw_slot_ids: Vec<u64> = targets
            .iter()
            .map(|(slot_id, _)| mbv_ctrl::slot_id_to_u64(*slot_id))
            .collect();
        let sent = self.queue_op(
            scope,
            mbv_remote_player::QueueOp::RemoveSlots {
                slot_ids: raw_slot_ids,
            },
        );
        if sent == QueueOpEdit::NotApplied {
            return;
        }
        // Undo entries pop in ascending index order, so each re-append
        // resolves its anchor against the state the previous undo produced.
        {
            let stack = self.undo_stack_for_scope_mut(scope);
            for (pos, item) in removed.iter().rev() {
                stack.push(UndoEntry::Remove {
                    item: item.clone(),
                    index: *pos,
                });
            }
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        if sent == QueueOpEdit::Applied {
            let len = self.queue_for_scope(scope).total_queue_len();
            self.queue_for_scope_mut(scope).queue_cursor = cursor_after.min(len.saturating_sub(1));
            // The mounted component owns the painted cursor and only adopts
            // `App`'s value when a re-anchor is armed; the adopted snapshot
            // moved the cursor onto the item preceding the range.
            self.pending_queue_cursor_reanchor = Some(scope);
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

    /// Moves the item at `from` to `to` within `scope`'s queue by asking the
    /// scope's Player owner to move the slot (row 5.2). `scope` is passed
    /// explicitly (D2) rather than re-read from `viewed_queue_scope()` as an
    /// ambient channel.
    pub(in crate::app) fn move_queue_item_to(&mut self, scope: QueueScope, from: usize, to: usize) {
        let len = self.queue_for_scope(scope).total_queue_len();
        if from >= len || to >= len || from == to {
            return;
        }
        let Some(slot_id) = self.queue_for_scope(scope).slot_id_at(from) else {
            return;
        };
        if scope == QueueScope::Remote {
            // Consumed by the answered adoption inside `send_queue_edit`.
            self.pending_remote_move_cursor = Some(to);
        }
        let sent = self.queue_op(
            scope,
            mbv_remote_player::QueueOp::MoveSlot {
                slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
                to_index: to,
            },
        );
        match sent {
            QueueOpEdit::Applied => {
                if scope != QueueScope::Remote {
                    // The selection stays on the moved entry at its new
                    // position.
                    let len = self.queue_for_scope(scope).total_queue_len();
                    self.queue_for_scope_mut(scope).queue_cursor = to.min(len.saturating_sub(1));
                }
                self.record_undoable_queue_edit(scope, UndoEntry::Move { slot_id, from });
            }
            QueueOpEdit::SentLegacy => {
                self.record_undoable_queue_edit(scope, UndoEntry::Move { slot_id, from });
            }
            QueueOpEdit::NotApplied => {
                if scope == QueueScope::Remote {
                    // No snapshot adopted the armed push, so it must not
                    // fire on a later background snapshot.
                    self.pending_remote_move_cursor = None;
                }
            }
        }
    }

    /// Pops and reverses the most recent undoable edit in `scope`'s queue by
    /// sending the inverse edit to the scope's Player owner (design D7):
    /// a removed item is re-appended before the entry now at its former
    /// position, and a moved slot is moved back to where it came from. The
    /// owner rejects an undo whose slot is gone as stale slot addressing; the
    /// pump flashes that rejection. No-op if the undo stack for that scope is
    /// empty.
    pub(in crate::app) fn undo_last_queue_edit(&mut self, scope: QueueScope) {
        let Some(entry) = self.undo_stack_for_scope_mut(scope).pop() else {
            return;
        };
        match entry {
            UndoEntry::Remove { item, index } => {
                // Resolve the anchor at undo time: the slot now at the
                // removed item's former position, or the end when that
                // position no longer exists.
                let before = self
                    .queue_for_scope(scope)
                    .slot_id_at(index)
                    .map(mbv_ctrl::slot_id_to_u64);
                let sent = self.queue_op(
                    scope,
                    mbv_remote_player::QueueOp::Append {
                        items: vec![item],
                        before,
                    },
                );
                if sent == QueueOpEdit::Applied {
                    let len = self.queue_for_scope(scope).total_queue_len();
                    self.queue_for_scope_mut(scope).queue_cursor = index.min(len.saturating_sub(1));
                }
            }
            UndoEntry::Move { slot_id, from } => {
                let sent = self.queue_op(
                    scope,
                    mbv_remote_player::QueueOp::MoveSlot {
                        slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
                        to_index: from,
                    },
                );
                if sent == QueueOpEdit::Applied {
                    let len = self.queue_for_scope(scope).total_queue_len();
                    self.queue_for_scope_mut(scope).queue_cursor = from.min(len.saturating_sub(1));
                }
            }
        }
        self.set_queue_scope(scope);
    }

    /// Bookkeeping for an edit the owner's queue now holds (or that a legacy
    /// owner took): mark local playlist metadata dirty, record the undo
    /// entry, and advance the queue epoch so in-flight playlist mutations
    /// detect the changed queue.
    fn record_undoable_queue_edit(&mut self, scope: QueueScope, entry: UndoEntry) {
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_scope_mut(scope).push(entry);
        self.advance_queue_epoch();
    }
}
