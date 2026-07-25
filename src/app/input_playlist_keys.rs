use super::{App, PanelFocus, PendingQueueAction, SavePlaylistDialog, SavePlaylistStage};
use crossterm::event::{KeyCode, KeyEvent};
use mbv_core::api::MediaItem;

impl App {
    pub(super) fn handle_key_save_playlist_entry(&mut self, key: KeyEvent) -> Option<bool> {
        if self.save_playlist_dialog.is_some() {
            Some(self.handle_save_playlist_key(key))
        } else {
            None
        }
    }

    pub(super) fn handle_key_save_modal(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.show_save_playlist_modal {
            return None;
        }
        let play_after = matches!(
            self.pending_queue_action,
            Some(PendingQueueAction::PlayItems { .. })
        );
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.save_playlist_to_emby();
                self.show_save_playlist_modal = false;
                if let Some(action) = self.pending_queue_action.take() {
                    self.execute_pending_queue_action(action);
                }
                if play_after {
                    self.show_playlists = false;
                    self.set_panel_focus(PanelFocus::Queue);
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.show_save_playlist_modal = false;
                if let Some(action) = self.pending_queue_action.take() {
                    self.execute_pending_queue_action(action);
                }
                if play_after {
                    self.show_playlists = false;
                    self.set_panel_focus(PanelFocus::Queue);
                }
            }
            KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => {
                self.show_save_playlist_modal = false;
                self.pending_queue_action = None;
            }
            _ => {}
        }
        Some(false)
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
                    let items: Vec<MediaItem> = self
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
                        if !self.show_save_playlist_modal {
                            self.show_playlists = false;
                            self.set_panel_focus(PanelFocus::Queue);
                        }
                    }
                } else if let Some(pl) = self.playlists.get(self.playlists_cursor).cloned() {
                    self.load_and_play_playlist(pl.id);
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
        match &dialog.stage {
            SavePlaylistStage::EnterName => match key.code {
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
                    let playlists = {
                        let c = self.client.lock().unwrap();
                        c.get_playlists().unwrap_or_default()
                    };
                    let existing = playlists
                        .into_iter()
                        .find(|p| p.name.to_lowercase() == name.to_lowercase());
                    if let Some(existing) = existing {
                        self.save_playlist_dialog = Some(SavePlaylistDialog {
                            input: name,
                            stage: SavePlaylistStage::ConfirmOverwrite {
                                existing_id: existing.id,
                            },
                        });
                    } else {
                        let ids: Vec<String> =
                            self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                        let result = {
                            let c = self.client.lock().unwrap();
                            c.create_playlist(&name, &ids)
                        };
                        self.save_playlist_dialog = None;
                        self.force_clear = true;
                        match result {
                            Ok(id) => {
                                self.queue_source = crate::config::QueueSource::Playlist {
                                    id: Some(id),
                                    name: name.clone(),
                                };
                                self.queue_dirty = false;
                                self.save_queue_state();
                                self.flash_status(format!("Saved as playlist \"{name}\""));
                            }
                            Err(e) => self.flash_status_high(format!("Error: {e}")),
                        }
                    }
                }
                _ => {}
            },
            SavePlaylistStage::ConfirmOverwrite { existing_id } => {
                let existing_id = existing_id.clone();
                match key.code {
                    KeyCode::Char('y') => {
                        let name = dialog.input.clone();
                        let ids: Vec<String> =
                            self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                        let result = {
                            let c = self.client.lock().unwrap();
                            c.delete_playlist(&existing_id)
                                .and_then(|_| c.create_playlist(&name, &ids))
                        };
                        self.save_playlist_dialog = None;
                        self.force_clear = true;
                        match result {
                            Ok(id) => {
                                self.queue_source = crate::config::QueueSource::Playlist {
                                    id: Some(id),
                                    name: name.clone(),
                                };
                                self.queue_dirty = false;
                                self.flash_status(format!("Saved as playlist \"{name}\""));
                            }
                            Err(e) => self.flash_status_high(format!("Error: {e}")),
                        }
                    }
                    KeyCode::Esc => {
                        let input = dialog.input.clone();
                        self.save_playlist_dialog = Some(SavePlaylistDialog {
                            input,
                            stage: SavePlaylistStage::EnterName,
                        });
                    }
                    _ => {}
                }
            }
        }
        false
    }
}
