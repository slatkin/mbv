use super::types_playback::PlaylistMutation;
use super::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, SavePlaylistDialog,
    SavePlaylistStage,
};
use crossterm::event::{KeyCode, KeyEvent};
use mbv_core::api::EmbyItem;

impl App {
    pub(super) fn handle_key_save_playlist_entry(&mut self, key: KeyEvent) -> Option<bool> {
        let is_entry_stage = matches!(
            self.save_playlist_dialog.as_ref().map(|d| &d.stage),
            Some(SavePlaylistStage::EnterName | SavePlaylistStage::RenamePlaylist { .. })
        );
        if is_entry_stage {
            Some(self.handle_save_playlist_key(key))
        } else {
            None
        }
    }

    pub(super) fn handle_key_playlists(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.show_playlists {
            return None;
        }
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                return Some(self.try_quit());
            }
            KeyCode::Esc | KeyCode::F(4) => {
                if self.playlists_open.is_some() {
                    self.playlists_open = None;
                    self.playlists_open_items = Vec::new();
                } else {
                    self.show_playlists = false;
                }
            }
            KeyCode::Backspace => {
                if self.playlists_open.is_some() {
                    self.playlists_open = None;
                    self.playlists_open_items = Vec::new();
                }
            }
            KeyCode::F(1) => {
                self.show_playlists = false;
                self.show_help = true;
            }
            KeyCode::F(2) => {
                self.show_playlists = false;
                self.show_settings = true;
            }
            KeyCode::F(3) => {
                self.show_playlists = false;
                self.show_sessions = true;
            }
            KeyCode::Up => {
                if self.playlists_open.is_some() {
                    if self.playlists_open_cursor > 0 {
                        self.playlists_open_cursor -= 1;
                    }
                } else if self.playlists_cursor > 0 {
                    self.playlists_cursor -= 1;
                }
            }
            KeyCode::Down => {
                if self.playlists_open.is_some() {
                    if !self.playlists_open_items.is_empty() {
                        self.playlists_open_cursor = (self.playlists_open_cursor + 1)
                            .min(self.playlists_open_items.len() - 1);
                    }
                } else if !self.playlists.is_empty() {
                    self.playlists_cursor =
                        (self.playlists_cursor + 1).min(self.playlists.len() - 1);
                }
            }
            KeyCode::PageUp => {
                let page = (self.terminal_height as usize).saturating_sub(4);
                if self.playlists_open.is_some() {
                    self.playlists_open_cursor = self.playlists_open_cursor.saturating_sub(page);
                } else {
                    self.playlists_cursor = self.playlists_cursor.saturating_sub(page);
                }
            }
            KeyCode::PageDown => {
                let page = (self.terminal_height as usize).saturating_sub(4);
                if self.playlists_open.is_some() {
                    if !self.playlists_open_items.is_empty() {
                        self.playlists_open_cursor = (self.playlists_open_cursor + page)
                            .min(self.playlists_open_items.len() - 1);
                    }
                } else if !self.playlists.is_empty() {
                    self.playlists_cursor =
                        (self.playlists_cursor + page).min(self.playlists.len() - 1);
                }
            }
            KeyCode::Home => {
                if self.playlists_open.is_some() {
                    self.playlists_open_cursor = 0;
                } else {
                    self.playlists_cursor = 0;
                }
            }
            KeyCode::End => {
                if self.playlists_open.is_some() {
                    self.playlists_open_cursor = self.playlists_open_items.len().saturating_sub(1);
                } else {
                    self.playlists_cursor = self.playlists.len().saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if self.playlists_open.is_none() {
                    if let Some(pl) = self.playlists.get(self.playlists_cursor).cloned() {
                        self.spawn_open_playlist(pl);
                    }
                }
            }
            KeyCode::Left => {
                if self.playlists_open.is_some() {
                    self.playlists_open = None;
                    self.playlists_open_items = Vec::new();
                }
            }
            KeyCode::Enter => {
                if self.playlists_open.is_some() {
                    let selected_id = self
                        .playlists_open_items
                        .get(self.playlists_open_cursor)
                        .map(|i| i.id.clone());
                    let pl_source = crate::config::QueueSource::Playlist {
                        id: self.playlists_open.as_ref().map(|p| p.id.clone()),
                        name: self
                            .playlists_open
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_default(),
                    };
                    let items: Vec<EmbyItem> = self
                        .playlists_open_items
                        .iter()
                        .filter(|i| !i.is_folder)
                        .cloned()
                        .collect();
                    if !items.is_empty() {
                        let start = selected_id
                            .as_deref()
                            .and_then(|id| items.iter().position(|i| i.id == id))
                            .unwrap_or(0);
                        let action = PendingQueueAction::PlayItems {
                            items,
                            start_idx: start,
                            source: pl_source,
                        };
                        self.replace_queue_or_prompt(action);
                        if self.confirm_modal.is_none() {
                            self.show_playlists = false;
                            self.set_panel_focus(PanelFocus::Queue);
                        }
                    }
                } else if let Some(pl) = self.playlists.get(self.playlists_cursor).cloned() {
                    self.load_and_play_playlist(pl.id);
                }
            }
            KeyCode::Char('n') if key.modifiers.is_empty() && self.playlists_open.is_none() => {
                if let Some(pl) = self.playlists.get(self.playlists_cursor).cloned() {
                    self.save_playlist_dialog = Some(SavePlaylistDialog {
                        input: pl.name,
                        stage: SavePlaylistStage::RenamePlaylist { id: pl.id },
                    });
                }
            }
            KeyCode::Char('d') if key.modifiers.is_empty() && self.playlists_open.is_none() => {
                if let Some(pl) = self.playlists.get(self.playlists_cursor).cloned() {
                    self.confirm_modal = Some(ConfirmModal {
                        title: " Delete Playlist ".into(),
                        message: format!(
                            "Delete playlist '{}'?",
                            super::ui_util::trunc_str(&pl.name, 40)
                        ),
                        hint: "[y] Confirm    [Esc] Cancel".into(),
                        on_confirm: ConfirmAction::DeletePlaylist {
                            id: pl.id,
                            name: pl.name,
                        },
                    });
                }
            }
            KeyCode::Char('r') => {
                if self.playlists_open.is_some() {
                    if let Some(pl) = self.playlists_open.clone() {
                        self.playlists_open = None;
                        self.spawn_open_playlist(pl);
                    }
                } else {
                    self.spawn_load_playlists();
                }
            }
            _ => {}
        }
        Some(false)
    }

    fn handle_save_playlist_key(&mut self, key: KeyEvent) -> bool {
        let Some(ref dialog) = self.save_playlist_dialog else {
            return false;
        };
        // Only `EnterName` and `RenamePlaylist` are handled here; once a name
        // collides with an existing playlist, the shared confirmation-modal
        // dispatcher handles the overwrite decision.
        if !matches!(
            dialog.stage,
            SavePlaylistStage::EnterName | SavePlaylistStage::RenamePlaylist { .. }
        ) {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.save_playlist_dialog = None;
                self.force_clear = true;
            }
            KeyCode::Backspace => {
                if let Some(d) = &mut self.save_playlist_dialog {
                    d.input.pop();
                }
            }
            KeyCode::Char(c)
                if key.modifiers == crossterm::event::KeyModifiers::NONE
                    || key.modifiers == crossterm::event::KeyModifiers::SHIFT =>
            {
                if let Some(d) = &mut self.save_playlist_dialog {
                    d.input.push(c);
                }
            }
            KeyCode::Enter => {
                let name = dialog.input.trim().to_string();
                if name.is_empty() {
                    return false;
                }
                if let SavePlaylistStage::RenamePlaylist { ref id } = dialog.stage {
                    let id = id.clone();
                    self.save_playlist_dialog = None;
                    self.force_clear = true;
                    self.spawn_rename_playlist(id, name);
                    return false;
                }
                let playlists = {
                    let c = self.client.lock().unwrap();
                    c.get_playlists().unwrap_or_default()
                };
                let existing = playlists
                    .into_iter()
                    .find(|p| p.name.to_lowercase() == name.to_lowercase());
                if let Some(existing) = existing {
                    self.save_playlist_dialog = None;
                    self.confirm_modal = Some(ConfirmModal {
                        title: " Overwrite Playlist ".into(),
                        message: format!(
                            "\"{}\" already exists.",
                            super::ui_util::trunc_str(&name, 40)
                        ),
                        hint: "[y] Overwrite    [Esc] Back".into(),
                        on_confirm: ConfirmAction::SaveOverwritePlaylist {
                            existing_id: existing.id,
                            name,
                        },
                    });
                } else {
                    self.save_playlist_dialog = None;
                    self.force_clear = true;
                    self.save_queue_as_playlist(name);
                }
            }
            _ => {}
        }
        false
    }

    /// Effect for `ConfirmAction::SaveOverwritePlaylist`'s "yes" answer
    /// (`y`): deletes the existing playlist and recreates it under the same
    /// name with the current queue's items. Extracted from the old
    /// `SavePlaylistStage::ConfirmOverwrite` key handler so the shared
    /// confirmation-modal dispatcher can call it directly.
    pub(super) fn do_overwrite_playlist(&mut self, existing_id: &str, name: &str) {
        self.save_playlist_dialog = None;
        self.force_clear = true;
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        self.enqueue_playlist_mutation(
            existing_id.to_string(),
            PlaylistMutation::Replace {
                mutation_id,
                queue_lineage: self.remote_queue_lineage,
                source_playlist_id: existing_id.to_string(),
                name: name.to_string(),
                item_ids: None,
            },
        );
    }
}
