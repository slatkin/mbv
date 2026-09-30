use crate::app::SidebarId;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, QueueScope,
    SavePlaylistDialog, SavePlaylistStage,
};
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    /// Shared dispatcher for the confirmation modal: matches on which
    /// `ConfirmAction` is pending and re-uses each action's existing effect.
    /// Only Enter (and `d`/Esc on the dirty-playlist prompt) answers; every
    /// other key is a no-op that leaves the modal mounted.
    pub(in crate::app) fn apply_confirm_action(
        &mut self,
        action: ConfirmAction,
        key: KeyEvent,
    ) -> bool {
        match action {
            ConfirmAction::ClearQueue => self.confirm_clear_queue(key),
            ConfirmAction::RemoveActiveQueueItem(pos) => {
                self.confirm_remove_active_queue_item_for_key(pos, key);
            }
            ConfirmAction::RescanLibrary(lib_idx) => self.confirm_rescan_library(lib_idx, key),
            ConfirmAction::SaveOverwritePlaylist { existing_id, name } => {
                self.confirm_save_overwrite_playlist(&existing_id, name, key);
            }
            ConfirmAction::DeletePlaylist { id, name } => {
                self.confirm_delete_playlist(id, name, key);
            }
            ConfirmAction::RemoveFeedSubscription(index) => {
                self.confirm_remove_feed_subscription(index, key);
            }
            ConfirmAction::RemoveEmby => self.confirm_remove_emby(key),
            ConfirmAction::ReplaceEmby(generation) => self.confirm_replace_emby(generation, key),
            ConfirmAction::RemoveAudiobookshelf => self.confirm_remove_audiobookshelf(key),
            ConfirmAction::ReplaceAudiobookshelf(generation) => {
                self.confirm_replace_audiobookshelf(generation, key);
            }
            ConfirmAction::PlayLocallyInstead => self.confirm_play_locally(key),
            ConfirmAction::DiscardOrSaveDirtyPlaylist => {
                self.confirm_discard_or_save_dirty_playlist(key);
            }
            // Only this arm calls `QueueDeferrals::take_confirmed_replacement`;
            // save completion calls `take_on_save_complete` for its bound mutation.
            ConfirmAction::ReplacePopulatedQueue => {
                self.confirm_replace_populated_queue(key);
            }
        }
        false
    }

    fn confirm_clear_queue(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
        }
    }

    fn confirm_remove_active_queue_item_for_key(&mut self, pos: usize, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.confirm_remove_active_queue_item(pos);
        }
    }

    fn confirm_rescan_library(&mut self, lib_idx: usize, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.trigger_lib_rescan(lib_idx);
        }
    }

    fn confirm_save_overwrite_playlist(&mut self, existing_id: &str, name: String, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.do_overwrite_playlist(existing_id, &name),
            KeyCode::Esc => self.open_save_playlist_dialog(SavePlaylistDialog {
                input: name,
                stage: SavePlaylistStage::EnterName,
            }),
            _ => {}
        }
    }

    fn confirm_delete_playlist(&mut self, id: String, name: String, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.spawn_delete_playlist(id, name);
        }
    }

    fn confirm_remove_feed_subscription(&mut self, index: usize, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.remove_feed_confirmed(index);
        }
    }

    fn confirm_remove_emby(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.remove_emby_confirmed();
        }
    }

    fn confirm_replace_emby(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        key: KeyEvent,
    ) {
        if key.code == KeyCode::Esc {
            self.setup.pending_emby_replacement = None;
        } else if key.code == KeyCode::Enter {
            self.replace_emby_confirmed(generation);
        }
    }

    fn confirm_remove_audiobookshelf(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Enter {
            self.remove_audiobookshelf_confirmed();
        }
    }

    fn confirm_replace_audiobookshelf(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        key: KeyEvent,
    ) {
        if key.code == KeyCode::Esc {
            self.setup.pending_audiobookshelf_replacement = None;
        } else if key.code == KeyCode::Enter {
            self.replace_audiobookshelf_confirmed(generation);
        }
    }

    fn confirm_play_locally(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.play_pending_local_play();
            }
            KeyCode::Esc => {
                self.queue_deferrals.cancel_local_play();
            }
            _ => {}
        }
    }

    /// Enter saves the dirty playlist, `[d]` discards it and runs the pending
    /// queue action (restoring the Queue panel and dismissing the playlists
    /// sidebar when that action was a play), and Esc cancels. The shell
    /// receives `ConfirmIntent::Save`/`Discard`/`Cancel`, translated to
    /// `s`/`d`/Esc in `Model::handle_confirm_intent`.
    fn confirm_discard_or_save_dirty_playlist(&mut self, key: KeyEvent) {
        let play_after = self.queue_deferrals.save_answer_is_play();
        match key.code {
            KeyCode::Char('s' | 'S') => {
                let mutation_id = self.save_playlist_to_emby();
                self.queue_deferrals.bind_to_save(mutation_id);
            }
            KeyCode::Char('d' | 'D') => {
                if let Some(action) = self.queue_deferrals.take_on_discard() {
                    self.execute_pending_queue_action(action);
                }
                if play_after {
                    self.request_sidebar_dismiss(SidebarId::Playlists);
                    self.set_panel_focus(PanelFocus::Queue);
                }
            }
            KeyCode::Esc => {
                self.queue_deferrals.cancel_save_answer();
            }
            _ => {}
        }
    }

    /// Confirming the populated-queue gate hands the stored payload back to
    /// the one queue-replacement executor; every other key cancels, so no
    /// executable payload can fire at a later step.
    fn confirm_replace_populated_queue(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Some((action, via)) = self.queue_deferrals.take_confirmed_replacement() {
                    self.run_replacement(action, &via);
                }
            }
            _ => {
                self.queue_deferrals.cancel_gated_replacement();
            }
        }
    }

    /// Confirmed removal of the active queue item at `pos`: leave the
    /// displayed queue untouched, stop playback, then remove the slot through
    /// the Player owner from the resulting `Stopped` event.
    fn confirm_remove_active_queue_item(&mut self, pos: usize) {
        let scope = self.viewed_queue_scope();
        let slot_id = self.queue_for_scope_mut(scope).slot_id_at(pos);
        if let Some(slot_id) = slot_id {
            self.pending_delete_slot = Some(slot_id);
            if self.connected_session_id.is_some() {
                self.playback_target().stop(self);
            } else {
                self.player.stop();
            }
            if self.local_queue_metadata_applies(scope) {
                self.queue_dirty = true;
            }
            self.advance_queue_epoch();
        }
    }

    /// Show the clear-queue confirmation modal (called from `QueueIntent::Clear`).
    pub(in crate::app) fn request_clear_queue(&mut self) {
        let scope = self.viewed_queue_scope();
        // Legacy `handle_key_clear_queue_prompt` refused a Queue-focused remote
        // scope outright, which also swallowed `c` for a socket-attached mbvd
        // whose queue the confirm's `y` handler can clear. Narrow the refusal to
        // a connected Emby session (queue owned on the remote device); the
        // direct-remote daemon queue falls through to the same prompt a local
        // queue gets.
        if scope == QueueScope::Remote && self.connected_session_id.is_some() {
            self.flash(
                "Remote queue is controlled by the daemon".into(),
                ToastSeverity::Error,
            );
            return;
        }
        if self.queue_for_scope(scope).total_queue_len() == 0 {
            return;
        }
        self.ask_confirm(ConfirmModal::two_button(
            " Clear Queue ".into(),
            "Clear the queue?".into(),
            "Confirm",
            "Cancel",
            ConfirmAction::ClearQueue,
        ));
    }
}

#[cfg(test)]
mod tests;
