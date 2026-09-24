use super::notify_actions::ToastSeverity;
use super::types_playback::PlaylistMutation;
use super::ui_util::is_playable;
use super::{
    App, ConfirmAction, ConfirmModal, LibEvent, PanelFocus, PendingQueueAction, QueueScope,
    ReplacementExecutor, RoutedReplacementPrep, SessionEvent, SidebarId, UndoEntry,
};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use mbv_core::player::PlayerCommand;

#[path = "queue_actions_playlist_mutation.rs"]
mod queue_actions_playlist_mutation;

impl App {
    pub(super) fn remove_from_queue(&mut self, pos: usize) {
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
        self.advance_remote_queue_lineage();
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
            .map(|(npid, item)| item.id == *npid)
            .unwrap_or(false)
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
    pub(super) fn remove_slots_from_queue(
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
        }
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
        self.advance_remote_queue_lineage();
    }

    /// Moves the item at `from` one position earlier. No-op at the start of
    /// the queue.
    pub(super) fn move_queue_item_up(&mut self, from: usize) {
        self.move_queue_item_by(from, -1);
    }

    /// Moves the item at `from` one position later. No-op at the end of the
    /// queue.
    pub(super) fn move_queue_item_down(&mut self, from: usize) {
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
    pub(super) fn move_queue_item_to(&mut self, scope: QueueScope, from: usize, to: usize) {
        let Some(slot_id) = self.queue_for_scope_mut(scope).slot_id_at(from) else {
            return;
        };
        if self.apply_queue_move_by_slot(scope, slot_id, from, to) {
            self.advance_remote_queue_lineage();
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
    pub(super) fn undo_last_queue_edit(&mut self, scope: QueueScope) {
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
                self.advance_remote_queue_lineage();
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
                self.advance_remote_queue_lineage();
            }
        }
        self.set_queue_scope(scope);
    }

    pub(super) fn on_queue_replace_silent(&mut self) {
        self.reset_bare_transitions();
        self.set_queue_source_if_not_local_daemon(crate::config::QueueSource::Unknown);
        self.queue_dirty = false;
    }

    pub(super) fn replace_queue_or_prompt(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action)
            && self.queue_dirty
            && self.queue_is_saved_playlist()
        {
            self.pending_queue_action = Some(action);
            let name = super::ui_util::trunc_str(self.queue_playlist_name(), 36);
            self.ask_confirm(ConfirmModal {
                title: " Unsaved Playlist Changes ".into(),
                message: format!("Save changes to \"{}\"?", name),
                hint: "[s]Save  [d]Discard  [Esc]Cancel".into(),
                on_confirm: ConfirmAction::DiscardOrSaveDirtyPlaylist,
            });
        } else {
            self.execute_pending_queue_action(action);
        }
    }

    /// Design D6 predicate: executing `action` would replace a populated
    /// playback-target queue (local or directly controlled remote), so the
    /// caller must confirm before the replacement runs. An empty target queue
    /// needs no confirmation. Only a `PlayItems` payload is gated; a bare
    /// clear already owns its own confirmation flow.
    pub(super) fn queue_replacement_needs_confirmation(&self, action: &PendingQueueAction) -> bool {
        matches!(action, PendingQueueAction::PlayItems { .. })
            && self.playback_queue().total_queue_len() > 0
    }

    /// Design D6 entry point for a resolved queue replacement: an empty target
    /// queue executes immediately, a populated one stores the complete action
    /// and asks first. Local saved-playlist protection is not part of this
    /// gate; the confirmed execution still runs it through
    /// `replace_queue_or_prompt`.
    ///
    /// The gated payload goes into its own `pending_queue_replacement` slot,
    /// never the save-deferral `pending_queue_action`: only the
    /// `ReplacePopulatedQueue` confirmation arm reads it, so an in-flight
    /// playlist save (whose completion consumes the shared deferral slot)
    /// cannot fire a replacement the user never confirmed.
    pub(super) fn request_queue_replacement(
        &mut self,
        action: PendingQueueAction,
        via: ReplacementExecutor,
    ) {
        if self.queue_replacement_needs_confirmation(&action) {
            self.pending_queue_replacement = Some((action, via));
            self.ask_confirm(ConfirmModal {
                title: " Replace Queue ".into(),
                message: "Replace the current queue?".into(),
                hint: "[y] Confirm    [Esc] Cancel".into(),
                on_confirm: ConfirmAction::ReplacePopulatedQueue,
            });
        } else {
            self.run_replacement(action, via);
        }
    }

    /// Runs one already-confirmed (or gate-free) queue replacement through the
    /// executor its entry point selected. Both the empty-queue path and the
    /// `ReplacePopulatedQueue` confirmation arm call this, so a gated payload
    /// replays exactly what an ungated one would have.
    pub(super) fn run_replacement(&mut self, action: PendingQueueAction, via: ReplacementExecutor) {
        match via {
            ReplacementExecutor::Pending => {
                let playlist_load = matches!(
                    &action,
                    PendingQueueAction::PlayItems {
                        source: crate::config::QueueSource::Playlist { .. },
                        ..
                    }
                );
                self.execute_queue_replacement(action);
                // A playlist load from the Playlists sidebar closes it so the
                // queue it just loaded is visible. Done here rather than at
                // the call site so a gated load still dismisses on confirm
                // while a cancelled one leaves the sidebar alone. A raised
                // save/discard prompt keeps the sidebar (existing behaviour).
                if playlist_load && self.pending_overlay.is_none() {
                    self.request_sidebar_dismiss(SidebarId::Playlists);
                    self.set_panel_focus(PanelFocus::Queue);
                }
            }
            ReplacementExecutor::Routed(prep) => self.run_routed_replacement(action, prep),
        }
    }

    /// Replays one routed entry point's pre-play prep, then runs the shared
    /// `play_items_routed` executor. The prep is the site policy carried by
    /// `ReplacementExecutor::Routed`; `play_items_routed` itself never
    /// replaces the queue.
    fn run_routed_replacement(&mut self, action: PendingQueueAction, prep: RoutedReplacementPrep) {
        let PendingQueueAction::PlayItems {
            items,
            start_idx,
            source,
            autostart: _,
        } = action
        else {
            return;
        };
        match prep {
            RoutedReplacementPrep::Album => {
                self.set_queue_source_if_not_local_daemon(source.clone());
                self.replace_playback_queue(items.clone(), start_idx);
                self.play_items_routed(items, start_idx, source);
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
            }
            RoutedReplacementPrep::MusicAlbums => {
                self.replace_playback_queue(items.clone(), start_idx);
                self.play_items_routed(items, start_idx, source);
                self.save_queue_state();
            }
            RoutedReplacementPrep::Folder => {
                self.replace_playback_queue(items.clone(), start_idx);
                self.set_panel_focus(PanelFocus::Queue);
                self.play_items_routed(items, start_idx, source);
                self.save_queue_state();
            }
            RoutedReplacementPrep::ShuffleFolder => {
                self.replace_playback_queue(items.clone(), start_idx);
                self.set_panel_focus(PanelFocus::Queue);
                self.set_queue_source_if_not_local_daemon(source.clone());
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
                self.play_items_routed(items, start_idx, source);
            }
            RoutedReplacementPrep::Selection => {
                self.rebuild_queue_for_selection(&items, source.clone());
                self.play_items_routed(items, start_idx, source);
            }
        }
    }

    /// Executes one already-resolved replacement through the existing playback
    /// and admission executor. A directly-controlled owner holds the target
    /// queue itself, so the executor's local-metadata gate never stages it or
    /// writes its source label; both happen here, in the same order the
    /// shipped album/artist track paths use (`replace_playback_queue`, then
    /// submission). Local saved-playlist protection still runs through
    /// `replace_queue_or_prompt`, which may raise its own save/discard prompt
    /// as a second step before this payload executes.
    pub(super) fn execute_queue_replacement(&mut self, action: PendingQueueAction) {
        if self.has_direct_remote_queue() {
            if let PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
                ..
            } = &action
            {
                self.queue_source = source.clone();
                self.replace_playback_queue(items.clone(), *start_idx);
            }
        }
        self.replace_queue_or_prompt(action);
    }

    fn clear_remote_queue(&mut self) {
        self.advance_remote_queue_lineage();
        self.player.clear_queue();
        if let Some(queue) = self.remote_player_tab.as_mut() {
            queue.clear();
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
                autostart,
            } => {
                if autostart && self.player.is_remote_disconnected() {
                    self.flash(
                        super::actions::CONNECTION_LOST_MESSAGE.into(),
                        ToastSeverity::Warning,
                    );
                    return;
                }
                let direct_remote = self.has_direct_remote_queue();
                if !autostart && self.stay_alive_owner_is_queue_authority() {
                    let request_id = self.next_owner_queue_load_request;
                    self.next_owner_queue_load_request =
                        self.next_owner_queue_load_request.saturating_add(1);
                    let slots = items
                        .into_iter()
                        .enumerate()
                        .map(|(index, item)| mbv_core::ctrl::UnifiedQueueSlot {
                            slot_id: (index + 1) as u64,
                            item: QueueItem::Emby(Box::new(item)),
                        })
                        .collect();
                    let result = self
                        .player
                        .as_remote()
                        .map(|remote| remote.load_queue_idle(request_id, slots, start_idx, source));
                    match result {
                        Some(Ok(())) => self.set_queue_scope(self.playing_queue_scope()),
                        Some(Err(_)) if self.player.is_remote_disconnected() => self.flash(
                            super::actions::CONNECTION_LOST_MESSAGE.into(),
                            ToastSeverity::Warning,
                        ),
                        Some(Err(reason)) => self.flash(reason, ToastSeverity::Error),
                        None => self.flash(
                            "Could not send idle queue load to Player owner".into(),
                            ToastSeverity::Error,
                        ),
                    }
                    return;
                }
                if self.local_queue_metadata_applies(self.playing_queue_scope()) {
                    self.set_queue_source_if_not_local_daemon(source.clone());
                }
                if !autostart {
                    // Playlist Enter populates the queue; Space/Enter starts it.
                    let loaded = items.get(start_idx).map(|i| i.playback_label());
                    if !direct_remote {
                        self.replace_playback_queue(items, start_idx);
                    }
                    self.set_queue_scope(self.playing_queue_scope());
                    self.persist_local_queue_state_if_needed(self.playing_queue_scope());
                    if let Some(label) = loaded {
                        self.flash(format!("Loaded: {label}"), ToastSeverity::Neutral);
                    }
                    return;
                }
                if !direct_remote {
                    self.replace_playback_queue(items.clone(), start_idx);
                }
                self.set_queue_scope(self.playing_queue_scope());
                if let Some(ref conn_id) = self.connected_session_id.clone() {
                    self.clear_playback_overlays();
                    let id = conn_id.clone();
                    let label = items
                        .get(start_idx)
                        .map(|i| i.playback_label())
                        .unwrap_or_default();
                    self.flash(
                        format!("Requesting playback: {label}"),
                        ToastSeverity::Neutral,
                    );
                    self.submit_attached_sequence(&id, &items, start_idx);
                } else {
                    self.submit_tab_queue(self.playing_queue_scope(), start_idx, source);
                    self.player
                        .send_command(PlayerCommand::SetMute(self.mute_on));
                }
                if !direct_remote {
                    self.save_queue_state();
                }
            }
            PendingQueueAction::ClearQueue => {
                let scope = self.viewed_queue_scope();
                let had_items = self.queue_for_scope(scope).total_queue_len() > 0;
                if self.local_queue_metadata_applies(scope) {
                    self.clear_local_queue_metadata();
                } else {
                    self.remote_queue_undo_stack.clear();
                }
                if scope == QueueScope::Remote && had_items {
                    self.clear_remote_queue();
                } else if self.queue_scope_is_playback(scope) {
                    self.reset_bare_transitions();
                    self.player.stop();
                    if self.stay_alive_owner_is_queue_authority() {
                        self.player.clear_queue();
                    }
                }
                if scope != QueueScope::Remote {
                    let queue = self.queue_for_scope_mut(scope);
                    queue.clear();
                }
                if had_items {
                    self.advance_remote_queue_lineage();
                }
                if self.local_queue_metadata_applies(scope) {
                    self.save_queue_state_after_explicit_clear();
                }
                self.flash("Queue cleared".into(), ToastSeverity::Success);
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
        let owner_queue_lineage = self
            .stay_alive_owner_is_queue_authority()
            .then(|| {
                self.player
                    .as_remote()
                    .and_then(|remote| remote.unified_queue_state())
                    .map(|state| state.lineage)
            })
            .flatten();
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
                owner_queue_lineage,
                item_ids: None,
            },
        );
    }

    fn playlist_mutation_pending(&self, playlist_id: &str) -> bool {
        self.playlist_mutations
            .get(playlist_id)
            .is_some_and(|state| state.active.is_some() || !state.queued.is_empty())
    }

    /// Clears `playlist_item_id` from local queue items after a full playlist
    /// update recreates server entry identities. Every full update
    /// (Save/Replace/CreateAs) pushes this queue to Emby, so those identities
    /// are invalidated.
    pub(super) fn clear_local_playlist_entry_ids(&mut self) {
        let slot_ids: Vec<_> = self
            .player_tab
            .queue
            .slots()
            .iter()
            .filter(|s| matches!(&s.item, QueueItem::Emby(_)))
            .map(|s| s.slot_id)
            .collect();
        for slot_id in slot_ids {
            if let Some(slot) = self.player_tab.queue.slot(slot_id) {
                if let QueueItem::Emby(mut item) = slot.item.clone() {
                    item.playlist_item_id.clear();
                    let _ = self
                        .player_tab
                        .queue
                        .update_slot_item(slot_id, QueueItem::Emby(item));
                }
            }
        }
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
