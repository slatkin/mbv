use super::ui_util::is_playable;
use super::{
    App, ConfirmAction, ConfirmModal, LibEvent, PendingQueueAction, QueueScope, UndoEntry,
};
use mbv_core::api::MediaItem;
use mbv_core::player::PlayerCommand;
use std::sync::Arc;

impl App {
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
        let Some(item) = self.queue_for_scope_mut(scope).remove_slot_at(pos) else {
            return;
        };
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_scope_mut(scope)
            .push(UndoEntry::Remove(pos, Box::new(item)));
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && (active || scope == QueueScope::Remote) {
            self.player.send_command(PlayerCommand::QueueRemove(pos));
            // Player thread adjusts current_idx when it processes the command.
            // No eager adjustment here — doing so races with the player thread
            // and can cause index mismatches during rapid removals.
        }
        let queue = self.queue_for_scope_mut(scope);
        queue.clamp_cursor();
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
            if scope == QueueScope::Remote {
                // Store the moved item's identity so QueueUpdated can verify
                // the move was actually applied before consuming the
                // optimistic cursor.
                if let Some(item) = self.queue_for_scope(scope).items.get(to) {
                    self.pending_remote_move = Some(super::types_playback::PendingRemoteMove {
                        item_id: item.id.clone(),
                        playlist_item_id: item.playlist_item_id.clone(),
                        target_idx: to,
                    });
                }
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
        let active = self.player.status.lock().unwrap().active;
        if !self.queue_for_scope_mut(scope).move_slot(slot_id, to) {
            return false;
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.persist_local_queue_state_if_needed(scope);
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
            }
            UndoEntry::Move { from, to, slot_id } => {
                let still_in_place = self.queue_for_scope(scope).slot_id_matches_at(to, slot_id);
                if !still_in_place || !self.apply_queue_move(scope, to, from) {
                    self.flash_status_high("Can't undo move: queue changed since then".into());
                    return;
                }
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
                    let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
                    let start_ticks = items
                        .get(start_idx)
                        .map_or(0, |i| i.playback_position_ticks);
                    let label = items
                        .get(start_idx)
                        .map(|i| i.playback_label())
                        .unwrap_or_default();
                    self.flash_status(format!("Playing on remote: {label}"));
                    self.do_session_command(move |c| {
                        c.session_play_items(&id, &item_ids, start_idx, start_ticks)
                    });
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
                if self.local_queue_metadata_applies(scope) {
                    self.clear_local_queue_metadata();
                } else {
                    self.remote_queue_undo_stack.clear();
                }
                if scope == QueueScope::Remote {
                    self.replace_direct_remote_queue(Vec::new(), 0);
                } else if self.queue_scope_is_playback(scope) {
                    self.player.stop();
                    if self.is_local_daemon {
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
                self.persist_local_queue_state_if_needed(scope);
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

    fn queue_playlist_id(&self) -> Option<&str> {
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

    pub(super) fn save_playlist_to_emby(&self) {
        let Some(playlist_id) = self.queue_playlist_id() else {
            return;
        };
        let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        let client = self.client.lock().unwrap().clone();
        let playlist_id = playlist_id.to_string();
        std::thread::spawn(move || {
            if let Err(e) = client.update_playlist_items(&playlist_id, &item_ids) {
                log::error!(target: "playlist", "Failed to save playlist: {e}");
            }
        });
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

    pub(super) fn save_queue_state(&self) {
        let state = self.build_queue_state();
        if state.items.is_empty() {
            // Don't nuke the on-disk queue just because the local tab happens to be
            // empty while attached to a remote session — that reflects remote-control
            // UI state, not the user intentionally clearing their local queue.
            if self.connected_session_id.is_none() {
                crate::config::clear_queue_state();
            }
        } else {
            crate::config::save_queue_state(&state);
        }
    }

    /// Like `save_queue_state`, but never deletes the on-disk snapshot when the
    /// in-memory queue happens to be empty. Quit is not a genuine "user cleared
    /// the queue" signal — an empty `player_tab.items` at quit time can equally
    /// mean this session never touched the local queue at all, and unconditionally
    /// deleting in that case wipes a perfectly valid snapshot from an earlier
    /// session with no recovery path. Only an explicit `ClearQueue` action (which
    /// goes through `save_queue_state`) should ever delete the file.
    pub(super) fn save_queue_state_no_clear(&self) {
        let state = self.build_queue_state();
        if !state.items.is_empty() {
            crate::config::save_queue_state(&state);
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
        if self.is_local_daemon {
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
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
    }
}
