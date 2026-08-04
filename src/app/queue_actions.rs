use super::types_playback::PlaylistMutation;
use super::ui_util::is_playable;
use super::{
    App, ConfirmAction, ConfirmModal, LibEvent, PendingQueueAction, QueueScope, SessionEvent,
    UndoEntry,
};
use mbv_core::api::MediaItem;
use mbv_core::player::PlayerCommand;
use std::sync::Arc;

impl App {
    fn retire_tracking_after_queue_mutation(&mut self) {
        self.retire_remote_tracking(true);
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
        if controls_playback_queue && (active || scope == QueueScope::Remote) {
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
        if controls_playback_queue && (active || scope == QueueScope::Remote) {
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

    pub(super) fn replace_active_playlist_mutation(
        &mut self,
        playlist_id: &str,
        mutation_id: u64,
        next: PlaylistMutation,
    ) -> bool {
        let Some(state) = self.playlist_mutations.get_mut(playlist_id) else {
            return false;
        };
        if state.active.as_ref().map(PlaylistMutation::mutation_id) != Some(mutation_id) {
            return false;
        }
        state.active = Some(next);
        self.start_playlist_mutation(playlist_id);
        true
    }

    fn start_playlist_mutation(&mut self, playlist_id: &str) {
        let stale = self
            .playlist_mutations
            .get(playlist_id)
            .and_then(|state| state.active.as_ref())
            .is_some_and(|mutation| match mutation {
                PlaylistMutation::Save {
                    queue_lineage,
                    source_playlist_id,
                    ..
                } => {
                    *queue_lineage != self.remote_queue_lineage
                        || self.queue_playlist_id() != Some(source_playlist_id.as_str())
                }
                PlaylistMutation::CreateAs {
                    queue_lineage,
                    source_playlist_id,
                    ..
                } => {
                    *queue_lineage != self.remote_queue_lineage
                        || self.queue_playlist_id() != source_playlist_id.as_deref()
                }
                PlaylistMutation::Replace { queue_lineage, .. } => {
                    *queue_lineage != self.remote_queue_lineage
                }
                _ => false,
            });
        if stale {
            log::debug!(target: "playlist", "discarding stale queued playlist mutation for {playlist_id}");
            if let Some(mutation_id) = self
                .playlist_mutations
                .get(playlist_id)
                .and_then(|state| state.active.as_ref())
                .map(PlaylistMutation::mutation_id)
            {
                self.finish_playlist_mutation(playlist_id, mutation_id);
            }
            return;
        }
        // Full updates (Save/Replace) recreate server playlist-entry IDs.
        // Consume eligibility derived from the old IDs dies at this boundary,
        // after earlier same-playlist mutations have completed, not when the
        // user merely queues the update. A Replace genuinely replaces queue
        // content/source and advances visible-queue lineage; a Save does not
        // change queue slots, content, or source, so it must preserve the
        // request lineage — otherwise its successful completion can never
        // satisfy the original-lineage checks that clear dirty state and run
        // an explicit save-before-replace continuation.
        let is_full_update = self
            .playlist_mutations
            .get(playlist_id)
            .and_then(|state| state.active.as_ref())
            .is_some_and(|mutation| {
                matches!(
                    mutation,
                    PlaylistMutation::Save { .. } | PlaylistMutation::Replace { .. }
                )
            });
        if is_full_update {
            let is_replace = self
                .playlist_mutations
                .get(playlist_id)
                .and_then(|state| state.active.as_ref())
                .is_some_and(|mutation| matches!(mutation, PlaylistMutation::Replace { .. }));
            if self.remote_tracking_source_is(playlist_id) {
                self.retire_remote_tracking(is_replace);
            }
            // A full update recreates entry identities for the playlist it
            // targets. Only the current source's items carry those identities,
            // so clear and persist them exactly when the update targets that
            // source — and regardless of whether a tracker is active. An
            // unrelated overwrite leaves the current source's identities valid
            // until completion flips the source.
            if self.queue_playlist_id() == Some(playlist_id) {
                self.clear_local_playlist_entry_ids();
                self.save_queue_state();
            }
        }
        let Some(state) = self.playlist_mutations.get_mut(playlist_id) else {
            return;
        };
        let Some(mutation) = state.active.as_mut() else {
            return;
        };
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        let playlist_id = playlist_id.to_string();
        let mutation_id = mutation.mutation_id();
        match mutation {
            PlaylistMutation::Save {
                queue_lineage,
                source_playlist_id,
                item_ids,
                ..
            } => {
                *item_ids = Some(self.player_tab.items.iter().map(|i| i.id.clone()).collect());
                let ids = item_ids.clone().unwrap_or_default();
                let queue_lineage = *queue_lineage;
                let source_playlist_id = source_playlist_id.clone();
                std::thread::spawn(move || {
                    let result = client.update_playlist_items(&playlist_id, &ids);
                    let _ = tx.send(SessionEvent::PlaylistMutationComplete {
                        mutation_id,
                        playlist_id,
                        queue_lineage,
                        source_playlist_id,
                        result,
                    });
                });
            }
            PlaylistMutation::CreateAs {
                name,
                coordinator_key,
                item_ids,
                queue_lineage,
                source_playlist_id,
                ..
            } => {
                *item_ids = Some(self.player_tab.items.iter().map(|i| i.id.clone()).collect());
                let ids = item_ids.clone().unwrap_or_default();
                let name = name.clone();
                let coordinator_key = coordinator_key.clone();
                let source_playlist_id = source_playlist_id.clone();
                let queue_lineage = *queue_lineage;
                std::thread::spawn(move || {
                    let result = client.create_playlist(&name, &ids);
                    let _ = tx.send(SessionEvent::PlaylistCreateComplete {
                        mutation_id,
                        coordinator_key,
                        name,
                        queue_lineage,
                        source_playlist_id,
                        result,
                    });
                });
            }
            PlaylistMutation::Replace {
                name,
                queue_lineage,
                source_playlist_id,
                item_ids,
                ..
            } => {
                *item_ids = Some(self.player_tab.items.iter().map(|i| i.id.clone()).collect());
                let replacement_name = name.to_string();
                let ids = item_ids.clone().unwrap_or_default();
                let queue_lineage = *queue_lineage;
                let source_playlist_id = source_playlist_id.clone();
                std::thread::spawn(move || {
                    let result = client
                        .delete_playlist(&playlist_id)
                        .and_then(|_| client.create_playlist(&replacement_name, &ids));
                    let _ = tx.send(SessionEvent::PlaylistReplacementComplete {
                        mutation_id,
                        playlist_id,
                        queue_lineage,
                        source_playlist_id,
                        name: replacement_name,
                        result,
                    });
                });
            }
            PlaylistMutation::ConsumeValidate {
                mutation_id,
                operation_id,
                session_id,
                tracking_id,
                epoch,
                occurrence_id,
                entry_id,
                media_id,
            } => {
                let (mutation_id, operation_id, session_id, tracking_id, epoch, occurrence_id) = (
                    *mutation_id,
                    *operation_id,
                    session_id.clone(),
                    *tracking_id,
                    *epoch,
                    *occurrence_id,
                );
                let entry_id = entry_id.to_string();
                let media_id = media_id.clone();
                std::thread::spawn(move || {
                    let result = client.get_playlist_items(&playlist_id).and_then(|items| {
                        super::run_loop_events::validate_remote_playlist_entry(
                            &items, &entry_id, &media_id,
                        )
                    });
                    let _ = tx.send(SessionEvent::ConsumeValidated {
                        mutation_id,
                        operation_id,
                        tracking_id,
                        session_id,
                        epoch,
                        occurrence_id,
                        playlist_id,
                        entry_id,
                        media_id,
                        result,
                    });
                });
            }
            PlaylistMutation::ConsumeDelete {
                mutation_id,
                operation_id,
                session_id,
                tracking_id,
                epoch,
                occurrence_id,
                entry_id,
                media_id,
                ..
            } => {
                let (mutation_id, operation_id, session_id, tracking_id, epoch, occurrence_id) = (
                    *mutation_id,
                    *operation_id,
                    session_id.clone(),
                    *tracking_id,
                    *epoch,
                    *occurrence_id,
                );
                let entry_id = entry_id.to_string();
                let media_id = media_id.to_string();
                std::thread::spawn(move || {
                    let result = client.delete_playlist_entry(&playlist_id, &entry_id);
                    let _ = tx.send(SessionEvent::ConsumeOutcome {
                        mutation_id,
                        operation_id,
                        tracking_id,
                        session_id,
                        epoch,
                        occurrence_id,
                        playlist_id,
                        entry_id,
                        media_id,
                        result,
                    });
                });
            }
        }
    }

    fn build_queue_state(&self) -> crate::config::QueueState {
        let positions: std::collections::HashMap<String, i64> = self
            .player_tab
            .items
            .iter()
            .filter(|i| i.playback_position_ticks > 0 && !i.is_audio())
            .map(|i| (i.id.clone(), i.playback_position_ticks))
            .collect();
        crate::config::QueueState {
            source: self.queue_source.clone(),
            items: self.player_tab.items.clone(),
            cursor: self.player_tab.queue_cursor,
            last_played_item_id: self.last_played_item_id.clone(),
            last_played_completed: self.last_played_completed,
            positions,
        }
    }

    pub(super) fn save_queue_state(&mut self) {
        let state = self.build_queue_state();
        if state.items.is_empty() {
            // Don't nuke the on-disk queue just because the local tab happens to be
            // empty while attached to a remote session — that reflects remote-control
            // UI state, not the user intentionally clearing their local queue.
            if self.connected_session_id.is_none() {
                crate::config::clear_queue_state();
            }
        } else {
            if let Err(e) = crate::config::save_queue_state(&state) {
                log::warn!(target: "queue", "failed to save queue state: {e}");
            }
        }
        self.persist_shared_queue_state(&state, false);
    }

    /// Like `save_queue_state`, but never deletes the on-disk snapshot when the
    /// in-memory queue happens to be empty. Quit is not a genuine "user cleared
    /// the queue" signal — an empty `player_tab.items` at quit time can equally
    /// mean this session never touched the local queue at all, and unconditionally
    /// deleting in that case wipes a perfectly valid snapshot from an earlier
    /// session with no recovery path. Only an explicit `ClearQueue` action (which
    /// goes through `save_queue_state`) should ever delete the file.
    pub(super) fn save_queue_state_no_clear(&mut self) {
        let state = self.build_queue_state();
        if !state.items.is_empty() {
            if let Err(e) = crate::config::save_queue_state(&state) {
                log::warn!(target: "queue", "failed to save queue state (no-clear): {e}");
            }
        }
        self.persist_shared_queue_state(&state, false);
    }

    fn save_queue_state_after_explicit_clear(&mut self) {
        self.save_queue_state();
        let state = self.build_queue_state();
        self.persist_shared_queue_state(&state, true);
    }

    pub(super) fn save_queue_state_after_remote_projection(&mut self) {
        let state = self.build_queue_state();
        if state.items.is_empty() {
            crate::config::clear_queue_state();
        } else if let Err(e) = crate::config::save_queue_state(&state) {
            log::warn!(target: "queue", "failed to save projected queue state: {e}");
        }
        self.persist_shared_queue_state(&state, true);
    }

    fn persist_shared_queue_state(&mut self, state: &crate::config::QueueState, allow_empty: bool) {
        if state.items.is_empty() && !allow_empty {
            return;
        }
        if let Ok(value) = serde_json::to_value(state) {
            let _ = self.persist_shared_document(
                mbv_core::shared_state::SharedDocumentKind::QueueState,
                value,
            );
        }
    }

    /// Wraps `restore_queue_state` with the guard startup needs: a local-
    /// daemon `App::new_remote` instance already had its queue populated
    /// during construction (`bootstrap_local_daemon_queue`, live-adopted
    /// from the daemon or loaded from disk for a cold daemon), so reading
    /// `queue_state.json` again here would clobber a live daemon queue with
    /// a stale disk snapshot on every new attach, and is simply redundant
    /// in the cold case.
    pub(super) fn maybe_restore_queue_state(&mut self) {
        if self.is_local_daemon() {
            return;
        }
        if self.shared_client.as_ref().is_some_and(|client| {
            matches!(
                client.state(),
                mbv_core::shared_client::SharedClientState::Shared
            )
        }) {
            return;
        }
        self.restore_queue_state();
    }

    /// Restore the queue from disk immediately and synchronously — the file
    /// already holds full `MediaItem`s, so this is a local read, no network
    /// round-trip, no in-flight window where the queue could be superseded
    /// by a real user action before it lands. See `spawn_enrich_queue_state`
    /// for the separate, best-effort refresh of played/position state.
    pub(super) fn restore_queue_state(&mut self) {
        let Some(state) = crate::config::load_queue_state() else {
            log::info!(target: "queue", "restore: no queue_state.json found, nothing to restore");
            return;
        };
        if state.items.is_empty() {
            log::info!(target: "queue", "restore: queue_state.json has no items, nothing to restore");
            return;
        }
        let cursor = super::actions::queue_restore_cursor(
            &state.items,
            state.cursor,
            state.last_played_item_id.as_deref(),
            state.last_played_completed,
        );
        let restored_count = state.items.len();
        self.last_played_item_id = state.last_played_item_id;
        self.last_played_completed = state.last_played_completed;
        self.queue_source = state.source;
        self.player_tab.set_items(state.items, cursor);
        self.queue_dirty = false;
        log::info!(target: "queue", "restore: restored {restored_count} item(s), cursor={cursor}");
        self.spawn_enrich_queue_state(state.positions);
    }

    /// Best-effort background refresh of played/position state for whatever
    /// is currently in `player_tab.items` (populated by `restore_queue_state`
    /// just before this is called). Merges by item ID into the *current*
    /// queue when it resolves, so it can never resurrect an item the user
    /// has since consumed, nor clobber a queue they've since replaced —
    /// unlike a wholesale overwrite, an ID that's no longer present is simply
    /// skipped.
    pub(super) fn spawn_enrich_queue_state(
        &self,
        positions: std::collections::HashMap<String, i64>,
    ) {
        let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        if item_ids.is_empty() {
            return;
        }
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let mut items = match client.get_items_by_ids(&item_ids) {
                Ok(items) => items,
                Err(e) => {
                    log::warn!(target: "queue", "restore: enrichment fetch failed: {e}");
                    return;
                }
            };
            // Apply locally-saved positions where they are fresher than what Emby returned.
            // Emby's UserData may lag by up to a few seconds after a Stopped report.
            for item in &mut items {
                if let Some(&saved_pos) = positions.get(&item.id) {
                    if saved_pos > item.playback_position_ticks {
                        log::info!(target: "player", "restore: applying saved pos={}s (Emby had {}s) for item={}",
                            saved_pos / mbv_core::api::TICKS_PER_SECOND,
                            item.playback_position_ticks / mbv_core::api::TICKS_PER_SECOND,
                            item.id);
                        item.playback_position_ticks = saved_pos;
                    }
                }
            }
            let _ = tx.send(LibEvent::QueueEnriched { items });
        });
    }

    pub(super) fn enqueue_selected(&mut self) {
        if self.library_tab == 0 {
            let Some(item) = self.current_home_item() else {
                return;
            };
            if item.is_folder {
                self.do_enqueue_folder(item);
                return;
            }
            if !is_playable(&item) {
                return;
            }
            log::info!(target: "library_route", "user action=enqueue item_id={:?} item_name={:?}", item.id, item.name);
            if self.in_non_library_thin_client_mode() {
                log::info!(target: "library_route", "route bypass action=enqueue item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
            }
            let resolved = self.route_for_item_via_ancestors(&item.id).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            self.append_item_to_queue_and_sync(item);
        } else {
            if self.enqueue_selected_artist_header() {
                return;
            }
            let Some(item) = self.current_lib_item() else {
                return;
            };
            if item.is_folder {
                self.do_enqueue_folder(item);
                return;
            }
            if !is_playable(&item) {
                return;
            }
            let lib_idx = self.library_tab - 1;
            let bypass = self.in_non_library_thin_client_mode();
            log::info!(target: "library_route", "{}", super::actions::enqueue_action_context(&item.id, &item.name, "library-view", bypass));
            let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            self.append_item_to_queue_and_sync(item);
        }
    }

    /// Shared append/sync/rollback tail for a single-item enqueue
    /// (extracted from `enqueue_selected`'s two branches, which had
    /// duplicated this verbatim): appends `item` to the visible queue,
    /// marks local queue metadata dirty when applicable, flashes a status
    /// confirmation, and syncs the append to the direct-remote queue /
    /// local persistence -- rolling the whole append back if the sync
    /// fails.
    fn append_item_to_queue_and_sync(&mut self, item: MediaItem) {
        let name = item.display_name();
        let scope = self.visible_queue_scope();
        let appended = item.clone();
        let previous_dirty = self.queue_dirty;
        let previous_queue = self.queue_for_scope(scope).clone();
        self.queue_for_scope_mut(scope).append_item(item);
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.flash_status(format!("Added: {name}"));
        if self.sync_playback_queue_after_append(scope, vec![appended]) {
            self.persist_local_queue_state_if_needed(scope);
            self.retire_tracking_after_queue_mutation();
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
    }
}
