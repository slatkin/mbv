use super::notify_actions::ToastSeverity;
use super::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, QueueScope,
    SavePlaylistDialog, SavePlaylistStage, SidebarId, UndoEntry,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::playback_queue::RemoveSlotResult;
use mbv_core::player::PlayerCommand;

impl App {
    /// Shared dispatcher for the confirmation-modal component (see
    /// `render/overlays/confirm_modal.rs`, `types_confirm.rs`): matches on
    /// which `ConfirmAction` is pending and re-uses each action's existing
    /// effect, preserving the exact key bindings each confirmation had
    /// before migrating off status-bar toast text / bespoke dialogs.
    pub(super) fn apply_confirm_action(
        &mut self,
        action: ConfirmAction,
        key: KeyEvent,
    ) -> Option<bool> {
        match action {
            ConfirmAction::ClearQueue => {
                if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
                }
            }
            ConfirmAction::RemoveActiveQueueItem(pos) => {
                if matches!(key.code, KeyCode::Char('y')) {
                    // Remove the slot from the queue model immediately, rather
                    // than deferring it until PlayerEvent::Stopped arrives back
                    // from the player thread — that round trip (send stop() ->
                    // mpv actually stops -> event travels back) is a real,
                    // noticeable delay for something the user should see happen
                    // right away. The gated remove_slot/remove_slot_at (used by
                    // ordinary removals) refuses the active slot, so this goes
                    // through remove_active_slot_confirmed directly, same as
                    // the Stopped handler used to do.
                    let scope = self.visible_queue_scope();
                    let slot_id = self.queue_for_scope_mut(scope).slot_id_at(pos);
                    if let Some(slot_id) = slot_id {
                        let removed_item = match self
                            .playback_queue_mut()
                            .queue
                            .remove_active_slot_confirmed(slot_id)
                        {
                            RemoveSlotResult::Removed(slot) => {
                                self.playback_queue_mut().clamp_cursor();
                                Some(slot.item)
                            }
                            RemoveSlotResult::RequiresActiveConfirmation(_)
                            | RemoveSlotResult::NotFound => None,
                        };
                        if let Some(item) = removed_item {
                            let queue = self.playback_queue_mut();
                            queue.clamp_cursor();
                            if !self.player.is_remote() {
                                self.queue_undo_stack.push(UndoEntry::Remove(pos, item));
                            }

                            // Only stop playback and record the deferred
                            // player-side removal if the app model actually
                            // removed the requested slot. The confirmation
                            // can outlive a queue refresh, making the slot
                            // stale by the time the user confirms it.
                            self.pending_delete_slot = Some(slot_id);
                            if self.connected_session_id.is_some() {
                                self.playback_target().stop(self);
                            } else {
                                self.player.stop();
                            }
                            if self.local_queue_metadata_applies(scope) {
                                self.queue_dirty = true;
                            }
                            self.retire_remote_tracking(true);
                        }
                    }
                }
            }
            ConfirmAction::RescanLibrary(lib_idx) => {
                if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.trigger_lib_rescan(lib_idx);
                }
            }
            ConfirmAction::SaveOverwritePlaylist { existing_id, name } => match key.code {
                KeyCode::Char('y') => {
                    self.do_overwrite_playlist(&existing_id, &name);
                }
                KeyCode::Esc => {
                    self.open_save_playlist_dialog(SavePlaylistDialog {
                        input: name,
                        stage: SavePlaylistStage::EnterName,
                    });
                }
                _ => {}
            },
            ConfirmAction::DeletePlaylist { id, name } => match key.code {
                KeyCode::Char('y') => {
                    self.spawn_delete_playlist(id, name);
                }
                KeyCode::Esc => {}
                _ => {}
            },
            ConfirmAction::RemoveFeedSubscription(index) => {
                if matches!(key.code, KeyCode::Char('y')) {
                    self.remove_feed_confirmed(index);
                }
            }
            ConfirmAction::RemoveEmby => {
                if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.remove_emby_confirmed();
                } else if key.code == KeyCode::Esc {
                }
            }
            ConfirmAction::ReplaceEmby(generation) => {
                if key.code == KeyCode::Esc {
                    self.pending_emby_replacement = None;
                } else if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.replace_emby_confirmed(generation);
                }
            }
            ConfirmAction::RemoveAudiobookshelf => {
                if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.remove_audiobookshelf_confirmed();
                } else if key.code == KeyCode::Esc {
                }
            }
            ConfirmAction::ReplaceAudiobookshelf(generation) => {
                if key.code == KeyCode::Esc {
                    self.pending_audiobookshelf_replacement = None;
                } else if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.replace_audiobookshelf_confirmed(generation);
                }
            }
            ConfirmAction::DiscardOrSaveDirtyPlaylist => {
                let play_after = matches!(
                    self.pending_queue_action,
                    Some(PendingQueueAction::PlayItems { .. })
                );
                match key.code {
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        self.save_playlist_to_emby();
                        // Keep the replacement queued until the coordinator
                        // reports that this save crossed its mutation boundary.
                        // Starting it here could snapshot/recreate the same
                        // playlist before the save has executed.
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        if let Some(action) = self.pending_queue_action.take() {
                            self.execute_pending_queue_action(action);
                        }
                        if play_after {
                            self.request_sidebar_dismiss(SidebarId::Playlists);
                            self.set_panel_focus(PanelFocus::Queue);
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.pending_queue_action = None;
                    }
                    _ => {}
                }
            }
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_skip_intro(
        &mut self,
        key: KeyEvent,
        _home_cw_selected: bool,
    ) -> Option<bool> {
        self.skip_intro_end_ticks?;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                let secs = end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
                self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                self.player.send_command(PlayerCommand::SkipIntroDismiss);
                self.status.clear();
            }
        } else {
            self.skip_intro_end_ticks = None;
            self.player.send_command(PlayerCommand::SkipIntroDismiss);
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_next_up(
        &mut self,
        key: KeyEvent,
        _home_cw_selected: bool,
    ) -> Option<bool> {
        self.next_up_item.as_ref()?;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(item) = self.next_up_item.take() {
                if let Some(idx) = self
                    .playback_queue()
                    .slots()
                    .iter()
                    .position(|s| matches!(&s.item, mbv_core::playback_queue::QueueItem::Emby(e) if e.id == item.id))
                {
                    let label = item.playback_label();
                    self.player.send_command(PlayerCommand::JumpTo(idx));
                    self.playback_queue_mut().queue_cursor = idx;
                    self.flash(label, ToastSeverity::Success);
                }
            }
        } else {
            self.next_up_item = None;
            self.player.send_command(PlayerCommand::NextUpDismiss);
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_clear_queue_prompt(
        &mut self,
        key: KeyEvent,
        _home_cw_selected: bool,
    ) -> Option<bool> {
        // Behavior change (phase 6, #135): gate on an open context menu. Before
        // this fix, `clear_queue_prompt_c` sat above `context_menu` in
        // CONTEXT_STACK with no guard, so pressing 'c' while a context menu was
        // open silently opened the clear-queue confirmation instead of being
        // swallowed by the menu (which has no 'c' binding of its own). See
        // docs/adr/0002-centralized-input-handling.md phase 6.
        if key.code != KeyCode::Char('c') || key.modifiers.contains(KeyModifiers::ALT) {
            return None;
        }
        if matches!(self.effective_panel_focus(), PanelFocus::Queue)
            && self.visible_queue_scope() == QueueScope::Remote
        {
            self.flash(
                "Remote queue is controlled by the daemon".into(),
                ToastSeverity::Error,
            );
            return Some(false);
        }
        if self.player_tab.total_queue_len() == 0 {
            return Some(false);
        }
        self.notify_with_actions(
            "mbv",
            "Clear queue?",
            &[("clear:yes", "Clear"), ("clear:no", "Cancel")],
        );
        self.ask_confirm(ConfirmModal {
            title: " Clear Queue ".into(),
            message: "Clear the queue?".into(),
            hint: "[y] Confirm    [Esc] Cancel".into(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        Some(false)
    }
}
