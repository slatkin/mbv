use super::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, QueueScope,
    SavePlaylistDialog, SavePlaylistStage,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::player::PlayerCommand;

impl App {
    /// Shared dispatcher for the confirmation-modal component (see
    /// `render/overlays/confirm_modal.rs`, `types_confirm.rs`): matches on
    /// which `ConfirmAction` is pending and re-uses each action's existing
    /// effect, preserving the exact key bindings each confirmation had
    /// before migrating off status-bar toast text / bespoke dialogs.
    pub(super) fn handle_key_confirm_modal(&mut self, key: KeyEvent) -> Option<bool> {
        let action = self.confirm_modal.as_ref()?.on_confirm.clone();
        match action {
            ConfirmAction::ClearQueue => {
                self.confirm_modal = None;
                if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
                }
            }
            ConfirmAction::RemoveActiveQueueItem(pos) => {
                self.confirm_modal = None;
                if matches!(key.code, KeyCode::Char('y')) {
                    // Defer the actual removal until PlayerEvent::Stopped arrives so the
                    // Stopped handler finds the correct item at index `pos`, not the next
                    // item (which would have its playback_position_ticks corrupted otherwise).
                    self.pending_delete_idx = Some(pos);
                    self.player.stop();
                    if self.local_queue_metadata_applies(self.visible_queue_scope()) {
                        self.queue_dirty = true;
                    }
                }
            }
            ConfirmAction::RescanLibrary(lib_idx) => {
                self.confirm_modal = None;
                if matches!(
                    key.code,
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
                ) {
                    self.trigger_lib_rescan(lib_idx);
                }
            }
            ConfirmAction::SaveOverwritePlaylist { existing_id, name } => match key.code {
                KeyCode::Char('y') => {
                    self.confirm_modal = None;
                    self.do_overwrite_playlist(&existing_id, &name);
                }
                KeyCode::Esc => {
                    self.confirm_modal = None;
                    self.save_playlist_dialog = Some(SavePlaylistDialog {
                        input: name,
                        stage: SavePlaylistStage::EnterName,
                    });
                }
                _ => {}
            },
            ConfirmAction::DeletePlaylist { id, name } => match key.code {
                KeyCode::Char('y') => {
                    self.confirm_modal = None;
                    self.spawn_delete_playlist(id, name);
                }
                KeyCode::Esc => {
                    self.confirm_modal = None;
                }
                _ => {}
            },
            ConfirmAction::DiscardOrSaveDirtyPlaylist => {
                let play_after = matches!(
                    self.pending_queue_action,
                    Some(PendingQueueAction::PlayItems { .. })
                );
                match key.code {
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        self.confirm_modal = None;
                        self.save_playlist_to_emby();
                        if let Some(action) = self.pending_queue_action.take() {
                            self.execute_pending_queue_action(action);
                        }
                        if play_after {
                            self.show_playlists = false;
                            self.set_panel_focus(PanelFocus::Queue);
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        self.confirm_modal = None;
                        if let Some(action) = self.pending_queue_action.take() {
                            self.execute_pending_queue_action(action);
                        }
                        if play_after {
                            self.show_playlists = false;
                            self.set_panel_focus(PanelFocus::Queue);
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.confirm_modal = None;
                        self.pending_queue_action = None;
                    }
                    _ => {}
                }
            }
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_skip_intro(&mut self, key: KeyEvent) -> Option<bool> {
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

    pub(super) fn handle_key_confirm_next_up(&mut self, key: KeyEvent) -> Option<bool> {
        self.next_up_item.as_ref()?;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(item) = self.next_up_item.take() {
                if let Some(idx) = self
                    .playback_queue()
                    .items
                    .iter()
                    .position(|i| i.id == item.id)
                {
                    let label = item.playback_label();
                    self.player.send_command(PlayerCommand::JumpTo(idx));
                    self.playback_queue_mut().queue_cursor = idx;
                    self.flash_status(label);
                }
            }
        } else {
            self.next_up_item = None;
            self.player.send_command(PlayerCommand::NextUpDismiss);
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_clear_queue_prompt(&mut self, key: KeyEvent) -> Option<bool> {
        // Behavior change (phase 6, #135): gate on an open context menu. Before
        // this fix, `clear_queue_prompt_c` sat above `context_menu` in
        // CONTEXT_STACK with no guard, so pressing 'c' while a context menu was
        // open silently opened the clear-queue confirmation instead of being
        // swallowed by the menu (which has no 'c' binding of its own). See
        // docs/adr/0002-centralized-input-handling.md phase 6 and phase-2's
        // `home_search`, which already guards the same way.
        if key.code != KeyCode::Char('c')
            || key.modifiers.contains(KeyModifiers::ALT)
            || self.context_menu_open()
        {
            return None;
        }
        let in_lib_search = self.library_tab > 0
            && self
                .libs
                .get(self.library_tab - 1)
                .is_some_and(|l| l.search.is_some());
        if in_lib_search {
            return None;
        }
        if matches!(self.panel_focus, PanelFocus::Queue)
            && self.visible_queue_scope() == QueueScope::Remote
        {
            self.flash_status_high("Remote queue is controlled by the daemon".into());
            return Some(false);
        }
        if self.player_tab.items.is_empty() {
            return Some(false);
        }
        self.notify_with_actions(
            "mbv",
            "Clear queue?",
            &[("clear:yes", "Clear"), ("clear:no", "Cancel")],
        );
        self.confirm_modal = Some(ConfirmModal {
            title: " Clear Queue ".into(),
            message: "Clear the queue?".into(),
            hint: "[y] Confirm    [Esc] Cancel".into(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        Some(false)
    }
}
