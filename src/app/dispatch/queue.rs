use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::playback::PlaylistMutation;
use crate::app::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, QueueLink, QueueTarget,
    ReplacementExecutor, RoutedReplacementPrep, SessionEvent, UndoEntry,
};
use mbv_emby_model::EmbyItem;
use mbv_queue::{QueueItem, QueueSlotId};
use mbv_ui_model::ui_util::is_playable;
use mbv_ui_msg::{QueueMove, QueueRequest};
use std::cmp::Ordering;

pub(in crate::app) use self::queue_op::{QUEUE_OP_ANSWER_BOUND, QueueOpEdit};
mod pending_playback;
mod playlist;
mod playlist_mutation;
mod queue_op;
mod replacement;

impl App {
    pub(in crate::app) fn remove_from_queue(&mut self, pos: usize) {
        let target = self.queue_edit_target();
        let len = self
            .view_for(target)
            .map_or(0, |view| view.total_queue_len());
        if pos >= len {
            if let Some(view) = self.view_for(target) {
                let cursor = view.cursor();
                view.set_cursor(cursor);
            }
            return;
        }
        let Some(slot_id) = self.view_for(target).and_then(|view| view.slot_id_at(pos)) else {
            return;
        };
        if self.queue_slot_needs_active_confirm(target.link, slot_id) {
            self.ask_confirm(ConfirmModal::two_button(
                "Remove now-playing item and stop playback?".into(),
                "Confirm",
                "Cancel",
                ConfirmAction::RemoveActiveQueueItem(pos),
            ));
            return;
        }
        // Slot identity and the undo payload are resolved against the
        // owner's last accepted state before the edit is sent: the Client
        // holds no editable queue, so the view changes only when the
        // owner's answer is adopted.
        let Some(view) = self.view_for(target) else {
            return;
        };
        let cursor_before = view.cursor();
        let Some(item) = view.item_at(pos).cloned() else {
            return;
        };
        let sent = self.queue_op(
            target,
            mbv_remote_player::QueueOp::RemoveSlot {
                slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
            },
        );
        if sent != QueueOpEdit::NotApplied {
            self.record_undoable_queue_edit(UndoEntry::Remove {
                target,
                item: Box::new(item),
                index: pos,
            });
        }
        if sent == QueueOpEdit::Applied {
            // The selected slot is gone; the selection moves to the entry
            // now at its former position.
            let cursor = if pos < cursor_before {
                cursor_before - 1
            } else {
                cursor_before
            };
            if let Some(view) = self.view_for(target) {
                view.set_cursor(cursor);
            }
        }
    }

    /// Whether removing `slot_id` from `link`'s playback queue needs the
    /// dedicated now-playing confirmation: the slot is the actively playing
    /// item. Identity, not ordinals: queue edits shift ordinals while the
    /// status' `current_idx` stays stale (invariant 26), so the comparison
    /// names the owner's observed active slot and falls back to the status
    /// ordinal only while no snapshot has named one. Non-playback links
    /// never need it, and neither does a shown resting queue: it holds no
    /// playhead, so no row of it can be the playing one (design D7).
    pub(in crate::app) fn queue_slot_needs_active_confirm(
        &self,
        link: QueueLink,
        slot_id: mbv_queue::QueueSlotId,
    ) -> bool {
        if !self.queue_link_is_playback(link) || self.shows_resting_local_queue() {
            return false;
        }
        let status = self.shown_playback_status();
        if !status.active {
            return false;
        }
        let queue = self.displayed_queue();
        match queue.active_slot_id() {
            Some(active) => active == slot_id,
            None => queue
                .slot_index(slot_id)
                .is_some_and(|pos| pos == status.current_idx),
        }
    }

    /// Remove the slots named by `slot_ids` from `link`'s queue as one
    /// owner edit (row 5.2): the owner applies the whole range before
    /// publishing the queue snapshot the Client adopts, so the list never
    /// shrinks one row per removal.
    ///
    /// The cursor comes to rest on the item that preceded the deleted range;
    /// when the range started the queue there is nothing before it, so the
    /// first item that followed takes the cursor instead. A selection that
    /// includes the now-playing row waits for one confirmation that controls
    /// the entire multi-delete: nothing is removed before the user answers,
    /// because stopping playback is a decision the user has to answer,
    /// not a plain edit.
    pub(in crate::app) fn remove_slots_from_queue(
        &mut self,
        target: QueueTarget,
        slot_ids: &[mbv_queue::QueueSlotId],
    ) {
        let needs_confirm = slot_ids
            .iter()
            .any(|slot_id| self.queue_slot_needs_active_confirm(target.link, *slot_id));
        if needs_confirm {
            let mut confirmed: Vec<mbv_queue::QueueSlotId> = slot_ids.to_vec();
            confirmed.sort_unstable();
            confirmed.dedup();
            confirmed.retain(|slot_id| {
                self.view_for(target)
                    .is_some_and(|view| view.slot_index(*slot_id).is_some())
            });
            if confirmed.is_empty() {
                return;
            }
            self.ask_confirm(ConfirmModal::two_button(
                format!(
                    "Remove {} items including now-playing and stop playback?",
                    confirmed.len()
                ),
                "Confirm",
                "Cancel",
                ConfirmAction::RemoveActiveQueueSelection {
                    link: target.link,
                    slot_ids: confirmed,
                },
            ));
            return;
        }
        let Some(view) = self.view_for(target) else {
            return;
        };
        let mut targets: Vec<(mbv_queue::QueueSlotId, usize)> = slot_ids
            .iter()
            .filter_map(|slot_id| view.slot_index(*slot_id).map(|pos| (*slot_id, pos)))
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
                let item = view.item_at(*pos)?.clone();
                Some((*pos, item))
            })
            .collect();
        let raw_slot_ids: Vec<u64> = targets
            .iter()
            .map(|(slot_id, _)| mbv_ctrl::slot_id_to_u64(*slot_id))
            .collect();
        let sent = self.queue_op(
            target,
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
            let stack = self.undo_stack_for_link_mut(target.link);
            for (pos, item) in removed.into_iter().rev() {
                stack.push(UndoEntry::Remove {
                    target,
                    item: Box::new(item),
                    index: pos,
                });
            }
        }
        if self.local_queue_metadata_applies(target.link) {
            self.queue_dirty = true;
        }
        if sent == QueueOpEdit::Applied {
            if let Some(view) = self.view_for(target) {
                view.set_cursor(cursor_after);
            }
            // The mounted component owns the painted cursor and only adopts
            // `App`'s value when a re-anchor is armed; the adopted snapshot
            // moved the cursor onto the item preceding the range.
            self.pending_queue_cursor_reanchor = Some(target.link);
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

    /// Move the queue item at `index` to play directly after the playhead
    /// (add-tauri-gui-client row 6.2, the canvas row menu's "Play next").
    /// The move is the owner's slot move, so the queue edit follows the same
    /// request path a drag or a `Shift+Arrow` move does. A playhead-less
    /// queue has no next position, and the playing row and the row already
    /// next to it are no-ops. A resting queue the GUI shows is playhead-less
    /// whatever the playing queue does — the playing queue's index names none
    /// of its rows — so its start slot stands where a playhead stands, and the
    /// row lands directly after it (design D7, D9 of `gui-named-queues`).
    pub(in crate::app) fn play_queue_item_next(&mut self, index: usize) {
        let len = self.displayed_queue().total_queue_len();
        if index >= len {
            return;
        }
        let Some(to) = self.play_next_position(index, len) else {
            return;
        };
        self.move_queue_item_to(self.queue_edit_target(), index, to);
    }

    /// Where a "Play next" move of the row at `index` lands in the shown queue
    /// (add-tauri-gui-client row 6.2): directly after the queue's anchor — the
    /// playhead, or a resting queue's start slot — that queue's own analogue of
    /// a playhead, so the move reaches the queue the panel shows (design D7,
    /// D9 of `gui-named-queues`). A resting queue with no start slot plays from
    /// its first row, so the row lands first. `None` when the shown queue has
    /// nothing to play next after (no anchor, or an anchor past the queue's
    /// end), or the row already sits in that place.
    fn play_next_position(&self, index: usize, len: usize) -> Option<usize> {
        let to = match self.shown_queue_anchor() {
            Some(anchor) if anchor < len => position_after(anchor, index)?,
            // A resting queue with no start slot plays from its first row, so
            // the row lands first: there is no anchor to land after.
            None if self.shows_resting_local_queue() => 0,
            _ => return None,
        };
        (to != index).then_some(to)
    }

    /// Moves the item at `from` by `delta` within the displayed queue's
    /// link. The target is passed explicitly (split-queue-cursor-ownership
    /// D2): the shell resolves the slot the user selected and hands it over
    /// rather than this function re-reading `queue.cursor()` as an
    /// ambient argument channel.
    fn move_queue_item_by(&mut self, from: usize, delta: isize) {
        let queue = self.displayed_queue();
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
        self.move_queue_item_to(self.queue_edit_target(), from, to);
    }

    /// Moves the item at `from` to `to` in the queue `target` names by asking
    /// that Player owner to move the slot (row 5.2). The caller resolves the
    /// target (split-queue-cursor-ownership D2): the shell resolves the slot
    /// the user selected rather than this function re-reading the shown
    /// queue's cursor as an ambient argument channel.
    pub(in crate::app) fn move_queue_item_to(
        &mut self,
        target: QueueTarget,
        from: usize,
        to: usize,
    ) {
        let Some(view) = self.view_for(target) else {
            return;
        };
        let len = view.total_queue_len();
        if from >= len || to >= len || from == to {
            return;
        }
        let Some(slot_id) = view.slot_id_at(from) else {
            return;
        };
        let sent = self.queue_op(
            target,
            mbv_remote_player::QueueOp::MoveSlot {
                slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
                to_index: to,
            },
        );
        let entry = || UndoEntry::Move {
            target,
            slot_id,
            from,
        };
        match sent {
            QueueOpEdit::Applied => {
                if target.link != QueueLink::Peer {
                    // The selection stays on the moved entry at its new
                    // position.
                    if let Some(view) = self.view_for(target) {
                        view.set_cursor(to);
                    }
                }
                self.record_undoable_queue_edit(entry());
            }
            QueueOpEdit::SentLegacy => {
                self.record_undoable_queue_edit(entry());
            }
            QueueOpEdit::NotApplied => {}
        }
    }

    /// Pops and reverses the most recent undoable edit in `link`'s queue by
    /// sending the inverse edit to the queue that edit reached (design D7):
    /// a removed item is re-appended before the entry now at its former
    /// position, and a moved slot is moved back to where it came from. The
    /// inverse edit names the Named queue the undone entry recorded, whichever
    /// entry is shown now. The owner rejects an undo whose slot is gone as
    /// stale slot addressing; the pump flashes that rejection. No-op if the
    /// undo stack for that link is empty.
    pub(in crate::app) fn undo_last_queue_edit(&mut self, link: QueueLink) {
        let Some(entry) = self.undo_stack_for_link_mut(link).pop() else {
            return;
        };
        let target = entry.target();
        match entry {
            UndoEntry::Remove { item, index, .. } => {
                // Resolve the anchor at undo time: the slot now at the
                // removed item's former position, or the end when that
                // position no longer exists. A resting queue the Client no
                // longer shows holds no Client-side view, so its anchor is
                // unknown and the item returns at the queue's end; the Owner
                // addresses it by identity either way.
                let before = self
                    .view_for(target)
                    .and_then(|view| view.slot_id_at(index))
                    .map(mbv_ctrl::slot_id_to_u64);
                let sent = self.queue_op(
                    target,
                    mbv_remote_player::QueueOp::Append {
                        items: vec![*item],
                        before,
                    },
                );
                if sent == QueueOpEdit::Applied {
                    self.place_undo_cursor(target, index);
                }
            }
            UndoEntry::Move { slot_id, from, .. } => {
                let sent = self.queue_op(
                    target,
                    mbv_remote_player::QueueOp::MoveSlot {
                        slot_id: mbv_ctrl::slot_id_to_u64(slot_id),
                        to_index: from,
                    },
                );
                if sent == QueueOpEdit::Applied {
                    self.place_undo_cursor(target, from);
                }
            }
        }
    }

    /// Put the shown queue's active row on the undone edit's position, when
    /// the Client shows the queue the edit reached: another entry's view is
    /// left where the user put it (design D7).
    fn place_undo_cursor(&mut self, target: QueueTarget, index: usize) {
        let shows_edited_queue = match target.link {
            // A peer's queue is the whole of that link, so its undo always
            // lands on the view the panel shows.
            QueueLink::Peer => true,
            QueueLink::Local => {
                target.queue.or_else(|| self.named_queues.current()) == self.displayed_queue_id()
            }
        };
        if shows_edited_queue && let Some(view) = self.view_for(target) {
            view.set_cursor(index);
        }
    }

    /// Bookkeeping for an edit the owner's queue now holds (or that a legacy
    /// owner took): mark local playlist metadata dirty, record the undo
    /// entry, and advance the queue epoch so in-flight playlist mutations
    /// detect the changed queue. The entry itself names the queue it edited,
    /// so the stack it lands on and the metadata gate both read it.
    fn record_undoable_queue_edit(&mut self, entry: UndoEntry) {
        if self.local_queue_metadata_applies(entry.target().link) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_link_mut(entry.target().link)
            .push(entry);
        self.advance_queue_epoch();
    }

    /// The `QueueRequest` dispatch every Client shares (extract-client-core
    /// design D5): each request runs the same App-side effect whether the TUI
    /// shell's Queue component emitted it or the GUI Client's Queue Surface
    /// leaf did. Slot resolution and the link/focus/hold-window side effects
    /// mirror `select_queue_slot`, and the effect is driven with the resolved
    /// index (design D2).
    pub fn handle_queue_request(&mut self, request: QueueRequest) {
        match request {
            QueueRequest::Cursor { link, slot_id } => {
                // Plain navigation: `select_queue_slot` applies the
                // link/focus/hold-window side effects; there is no effect to
                // drive with the resolved index, so it is discarded.
                let _ = self.select_queue_slot(link, slot_id);
            }
            QueueRequest::Play { link, slot_id } => {
                if let Some(index) = self.select_queue_slot(link, slot_id) {
                    self.dispatch(&crate::app::dispatch::action::AppCommand::QueuePlayCursor(
                        index,
                    ));
                }
            }
            QueueRequest::Remove { link, slot_id } => {
                if let Some(index) = self.select_queue_slot(link, slot_id) {
                    self.remove_from_queue(index);
                }
            }
            QueueRequest::RemoveSelection { link, slot_ids } => {
                // One batch edit: the owner applies the whole range before
                // publishing a queue snapshot, so the list does not shrink one
                // row per removal.
                self.remove_slots_from_queue(self.queue_target(link), &slot_ids);
            }
            QueueRequest::Move {
                link,
                slot_id,
                direction,
            } => {
                self.handle_queue_move(link, slot_id, direction);
            }
            QueueRequest::MoveTo {
                link,
                slot_id,
                onto,
            } => {
                self.handle_queue_move_to(link, slot_id, onto);
            }
            // The row drag's resolved drop: the slot's final queue index, so
            // the move lands where the insertion line was painted. One
            // request per drag: the Client core emits it on the release, and
            // motion only previews the drop.
            QueueRequest::MoveToIndex {
                link,
                slot_id,
                to_index,
            } => {
                let Some(from) = self.select_queue_slot(link, slot_id) else {
                    return;
                };
                if from != to_index {
                    self.move_queue_item_to(self.queue_target(link), from, to_index);
                }
            }
            QueueRequest::ResizeColumnLive(width) => {
                self.queue_column_width = width;
            }
            // The boundary only emits End after a Live move, so persist
            // unconditionally rather than tracking the drag's start width.
            QueueRequest::ResizeColumnEnd(width) => {
                self.queue_column_width = width;
                self.save_prefs();
            }
            QueueRequest::Undo { link } => {
                self.handle_queue_undo(link);
            }
        }
    }

    fn handle_queue_move(&mut self, link: QueueLink, slot_id: QueueSlotId, direction: QueueMove) {
        if let Some(index) = self.select_queue_slot(link, slot_id) {
            match direction {
                QueueMove::Up => self.move_queue_item_up(index),
                QueueMove::Down => self.move_queue_item_down(index),
            }
        }
    }

    fn handle_queue_move_to(&mut self, link: QueueLink, slot_id: QueueSlotId, onto: QueueSlotId) {
        let Some(from) = self.select_queue_slot(link, slot_id) else {
            return;
        };
        let Some(to) = self.displayed_slot_index(onto) else {
            return;
        };
        if from != to {
            self.move_queue_item_to(self.queue_target(link), from, to);
        }
    }

    /// The undo request's dispatch: an owner operation on the shown queue's
    /// link (`unified-playback-queue`: undo history belongs to the Client that
    /// made the edits, and the inverse edit goes to the Player owner that
    /// holds the queue). The component can still show `Peer` for a frame
    /// after a remote disconnect, before the projection refresh flips it back;
    /// once no direct remote queue exists the visible queue is the Local one,
    /// so the undo targets Local rather than naming a queue that is gone.
    fn handle_queue_undo(&mut self, link: QueueLink) {
        let link = if link == QueueLink::Peer && !self.has_direct_remote_queue() {
            QueueLink::Local
        } else {
            link
        };
        self.undo_last_queue_edit(link);
    }

    /// Resolve `slot_id` to its index in `link`'s queue and apply the
    /// link/focus/hold-window side effects, returning the resolved index —
    /// the operand for the effect the caller is about to run. The
    /// component's own cursor is authoritative for selection; the App's queue
    /// cursor is not written here (task 3.1: the mirror is gone).
    pub fn select_queue_slot(&mut self, link: QueueLink, slot_id: QueueSlotId) -> Option<usize> {
        if link == QueueLink::Peer && !self.has_direct_remote_queue() {
            return None;
        }
        let index = self.displayed_slot_index(slot_id)?;
        self.set_panel_focus(PanelFocus::Queue);
        self.mark_queue_cursor_user_active();
        Some(index)
    }
}

/// The position a moved row lands at so it sits directly after `anchor`: the
/// owner applies the move as a removal followed by an insert, so a row that
/// stood before the anchor lands on the anchor's own index — the anchor shifts
/// down by one — and one after it lands one past. `None` when the row is the
/// anchor and has nowhere to go.
fn position_after(anchor: usize, index: usize) -> Option<usize> {
    match index.cmp(&anchor) {
        Ordering::Less => Some(anchor),
        Ordering::Greater => Some(anchor + 1),
        Ordering::Equal => None,
    }
}
