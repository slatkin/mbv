use super::types_playback::PlaylistMutation;
use super::ui_util::is_playable;
use super::{
    App, ConfirmAction, ConfirmModal, LibEvent, PendingQueueAction, QueueScope, SessionEvent,
    UndoEntry,
};
use mbv_core::api::MediaItem;
use mbv_core::player::PlayerCommand;
use std::sync::Arc;

#[path = "queue_actions_playlist_mutation.rs"]
mod queue_actions_playlist_mutation;

impl App {
    fn retire_tracking_after_queue_mutation(&mut self) {
        let was_tracking = self.remote_tracker.is_some();
        self.retire_remote_tracking(true);
        if was_tracking {
            self.flash_status("Remote tracking stopped".into());
        }
    }

    pub(super) fn retire_remote_tracking_after_queue_mutation(&mut self) {
        self.retire_tracking_after_queue_mutation();
    }

    pub(super) fn remove_from_queue(&mut self, pos: usize) {
        let scope = self.visible_queue_scope();
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if pos >= self.queue_for_scope(scope).items.len() {
            let queue = self.queue_for_scope_mut(scope);
            queue.clamp_cursor();
            return;
        }
        if controls_playback_queue && active && current_idx == pos {
            self.confirm_modal = Some(ConfirmModal {
                title: " Remove Item ".into(),
                message: "Remove now-playing item and stop playback?".into(),
                hint: "[y] Confirm    [Esc] Cancel".into(),
                on_confirm: ConfirmAction::RemoveActiveQueueItem(pos),
            });
            return;
        }
        let cursor_before = self.queue_for_scope(scope).queue_cursor;
        let Some(item) = self.queue_for_scope_mut(scope).remove_slot_at(pos) else {
            return;
        };
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_scope_mut(scope)
            .push(UndoEntry::Remove(pos, Box::new(item)));
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && active && pos < current_idx {
            self.pending_active_idx = Some(current_idx - 1);
        }
        if controls_playback_queue
            && (active || scope == QueueScope::Remote || self.player.is_remote())
        {
            self.player.send_command(PlayerCommand::QueueRemove(pos));
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
        self.retire_tracking_after_queue_mutation();
    }

    /// Moves the item at the displayed queue's cursor one position earlier.
    /// No-op at the start of the queue.
    pub(super) fn move_queue_item_up(&mut self) {
        self.move_queue_item_by(-1);
    }

    /// Moves the item at the displayed queue's cursor one position later.
    /// No-op at the end of the queue.
    pub(super) fn move_queue_item_down(&mut self) {
        self.move_queue_item_by(1);
    }

    fn move_queue_item_by(&mut self, delta: isize) {
        let scope = self.visible_queue_scope();
        let queue = self.queue_for_scope(scope);
        let from = queue.queue_cursor;
        let len = queue.items.len();
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
        let Some(slot_id) = self.queue_for_scope_mut(scope).slot_id_at(from) else {
            return;
        };
        if self.apply_queue_move_by_slot(scope, slot_id, from, to) {
            self.retire_tracking_after_queue_mutation();
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
    pub(super) fn apply_queue_move(&mut self, scope: QueueScope, from: usize, to: usize) -> bool {
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
        let len = self.queue_for_scope(scope).items.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let (active, active_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if !self.queue_for_scope_mut(scope).move_slot(slot_id, to) {
            return false;
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && active {
            let new_active_idx = if active_idx == from {
                Some(to)
            } else if from < active_idx && active_idx <= to {
                Some(active_idx - 1)
            } else if to <= active_idx && active_idx < from {
                Some(active_idx + 1)
            } else {
                None
            };
            if let Some(new_active_idx) = new_active_idx {
                if new_active_idx != active_idx {
                    self.pending_active_idx = Some(new_active_idx);
                }
            }
        }
        if controls_playback_queue
            && (active || scope == QueueScope::Remote || self.player.is_remote())
        {
            self.player.send_command(PlayerCommand::QueueMove(from, to));
        }
        true
    }

    /// Pops and reverses the most recent undoable edit in `scope`'s queue —
    /// re-inserting a removed item, or swapping a moved item back to where it
    /// came from. No-op if the undo stack for that scope is empty.
    pub(super) fn undo_last_queue_edit(&mut self, scope: QueueScope) {
        let Some(entry) = self.undo_stack_for_scope_mut(scope).pop() else {
            return;
        };
        match entry {
            UndoEntry::Remove(idx, item) => {
                let queue = self.queue_for_scope_mut(scope);
                let idx = idx.min(queue.items.len());
                queue.insert_item_at(idx, *item);
                if self.local_queue_metadata_applies(scope) {
                    self.queue_dirty = true;
                }
                self.persist_local_queue_state_if_needed(scope);
                self.retire_tracking_after_queue_mutation();
            }
            UndoEntry::Move { from, to, slot_id } => {
                let still_in_place = self.queue_for_scope(scope).slot_id_matches_at(to, slot_id);
                if !still_in_place || !self.apply_queue_move(scope, to, from) {
                    self.flash_status_high("Can't undo move: queue changed since then".into());
                    return;
                }
                self.retire_tracking_after_queue_mutation();
            }
        }
        self.set_queue_scope(scope);
    }

    pub(super) fn on_queue_replace_silent(&mut self) {
        self.queue_source = crate::config::QueueSource::Unknown;
        self.queue_dirty = false;
    }

    pub(super) fn replace_queue_or_prompt(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action)
            && self.queue_dirty
            && self.queue_is_saved_playlist()
        {
            self.pending_queue_action = Some(action);
            let name = super::ui_util::trunc_str(self.queue_playlist_name(), 36);
            self.confirm_modal = Some(ConfirmModal {
                title: " Unsaved Playlist Changes ".into(),
                message: format!("Save changes to \"{}\"?", name),
                hint: "[s]Save  [d]Discard  [Esc]Cancel".into(),
                on_confirm: ConfirmAction::DiscardOrSaveDirtyPlaylist,
            });
        } else {
            self.execute_pending_queue_action(action);
        }
    }

    pub(super) fn execute_pending_queue_action(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action) {
            self.queue_dirty = false;
        }
        match action {
            PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
            } => {
                let direct_remote = self.has_direct_remote_queue();
                if self.local_queue_metadata_applies(self.playback_target_queue_scope()) {
                    self.queue_source = source;
                }
                if !direct_remote {
                    self.replace_playback_queue(items.clone(), start_idx);
                }
                self.set_queue_scope(self.playback_target_queue_scope());
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    self.clear_playback_overlays();
                    let id = conn_id.clone();
                    let label = items
                        .get(start_idx)
                        .map(|i| i.playback_label())
                        .unwrap_or_default();
                    self.flash_status(format!("Playing on remote: {label}"));
                    self.submit_attached_sequence(&id, &items, start_idx);
                } else {
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    self.player.play_queue(
                        items,
                        start_idx,
                        self.queue_source.clone(),
                        c,
                        self.ui_volume,
                    );
                    self.player
                        .send_command(PlayerCommand::SetMute(self.mute_on));
                }
                if !direct_remote {
                    self.save_queue_state();
                }
            }
            PendingQueueAction::ClearQueue => {
                let scope = self.visible_queue_scope();
                let had_items = !self.queue_for_scope(scope).items.is_empty();
                if self.local_queue_metadata_applies(scope) {
                    self.clear_local_queue_metadata();
                } else {
                    self.remote_queue_undo_stack.clear();
                }
                if scope == QueueScope::Remote && had_items {
                    self.replace_direct_remote_queue(Vec::new(), 0);
                } else if self.queue_scope_is_playback(scope) {
                    self.player.stop();
                    if self.is_local_daemon() {
                        self.player.send_command(PlayerCommand::ReplaceQueue {
                            items: Vec::new(),
                            start_idx: 0,
                        });
                    }
                }
                if scope != QueueScope::Remote {
                    let queue = self.queue_for_scope_mut(scope);
                    queue.clear();
                }
                if had_items {
                    self.retire_tracking_after_queue_mutation();
                }
                if self.local_queue_metadata_applies(scope) {
                    self.save_queue_state_after_explicit_clear();
                }
                self.flash_status("Queue cleared".into());
            }
        }
    }

    pub(super) fn queue_is_saved_playlist(&self) -> bool {
        matches!(
            &self.queue_source,
            crate::config::QueueSource::Playlist { id: Some(_), .. }
        )
    }

    pub(super) fn queue_playlist_id(&self) -> Option<&str> {
        if let crate::config::QueueSource::Playlist {
            id: Some(ref id), ..
        } = self.queue_source
        {
            Some(id.as_str())
        } else {
            None
        }
    }

    pub(super) fn queue_playlist_name(&self) -> &str {
        if let crate::config::QueueSource::Playlist { ref name, .. } = self.queue_source {
            name.as_str()
        } else {
            ""
        }
    }

    pub(super) fn save_playlist_to_emby(&mut self) {
        let Some(playlist_id) = self.queue_playlist_id() else {
            return;
        };
        let playlist_id = playlist_id.to_string();
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        self.enqueue_playlist_mutation(
            playlist_id.clone(),
            PlaylistMutation::Save {
                mutation_id,
                queue_lineage: self.remote_queue_lineage,
                source_playlist_id: playlist_id,
                item_ids: None,
            },
        );
    }

    pub(super) fn save_queue_as_playlist(&mut self, name: String) {
        let source_playlist_id = self.queue_playlist_id().map(str::to_string);
        let queue_lineage = self.remote_queue_lineage;
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        let key = source_playlist_id
            .clone()
            .filter(|id| self.playlist_mutation_pending(id))
            .unwrap_or_else(|| format!("create:{mutation_id}"));
        self.enqueue_playlist_mutation(
            key.clone(),
            PlaylistMutation::CreateAs {
                mutation_id,
                coordinator_key: key,
                name,
                queue_lineage,
                source_playlist_id,
                item_ids: None,
            },
        );
    }

    fn playlist_mutation_pending(&self, playlist_id: &str) -> bool {
        self.playlist_mutations
            .get(playlist_id)
            .is_some_and(|state| state.active.is_some() || !state.queued.is_empty())
    }

    /// Clears `playlist_item_id` from the local queue's items after a full
    /// playlist update that recreates server entry identities. The local queue
    /// is the queue whose items every full update (Save/Replace/CreateAs)
    /// pushes to Emby, so those identities are invalidated whether or not
    /// tracking is active.
    pub(super) fn clear_local_playlist_entry_ids(&mut self) {
        for item in &mut self.player_tab.items {
            item.playlist_item_id.clear();
        }
        self.player_tab.sync_queue_model_from_items_if_needed();
    }

    pub(super) fn enqueue_playlist_mutation(
        &mut self,
        playlist_id: String,
        mutation: PlaylistMutation,
    ) {
        let state = self
            .playlist_mutations
            .entry(playlist_id.clone())
            .or_default();
        if state.active.is_some() {
            state.queued.push_back(mutation);
        } else {
            state.active = Some(mutation);
            self.start_playlist_mutation(&playlist_id);
        }
    }

    pub(super) fn finish_playlist_mutation(&mut self, playlist_id: &str, mutation_id: u64) {
        let Some(state) = self.playlist_mutations.get_mut(playlist_id) else {
            return;
        };
        if state.active.as_ref().map(PlaylistMutation::mutation_id) != Some(mutation_id) {
            return;
        }
        state.active = None;
        if let Some(next) = state.queued.pop_front() {
            state.active = Some(next);
            self.start_playlist_mutation(playlist_id);
        } else {
            self.playlist_mutations.remove(playlist_id);
        }
    }
}
