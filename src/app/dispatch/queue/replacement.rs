use super::{
    App, ConfirmAction, ConfirmModal, EmbyItem, LocalQueueOwner, PanelFocus, PendingQueueAction,
    PlayerCommand, QueueItem, QueueScope, ReplacementExecutor, RoutedReplacementPrep, SidebarId,
    ToastSeverity,
};

impl App {
    pub(in crate::app) fn on_queue_replace_silent(&mut self) {
        self.reset_bare_transitions();
        self.set_queue_source_if_not_local_daemon(crate::config::QueueSource::Unknown);
        self.queue_dirty = false;
    }

    pub(in crate::app) fn replace_queue_or_prompt(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action)
            && self.queue_dirty
            && self.queue_is_saved_playlist()
        {
            self.pending_queue_action = Some(action);
            let name = crate::app::infra::ui_util::trunc_str(self.queue_playlist_name(), 36);
            self.ask_confirm(ConfirmModal {
                title: " Unsaved Playlist Changes ".into(),
                message: format!("Save changes to \"{name}\"?"),
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
    pub(in crate::app) fn queue_replacement_needs_confirmation(
        &self,
        action: &PendingQueueAction,
    ) -> bool {
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
    pub(in crate::app) fn request_queue_replacement(
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
            self.run_replacement(action, &via);
        }
    }

    /// Runs one already-confirmed (or gate-free) queue replacement through the
    /// executor its entry point selected. Both the empty-queue path and the
    /// `ReplacePopulatedQueue` confirmation arm call this, so a gated payload
    /// replays exactly what an ungated one would have.
    pub(in crate::app) fn run_replacement(
        &mut self,
        action: PendingQueueAction,
        via: &ReplacementExecutor,
    ) {
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
            ReplacementExecutor::Routed(prep) => self.run_routed_replacement(action, *prep),
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
            ..
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
    pub(in crate::app) fn execute_queue_replacement(&mut self, action: PendingQueueAction) {
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
        self.advance_queue_epoch();
        self.player.clear_queue();
        if let Some(queue) = self.remote_player_tab.as_mut() {
            queue.clear();
        }
    }

    pub(in crate::app) fn execute_pending_queue_action(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action) {
            self.queue_dirty = false;
        }
        match action {
            PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
                autostart,
            } => self.execute_pending_play_items(items, start_idx, source, autostart),
            PendingQueueAction::ClearQueue => self.execute_pending_queue_clear(),
        }
    }

    fn execute_pending_play_items(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        autostart: bool,
    ) {
        if autostart && self.player.is_remote_disconnected() {
            self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            );
            return;
        }
        let direct_remote = self.has_direct_remote_queue();
        if !autostart && self.local_queue_owner() == LocalQueueOwner::StayAlive {
            self.load_idle_queue_on_owner(items, start_idx, source);
            return;
        }
        if self.local_queue_metadata_applies(self.playing_queue_scope()) {
            self.set_queue_source_if_not_local_daemon(source.clone());
        }
        if autostart {
            self.start_pending_queue_playback(&items, start_idx, source, direct_remote);
        } else {
            self.load_pending_queue_locally(&items, start_idx, direct_remote);
        }
    }

    fn load_idle_queue_on_owner(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
    ) {
        let request_id = self.next_owner_queue_load_request;
        self.next_owner_queue_load_request = self.next_owner_queue_load_request.saturating_add(1);
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
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            ),
            Some(Err(reason)) => self.flash(reason, ToastSeverity::Error),
            None => self.flash(
                "Could not send idle queue load to Player owner".into(),
                ToastSeverity::Error,
            ),
        }
    }

    fn load_pending_queue_locally(
        &mut self,
        items: &[EmbyItem],
        start_idx: usize,
        direct_remote: bool,
    ) {
        // Playlist Enter populates the queue; Space/Enter starts it.
        let loaded = items
            .get(start_idx)
            .map(mbv_core::api::EmbyItem::playback_label);
        if !direct_remote {
            self.replace_playback_queue(items.to_vec(), start_idx);
        }
        self.set_queue_scope(self.playing_queue_scope());
        self.persist_local_queue_state_if_needed(self.playing_queue_scope());
        if let Some(label) = loaded {
            self.flash(format!("Loaded: {label}"), ToastSeverity::Neutral);
        }
    }

    fn start_pending_queue_playback(
        &mut self,
        items: &[EmbyItem],
        start_idx: usize,
        source: crate::config::QueueSource,
        direct_remote: bool,
    ) {
        if !direct_remote {
            self.replace_playback_queue(items.to_vec(), start_idx);
        }
        self.set_queue_scope(self.playing_queue_scope());
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let label = items
                .get(start_idx)
                .map(mbv_core::api::EmbyItem::playback_label)
                .unwrap_or_default();
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.submit_attached_sequence(&id, items, start_idx);
        } else {
            self.submit_tab_queue(self.playing_queue_scope(), start_idx, source);
            self.player
                .send_command(PlayerCommand::SetMute(self.mute_on));
        }
        if !direct_remote {
            self.save_queue_state();
        }
    }

    fn execute_pending_queue_clear(&mut self) {
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
            if self.local_queue_owner() == LocalQueueOwner::StayAlive {
                self.player.clear_queue();
            }
        }
        if scope != QueueScope::Remote {
            let queue = self.queue_for_scope_mut(scope);
            queue.clear();
        }
        if had_items {
            self.advance_queue_epoch();
        }
        if self.local_queue_metadata_applies(scope) {
            self.save_queue_state_after_explicit_clear();
        }
        self.flash("Queue cleared".into(), ToastSeverity::Success);
    }
}
