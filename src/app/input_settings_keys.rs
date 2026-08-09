use super::settings::settings_total_rows;
use super::App;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(super) fn handle_key_settings(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.show_settings {
            return None;
        }
        if self.multiselect_popup.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.close_multiselect_popup();
                }
                KeyCode::Up => {
                    if let Some(p) = &mut self.multiselect_popup {
                        if p.cursor > 0 {
                            p.cursor -= 1;
                        }
                    }
                }
                KeyCode::Down => {
                    if let Some(p) = &mut self.multiselect_popup {
                        if p.cursor + 1 < p.items.len() {
                            p.cursor += 1;
                        }
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(p) = &mut self.multiselect_popup {
                        let i = p.cursor;
                        p.items[i].2 = !p.items[i].2;
                    }
                }
                _ => {}
            }
            return Some(false);
        }
        if self.feeds_manage_popup.is_some() {
            return self.handle_key_feeds_manage(key);
        }
        if self.library_routes_popup.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.handle_library_routes_esc();
                }
                KeyCode::Enter => {
                    self.handle_library_routes_enter();
                }
                KeyCode::Up => {
                    self.move_library_routes_cursor(-1);
                }
                KeyCode::Down => {
                    self.move_library_routes_cursor(1);
                }
                _ => {}
            }
            return Some(false);
        }
        if self.confirm_logout {
            if matches!(key.code, KeyCode::Char('y')) {
                mbv_core::api::clear_cached_token();
                self.confirm_logout = false;
                self.show_settings = false;
                return Some(true);
            } else {
                self.confirm_logout = false;
            }
            return Some(false);
        }
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                return Some(self.try_quit());
            }
            KeyCode::Esc => {
                self.close_settings();
            }
            KeyCode::F(1) => {
                self.close_settings();
                self.show_help = true;
            }
            KeyCode::F(3) => {
                self.close_settings();
                self.show_sessions = true;
            }
            KeyCode::F(4) => {
                self.close_settings();
                self.open_playlists_panel();
            }
            KeyCode::Up => {
                if self.settings_cursor > 0 {
                    self.settings_cursor -= 1;
                    self.settings_scroll_follow();
                }
            }
            KeyCode::Down => {
                if self.settings_cursor + 1 < settings_total_rows() {
                    self.settings_cursor += 1;
                    self.settings_scroll_follow();
                }
            }
            KeyCode::PageUp => {
                self.settings_scroll = self.settings_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.settings_scroll += 10;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') | KeyCode::Enter => {
                self.handle_settings_activate();
            }
            _ => {}
        }
        Some(false)
    }

    pub(super) fn handle_key_help(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.show_help {
            return None;
        }
        match super::input_resolver::help_resolve(super::input_resolver::KeyChord::from_key(key)) {
            super::input_resolver::KeyResolution::Command(cmd) => Some(self.dispatch(cmd)),
            // Help swallows unknown keys; FallThrough is unreachable for this
            // context but treated identically (still consumed) to preserve today's
            // "help eats every key" behavior.
            super::input_resolver::KeyResolution::Swallow
            | super::input_resolver::KeyResolution::FallThrough => Some(false),
        }
    }

    pub(super) fn handle_key_sessions(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.show_sessions {
            return None;
        }
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                return Some(self.try_quit());
            }
            KeyCode::Esc | KeyCode::F(3) => {
                self.show_sessions = false;
            }
            KeyCode::F(1) => {
                self.show_sessions = false;
                self.show_help = true;
            }
            KeyCode::F(2) => {
                self.show_sessions = false;
                self.show_settings = true;
            }
            KeyCode::F(4) => {
                self.show_sessions = false;
                self.open_playlists_panel();
            }
            KeyCode::Up => {
                self.sessions_cursor = self.sessions_cursor.saturating_sub(1);
            }
            KeyCode::Down => {
                if !self.sessions.is_empty() {
                    self.sessions_cursor = (self.sessions_cursor + 1).min(self.sessions.len() - 1);
                }
            }
            KeyCode::Char('r') => {
                self.spawn_sessions_load();
            }
            KeyCode::Enter => {
                if let Some(sess) = self.sessions.get(self.sessions_cursor) {
                    let sess = sess.clone();
                    self.connect_to_session(&sess);
                }
            }
            KeyCode::Char('d') => {
                self.disconnect_remote();
                self.show_sessions = false;
            }
            _ => {}
        }
        Some(false)
    }
}
