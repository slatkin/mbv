#![allow(unused_imports)]

use crate::app::action::Command;
use crate::app::layout::LibraryRowTarget;
use crate::app::{
    App, PanelFocus, PendingQueueAction, QueueScope, HELP_PANEL_W, PLAYLISTS_PANEL_W,
    SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
impl App {
    /// Handle a mouse event when a panel overlay (help/settings/sessions/playlists) is open.
    /// Returns true if the event was consumed.
    pub(super) fn handle_mouse_panels(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        use crossterm::event::{MouseButton, MouseEventKind};
        let col = mouse.column;
        let row = mouse.row;
        let panel_w: u16 = if self.show_help {
            HELP_PANEL_W
        } else if self.show_settings {
            SETTINGS_PANEL_W
        } else if self.show_sessions {
            SESSIONS_PANEL_W
        } else if self.show_playlists {
            PLAYLISTS_PANEL_W
        } else {
            return false;
        };
        let power_panel = self.layout.main.panel_area.width > 0;
        let panel_area = if power_panel {
            self.layout.main.panel_area
        } else {
            Rect {
                x: 0,
                y: 0,
                width: panel_w.min(self.terminal_width),
                height: self.terminal_height,
            }
        };
        let content_area = self.layout.main.panel_content_area;
        let inside_panel = panel_area.contains((col, row).into());
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) && !inside_panel {
            if self.show_settings {
                self.close_settings();
            } else {
                self.show_help = false;
                self.show_sessions = false;
                self.show_playlists = false;
            }
            return true;
        }
        if self.show_help {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.help_scroll += 3;
                }
                MouseEventKind::ScrollUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(3);
                }
                _ => {}
            }
            return true;
        }
        if self.show_settings && self.multiselect_popup.is_none() {
            let settings_content_area = self.layout.settings_content_area;
            let content_top = settings_content_area.y;
            let content_bottom = settings_content_area
                .y
                .saturating_add(settings_content_area.height);
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.settings_scroll += 3;
                }
                MouseEventKind::ScrollUp => {
                    self.settings_scroll = self.settings_scroll.saturating_sub(3);
                }
                MouseEventKind::Down(MouseButton::Left)
                    if row >= content_top && row < content_bottom =>
                {
                    let lines_idx = (row - content_top) as usize + self.settings_scroll;
                    if let Some(cur) = self
                        .layout
                        .settings_line_of_cursor
                        .iter()
                        .position(|&l| l == lines_idx)
                    {
                        self.settings_cursor = cur;
                        self.settings_scroll_follow();
                        self.handle_settings_activate();
                    }
                }
                _ => {}
            }
            return true;
        }
        if self.show_sessions {
            const ENTRY_H: u16 = 4;
            let content_top = if power_panel { content_area.y } else { 1 };
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    if !self.sessions.is_empty() {
                        self.sessions_cursor =
                            (self.sessions_cursor + 1).min(self.sessions.len() - 1);
                    }
                }
                MouseEventKind::ScrollUp => {
                    self.sessions_cursor = self.sessions_cursor.saturating_sub(1);
                }
                MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                    let idx = ((row - content_top) / ENTRY_H) as usize;
                    if idx < self.sessions.len() {
                        if self.sessions_cursor == idx {
                            if let Some(sess) = self.sessions.get(idx) {
                                let sess = sess.clone();
                                self.connect_to_session(&sess);
                            }
                        } else {
                            self.sessions_cursor = idx;
                        }
                    }
                }
                _ => {}
            }
            return true;
        }
        if self.show_playlists {
            let content_top = if power_panel { content_area.y } else { 1 };
            if self.playlists_open.is_some() {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if !self.playlists_open_items.is_empty() {
                            self.playlists_open_cursor = (self.playlists_open_cursor + 1)
                                .min(self.playlists_open_items.len() - 1);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        self.playlists_open_cursor = self.playlists_open_cursor.saturating_sub(1);
                    }
                    MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                        let click_line = (row - content_top) as usize;
                        let mut y = 0usize;
                        let mut idx = self.playlists_open_scroll;
                        for i in self.playlists_open_items[self.playlists_open_scroll..].iter() {
                            let text_width = if power_panel {
                                content_area.width as usize
                            } else {
                                PLAYLISTS_PANEL_W.min(self.terminal_width) as usize
                            };
                            let h = if i.display_name().len() <= text_width.saturating_sub(6) {
                                1
                            } else {
                                2
                            };
                            if click_line < y + h {
                                break;
                            }
                            y += h;
                            idx += 1;
                        }
                        if idx < self.playlists_open_items.len() {
                            if self.playlists_open_cursor == idx {
                                let selected_id =
                                    self.playlists_open_items.get(idx).map(|i| i.id.clone());
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
                            } else {
                                self.playlists_open_cursor = idx;
                            }
                        }
                    }
                    MouseEventKind::Down(MouseButton::Right) if row >= content_top => {
                        self.playlists_open = None;
                        self.playlists_open_items = Vec::new();
                    }
                    _ => {}
                }
            } else {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if !self.playlists.is_empty() {
                            self.playlists_cursor =
                                (self.playlists_cursor + 1).min(self.playlists.len() - 1);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        self.playlists_cursor = self.playlists_cursor.saturating_sub(1);
                    }
                    MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                        let idx = (row - content_top) as usize + self.playlists_scroll;
                        if idx < self.playlists.len() {
                            if self.playlists_cursor == idx {
                                let id = self.playlists[idx].id.clone();
                                self.load_and_play_playlist(id);
                            } else {
                                self.playlists_cursor = idx;
                            }
                        }
                    }
                    _ => {}
                }
            }
            return true;
        }
        false
    }
}
