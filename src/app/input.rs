use std::sync::Arc;
use std::time::{Duration, Instant};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Borders};
use textwrap::wrap;
use crate::api::{MediaItem, TICKS_PER_SECOND};
use crate::player::PlayerCommand;
use super::{
    App, HOME_MIN_SECTION_H,
    LogPane, PendingQueueAction, ContextAction, ContextMenu,
    LibSearch, SavePlaylistDialog, SavePlaylistStage,
    SESSIONS_PANEL_W, PLAYLISTS_PANEL_W, HELP_PANEL_W, SETTINGS_PANEL_W,
};
use super::settings::settings_total_rows;
use super::ui_util::item_text_and_style;

impl App {
    pub(super) fn tab_count(&self) -> usize { 2 + self.libs.len() + if self.show_log_tab { 1 } else { 0 } }
    pub(super) fn log_tab_idx(&self) -> usize { 2 + self.libs.len() }
    pub(super) fn lib_tab_offset(&self) -> usize { 2 }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.show_save_playlist_modal {
            let quit_after = matches!(self.pending_queue_action, Some(PendingQueueAction::Quit));
            let play_after = matches!(self.pending_queue_action, Some(PendingQueueAction::PlayItems { .. }));
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.save_playlist_to_emby();
                    self.show_save_playlist_modal = false;
                    if let Some(action) = self.pending_queue_action.take() {
                        self.execute_pending_queue_action(action);
                    }
                    if play_after { self.show_playlists = false; self.set_tab(1); }
                    if quit_after { return true; }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.show_save_playlist_modal = false;
                    if let Some(action) = self.pending_queue_action.take() {
                        self.execute_pending_queue_action(action);
                    }
                    if play_after { self.show_playlists = false; self.set_tab(1); }
                    if quit_after { return true; }
                }
                KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.show_save_playlist_modal = false;
                    self.pending_queue_action = None;
                }
                _ => {}
            }
            return false;
        }
        if self.save_playlist_dialog.is_some() {
            return self.handle_save_playlist_key(key);
        }
        if self.show_settings {
            if self.multiselect_popup.is_some() {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => { self.close_multiselect_popup(); }
                    KeyCode::Up => {
                        if let Some(p) = &mut self.multiselect_popup {
                            if p.cursor > 0 { p.cursor -= 1; }
                        }
                    }
                    KeyCode::Down => {
                        if let Some(p) = &mut self.multiselect_popup {
                            if p.cursor + 1 < p.items.len() { p.cursor += 1; }
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
                return false;
            }
            if self.confirm_logout {
                if matches!(key.code, KeyCode::Char('y')) {
                    crate::api::clear_cached_token();
                    self.confirm_logout = false;
                    self.show_settings = false;
                    self.flash_status("Logged out — restart mbv to sign in again".into());
                } else {
                    self.confirm_logout = false;
                }
                return false;
            }
            match key.code {
                KeyCode::Char('q') => { return self.try_quit(); }
                KeyCode::Esc => { self.close_settings(); }
                KeyCode::F(1) => { self.close_settings(); self.show_help = true; }
                KeyCode::F(3) => { self.close_settings(); self.show_sessions = true; }
                KeyCode::F(4) => { self.close_settings(); self.open_playlists_panel(); }
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
            return false;
        }
        if self.show_help {
            match key.code {
                KeyCode::Char('q') => { return self.try_quit(); }
                KeyCode::Esc | KeyCode::F(1) => { self.show_help = false; }
                KeyCode::F(2) => { self.show_help = false; self.show_settings = true; }
                KeyCode::F(3) => { self.show_help = false; self.show_sessions = true; }
                KeyCode::F(4) => { self.show_help = false; self.open_playlists_panel(); }
                KeyCode::Up       => { self.help_scroll = self.help_scroll.saturating_sub(1); }
                KeyCode::Down     => { self.help_scroll += 1; }
                KeyCode::PageUp   => { self.help_scroll = self.help_scroll.saturating_sub(10); }
                KeyCode::PageDown => { self.help_scroll += 10; }
                KeyCode::Home     => { self.help_scroll = 0; }
                _ => {}
            }
            return false;
        }
        if self.show_sessions {
            match key.code {
                KeyCode::Char('q') => { return self.try_quit(); }
                KeyCode::Esc | KeyCode::F(3) => { self.show_sessions = false; }
                KeyCode::F(1) => { self.show_sessions = false; self.show_help = true; }
                KeyCode::F(2) => { self.show_sessions = false; self.show_settings = true; }
                KeyCode::F(4) => { self.show_sessions = false; self.open_playlists_panel(); }
                KeyCode::Up => {
                    self.sessions_cursor = self.sessions_cursor.saturating_sub(1);
                }
                KeyCode::Down => {
                    if !self.sessions.is_empty() {
                        self.sessions_cursor = (self.sessions_cursor + 1).min(self.sessions.len() - 1);
                    }
                }
                KeyCode::Char('r') => { self.spawn_sessions_load(); }
                KeyCode::Enter => {
                    if let Some(sess) = self.sessions.get(self.sessions_cursor) {
                        let id = sess.id.clone();
                        let name = sess.device_name.clone();
                        self.connected_session_id = Some(id);
                        self.connected_session_state = Some(sess.clone());
                        self.session_miss_count = 0;
                        self.remote_pos_s = sess.position_s;
                        self.remote_pos_at = Instant::now();
                        self.remote_api_pos_advanced_at = Instant::now();
                        self.show_sessions = false;
                        self.flash_status(format!("Connected to {name}"));
                        self.spawn_sessions_load();
                    }
                }
                KeyCode::Char('d') => {
                    self.connected_session_id = None;
                    self.connected_session_state = None;
                    self.session_miss_count = 0;
                    self.remote_pos_s = 0;
                    self.show_sessions = false;
                    self.flash_status("Disconnected from remote session".to_string());
                }
                _ => {}
            }
            return false;
        }
        if self.show_playlists {
            match key.code {
                KeyCode::Char('q') => { return self.try_quit(); }
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
                KeyCode::F(1) => { self.show_playlists = false; self.show_help = true; }
                KeyCode::F(2) => { self.show_playlists = false; self.show_settings = true; }
                KeyCode::F(3) => { self.show_playlists = false; self.show_sessions = true; }
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
                            self.playlists_open_cursor = (self.playlists_open_cursor + 1).min(self.playlists_open_items.len() - 1);
                        }
                    } else if !self.playlists.is_empty() {
                        self.playlists_cursor = (self.playlists_cursor + 1).min(self.playlists.len() - 1);
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
                            self.playlists_open_cursor = (self.playlists_open_cursor + page).min(self.playlists_open_items.len() - 1);
                        }
                    } else if !self.playlists.is_empty() {
                        self.playlists_cursor = (self.playlists_cursor + page).min(self.playlists.len() - 1);
                    }
                }
                KeyCode::Home => {
                    if self.playlists_open.is_some() { self.playlists_open_cursor = 0; }
                    else { self.playlists_cursor = 0; }
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
                        let selected_id = self.playlists_open_items.get(self.playlists_open_cursor).map(|i| i.id.clone());
                        let pl_source = crate::config::QueueSource::Playlist {
                            id: self.playlists_open.as_ref().map(|p| p.id.clone()),
                            name: self.playlists_open.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                        };
                        let items: Vec<MediaItem> = self.playlists_open_items.iter().filter(|i| !i.is_folder).cloned().collect();
                        if !items.is_empty() {
                            let start = selected_id.as_deref()
                                .and_then(|id| items.iter().position(|i| i.id == id))
                                .unwrap_or(0);
                            let action = PendingQueueAction::PlayItems {
                                items, start_idx: start, source: pl_source,
                            };
                            self.replace_queue_or_prompt(action);
                            if !self.show_save_playlist_modal {
                                self.show_playlists = false;
                                self.set_tab(1);
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
            return false;
        }
        // When library search is active, unmodified keys feed the search
        if self.tab_idx > 1
            && self.tab_idx != self.log_tab_idx()
            && !key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self.libs.get(self.tab_idx - self.lib_tab_offset()).is_some_and(|l| l.search.is_some())
        {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let alt = key.modifiers.contains(KeyModifiers::ALT);
            match key.code {
                KeyCode::Esc => { self.libs[lib_idx].search = None; }
                KeyCode::Backspace => {
                    let empty = self.libs[lib_idx].search.as_ref().is_none_or(|s| s.query.is_empty());
                    if empty { self.libs[lib_idx].search = None; }
                    else {
                        self.libs[lib_idx].search.as_mut().unwrap().query.pop();
                        self.update_lib_search(lib_idx);
                    }
                }
                KeyCode::Up       => self.move_lib_cursor(-1),
                KeyCode::Down     => self.move_lib_cursor(1),
                KeyCode::PageUp   => { let p = self.lib_page_size(); self.move_lib_cursor(-(p as i64)); }
                KeyCode::PageDown => { let p = self.lib_page_size(); self.move_lib_cursor(p as i64); }
                KeyCode::Home     => self.jump_lib_cursor(false),
                KeyCode::End      => self.jump_lib_cursor(true),
                KeyCode::Enter => self.select(),
                KeyCode::Char(c) if !alt => {
                    self.libs[lib_idx].search.as_mut().unwrap().query.push(c);
                    self.update_lib_search(lib_idx);
                }
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::F(1) { self.show_help = true; return false; }
        if key.code == KeyCode::F(2) { self.show_settings = !self.show_settings; return false; }
        if key.code == KeyCode::F(3) { self.show_sessions = true; self.spawn_sessions_load(); return false; }
        if key.code == KeyCode::F(4) { self.open_playlists_panel(); return false; }
        if key.code == KeyCode::Char('h') {
            let active = self.player.status.lock().unwrap().active;
            let show_controls = active || self.connected_session_id.is_some();
            let in_presentation = self.tab_idx == 1 && self.playlist_view == 2;
            if show_controls && !in_presentation {
                self.show_playback_panel = !self.show_playback_panel;
            }
            return false;
        }
        let in_lib_search = self.tab_idx > 1
            && self.tab_idx != self.log_tab_idx()
            && self.libs.get(self.tab_idx - self.lib_tab_offset()).is_some_and(|l| l.search.is_some());
        if self.confirm_clear_playlist {
            self.confirm_clear_playlist = false;
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter) {
                self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
                if !self.show_save_playlist_modal {
                    self.flash_status("Playlist cleared".into());
                }
            } else {
                self.status.clear();
            }
            return false;
        }
        if self.skip_intro_end_ticks.is_some() {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter) {
                if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                    let secs = end_ticks as f64 / crate::api::TICKS_PER_SECOND as f64;
                    self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                    self.player.send_command(PlayerCommand::SkipIntroDismiss);
                    self.status.clear();
                }
            } else {
                self.skip_intro_end_ticks = None;
                self.player.send_command(PlayerCommand::SkipIntroDismiss);
                self.status.clear();
            }
            return false;
        }
        if self.next_up_item.is_some() {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter) {
                if let Some(item) = self.next_up_item.take() {
                    if let Some(idx) = self.player_tab.items.iter().position(|i| i.id == item.id) {
                        let label = item.playback_label();
                        self.player.send_command(PlayerCommand::JumpTo(idx));
                        self.player_tab.playlist_cursor = idx;
                        self.flash_status(label);
                    }
                }
            } else {
                self.next_up_item = None;
                self.player.send_command(PlayerCommand::NextUpDismiss);
                self.status.clear();
            }
            return false;
        }
        if self.tab_idx != self.log_tab_idx() {
            if key.code == KeyCode::Char('c') && !key.modifiers.contains(KeyModifiers::ALT) && !in_lib_search {
                if self.player_tab.items.is_empty() { return false; }
                self.notify_with_actions("mbv", "Clear queue?", &[("clear:yes", "Clear"), ("clear:no", "Cancel")]);
                self.status = "Clear queue? (Y/n)".into();
                self.confirm_clear_playlist = true;
                return false;
            }
        }
        if self.tab_idx != self.log_tab_idx() {
            if let Some(quit) = self.handle_playback_key(key) { return quit; }
        }
        if self.context_menu.is_some() {
            match key.code {
                KeyCode::Esc => { self.context_menu = None; self.force_clear = true; }
                KeyCode::Up   => {
                    if let Some(m) = &mut self.context_menu {
                        if m.cursor > 0 { m.cursor -= 1; }
                    }
                }
                KeyCode::Down => {
                    if let Some(m) = &mut self.context_menu {
                        if m.cursor + 1 < m.items.len() { m.cursor += 1; }
                    }
                }
                KeyCode::Enter => {
                    if let Some(m) = self.context_menu.take() {
                        self.force_clear = true;
                        let action = m.actions.get(m.cursor).cloned();
                        self.execute_context_action(action);
                    }
                }
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.force_clear = true;
            return false;
        }
        if key.code == KeyCode::F(5) { self.refresh_current_view(); return false; }
        if self.tab_idx == 0 { return self.handle_combined_key(key); }
        if self.tab_idx == 1 { return self.handle_playlist_key(key); }
        if self.tab_idx == self.log_tab_idx() { return self.handle_log_key(key); }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => self.enqueue_selected(),
            KeyCode::Char('q') => { return self.try_quit(); }
            KeyCode::Tab => { let n = (self.tab_idx + 1) % self.tab_count(); self.set_tab(n); }
            KeyCode::BackTab => { let n = self.tab_count(); self.set_tab((self.tab_idx + n - 1) % n); }
            KeyCode::Esc | KeyCode::Backspace => self.go_back(),
            KeyCode::Up    => self.move_lib_cursor(if self.is_viewing_season_grid(lib_idx) { -4 } else { -1 }),
            KeyCode::Down  => self.move_lib_cursor(if self.is_viewing_season_grid(lib_idx) {  4 } else {  1 }),
            KeyCode::Left  if self.is_viewing_season_grid(lib_idx) => self.move_lib_cursor(-1),
            KeyCode::Right if self.is_viewing_season_grid(lib_idx) => self.move_lib_cursor(1),
            KeyCode::PageUp   => { let p = self.lib_page_size(); self.move_lib_cursor(-(p as i64)); }
            KeyCode::PageDown => { let p = self.lib_page_size(); self.move_lib_cursor(p as i64); }
            KeyCode::Home     => self.jump_lib_cursor(false),
            KeyCode::End      => self.jump_lib_cursor(true),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let item = self.current_lib_item();
                if let Some(item) = item {
                    if item.is_folder {
                        let ct = self.libs[self.tab_idx - self.lib_tab_offset()].library.collection_type.clone();
                        self.play_folder(&item.id.clone());
                        self.queue_source = crate::config::QueueSource::Collection { collection_type: ct };
                        self.save_queue_state();
                    }
                    else { self.select(); }
                }
            }
            KeyCode::Enter => self.select(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self.toggle_watched(),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self.shuffle_play(),
            KeyCode::Char('o') if alt => self.open_context_menu(),
            KeyCode::Char('o') if !alt => self.open_context_menu(),
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
            }
            KeyCode::Char('/') => {
                let (items, needs_full_load) = self.libs[lib_idx].nav_stack.last()
                    .map(|l| {
                        let all = l.all_items.clone().unwrap_or_else(|| l.items.clone());
                        let needs = l.all_items.is_none() && l.items.len() < l.total_count;
                        (all, needs)
                    })
                    .unwrap_or_default();
                let n = items.len();
                self.libs[lib_idx].search = Some(LibSearch {
                    query: String::new(),
                    items,
                    results: (0..n).collect(),
                    cursor: 0,
                    loading: needs_full_load,
                });
                if needs_full_load {
                    self.spawn_search_items_load(lib_idx);
                }
                self.update_lib_search(lib_idx);
            }
            _ => {}
        }
        false
    }

    fn handle_combined_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => { return self.try_quit(); }
            KeyCode::Tab => {
                let n = (self.tab_idx + 1) % self.tab_count(); self.set_tab(n); return false;
            }
            KeyCode::BackTab => {
                let n = self.tab_count(); self.set_tab((self.tab_idx + n - 1) % n); return false;
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                let n = 1 + self.home.latest.len();
                self.home.section = (self.home.section + n - 1) % n;
                self.ensure_home_section_visible();
                if self.home_card_view && !self.card_image_states.is_empty() { self.force_clear = true; }
                return false;
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                let n = 1 + self.home.latest.len();
                self.home.section = (self.home.section + 1) % n;
                self.ensure_home_section_visible();
                if self.home_card_view && !self.card_image_states.is_empty() { self.force_clear = true; }
                return false;
            }
            KeyCode::Char('v') => {
                if self.images_enabled() {
                    self.home_card_view = !self.home_card_view;
                    self.save_home_card_view();
                    if !self.card_image_states.is_empty() { self.force_clear = true; }
                }
                return false;
            }
            KeyCode::Char('o') => {
                self.open_context_menu(); return false;
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
                return false;
            }
            _ => {}
        }
        match key.code {
            KeyCode::Up => {
                if self.home_card_view {
                    self.home.section = self.home.section.saturating_sub(1);
                    self.ensure_home_section_visible();
                    if !self.card_image_states.is_empty() { self.force_clear = true; }
                } else {
                    self.move_home_cursor(-1);
                }
            }
            KeyCode::Down => {
                if self.home_card_view {
                    let n = 1 + self.home.latest.len();
                    self.home.section = (self.home.section + 1).min(n.saturating_sub(1));
                    self.ensure_home_section_visible();
                    if !self.card_image_states.is_empty() { self.force_clear = true; }
                } else {
                    self.move_home_cursor(1);
                }
            }
            KeyCode::Left  => { if self.home_card_view { self.move_home_cursor(-1); } }
            KeyCode::Right => { if self.home_card_view { self.move_home_cursor(1); } }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => self.enqueue_selected(),
            KeyCode::Enter => self.select_home(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self.toggle_watched_home(),
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => self.enqueue_selected(),
            _ => {}
        }
        false
    }

    pub(super) fn adjust_volume(&mut self, delta: i64) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let vol = self.connected_session_state.as_ref().map(|s| s.volume).unwrap_or(50);
            let new_vol = (vol + delta).clamp(0, 100);
            let id = conn_id.clone();
            self.do_session_command(move |c| c.session_set_volume(&id, new_vol));
            return;
        }
        let active = self.player.status.lock().unwrap().active;
        if active {
            let st = self.player.status.lock().unwrap();
            let v = (st.volume as i64 + delta).clamp(0, st.volume_max as i64) as u8;
            drop(st);
            self.player.send_command(PlayerCommand::SetVolume(v as i64));
            self.ui_volume = v;
        } else {
            self.ui_volume = (self.ui_volume as i64 + delta).clamp(0, 200) as u8;
        }
        self.save_prefs();
    }

    fn handle_playback_key(&mut self, key: KeyEvent) -> Option<bool> {
        let active = self.player.status.lock().unwrap().active;
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let pos_s = self.connected_session_state.as_ref().map(|s| s.position_s).unwrap_or(0);
            let id = conn_id.clone();
            match key.code {
                KeyCode::Char(' ') => {
                    self.do_session_command(move |c| c.session_transport(&id, "PlayPause"));
                    return Some(false);
                }
                KeyCode::Enter if alt => {
                    self.do_session_command(move |c| c.session_transport(&id, "Stop"));
                    return Some(false);
                }
                KeyCode::Left if key.modifiers == KeyModifiers::ALT => {
                    let ticks = (pos_s - 5).max(0) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Right if key.modifiers == KeyModifiers::ALT => {
                    let ticks = (pos_s + 5) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Char('<') => {
                    let ticks = (pos_s - 5).max(0) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Char('>') => {
                    let ticks = (pos_s + 5) * crate::api::TICKS_PER_SECOND;
                    self.do_session_command(move |c| c.session_seek(&id, ticks));
                    return Some(false);
                }
                KeyCode::Char('z') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.cycle_sub();
                    return Some(false);
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('-') => { self.adjust_volume(-5); return Some(false); }
            KeyCode::Char('+') | KeyCode::Char('=') => { self.adjust_volume(5); return Some(false); }
            KeyCode::Char('z') if !key.modifiers.contains(KeyModifiers::CONTROL) => { self.toggle_sub(); return Some(false); }
            KeyCode::Char('m') => {
                self.mute_on = !self.mute_on;
                self.player.send_command(PlayerCommand::SetMute(self.mute_on));
                return Some(false);
            }
            _ => {}
        }
        if !active { return None; }
        match key.code {
            KeyCode::Enter if alt => { self.player.stop(); Some(false) }
            KeyCode::Char(' ') => { self.player.send_command(PlayerCommand::TogglePause); Some(false) }
            KeyCode::Left  if key.modifiers == KeyModifiers::ALT => { self.player.send_command(PlayerCommand::Seek(-5.0)); Some(false) }
            KeyCode::Right if key.modifiers == KeyModifiers::ALT => { self.player.send_command(PlayerCommand::Seek(5.0));  Some(false) }
            KeyCode::Char('<') => { self.player.send_command(PlayerCommand::Seek(-5.0)); Some(false) }
            KeyCode::Char('>') => { self.player.send_command(PlayerCommand::Seek(5.0));  Some(false) }
            KeyCode::Char('a') => { if self.is_audio_item() { self.toggle_mute(); } else { self.cycle_audio(); } Some(false) }
            _ => None,
        }
    }

    fn handle_playlist_key(&mut self, key: KeyEvent) -> bool {
        if let Some(t) = self.confirm_remove_idx {
            self.confirm_remove_idx = None;
            self.status.clear();
            if matches!(key.code, KeyCode::Char('y')) {
                self.player.stop();
                let item = self.player_tab.items.remove(t);
                self.playlist_undo_stack.push((t, item));
                self.queue_dirty = true;
                self.player_tab.playlist_cursor =
                    if self.player_tab.items.is_empty() { 0 }
                    else { t.min(self.player_tab.items.len() - 1) };
                self.save_queue_state();
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') => { return self.try_quit(); }
            KeyCode::Tab => { let n = (self.tab_idx + 1) % self.tab_count(); self.set_tab(n); }
            KeyCode::BackTab => { let n = self.tab_count(); self.set_tab((self.tab_idx + n - 1) % n); }
            KeyCode::Up | KeyCode::Left
                if self.player_tab.playlist_cursor > 0 && (key.code == KeyCode::Up || self.playlist_view == 1) => {
                    self.last_nav_at = Instant::now();
                    self.player_tab.playlist_cursor -= 1;
                }
            KeyCode::Down | KeyCode::Right
                if self.player_tab.playlist_cursor + 1 < self.player_tab.items.len()
                && (key.code == KeyCode::Down || self.playlist_view == 1) => {
                    self.last_nav_at = Instant::now();
                    self.player_tab.playlist_cursor += 1;
                }
            KeyCode::PageUp => {
                let p = self.playlist_page_size();
                self.player_tab.playlist_cursor = self.player_tab.playlist_cursor.saturating_sub(p);
            }
            KeyCode::PageDown => {
                let p = self.playlist_page_size();
                let n = self.player_tab.items.len();
                self.player_tab.playlist_cursor = (self.player_tab.playlist_cursor + p).min(n.saturating_sub(1));
            }
            KeyCode::Home => {
                self.player_tab.playlist_cursor = 0;
            }
            KeyCode::End => {
                let n = self.player_tab.items.len();
                if n > 0 { self.player_tab.playlist_cursor = n - 1; }
            }
            KeyCode::Enter => {
                let t = self.player_tab.playlist_cursor;
                let n = self.player_tab.items.len();
                if t < n {
                    if let Some(ref conn_id) = self.connected_session_id.clone() {
                        let item = self.player_tab.items[t].clone();
                        let id = conn_id.clone();
                        let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                        let start_ticks = item.playback_position_ticks;
                        let label = item.playback_label();
                        self.flash_status(format!("Playing on remote: {label}"));
                        self.do_session_command(move |c| c.session_play_items(&id, &item_ids, t, start_ticks));
                    } else {
                        let st = self.player.status.lock().unwrap();
                        let active = st.active;
                        let current_idx = st.current_idx;
                        drop(st);
                        if active {
                            let is_audio = self.player_tab.items.get(t).map(|i| i.is_audio()).unwrap_or(false);
                            if t == current_idx && is_audio {
                                self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
                            } else if t != current_idx {
                                self.player.send_command(PlayerCommand::JumpTo(t));
                            }
                        } else if !self.player_tab.items.is_empty() {
                            let items = self.player_tab.items.clone();
                            let c = Arc::new(self.client.lock().unwrap().clone());
                            self.player.play_playlist(items, t, c, self.ui_volume);
                        }
                    }
                }
            }
            KeyCode::Delete => {
                let t = self.player_tab.playlist_cursor;
                if t < self.player_tab.items.len() { self.remove_from_playlist(t); }
            }
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some((idx, item)) = self.playlist_undo_stack.pop() {
                    let name = item.display_name();
                    let idx = idx.min(self.player_tab.items.len());
                    self.player_tab.items.insert(idx, item);
                    self.player_tab.playlist_cursor = idx;
                    self.queue_dirty = true;
                    self.flash_status(format!("Restored: {name}"));
                    self.save_queue_state();
                }
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
            }
            KeyCode::Char('i') => {
                let cursor = self.player_tab.playlist_cursor;
                if let Some(item) = self.player_tab.items.get(cursor) {
                    let item_id = item.id.clone();
                    let lib_ids: Vec<(usize, String)> = self.libs.iter().enumerate()
                        .map(|(i, lib)| (i, lib.library.id.clone()))
                        .collect();
                    self.flash_status("Navigating to library\u{2026}".into());
                    self.spawn_navigate_to_item(item_id, lib_ids);
                }
            }
            KeyCode::Char('o') => {
                self.open_context_menu();
            }
            KeyCode::Char('v') => {
                self.playlist_view = (self.playlist_view + 1) % 3;
                self.save_playlist_view();
                if !self.card_image_states.is_empty() { self.force_clear = true; }
            }
            KeyCode::Char('.') => {
                let s = self.player.status.lock().unwrap();
                if s.active {
                    self.player_tab.playlist_cursor = s.current_idx;
                } else {
                    drop(s);
                    self.flash_status("Nothing is playing".into());
                }
            }
            KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                if !self.player_tab.items.is_empty() {
                    self.save_playlist_dialog = Some(SavePlaylistDialog {
                        input: self.queue_playlist_name().to_string(),
                        stage: SavePlaylistStage::EnterName,
                    });
                }
            }
            _ => {}
        }
        false
    }

    fn handle_save_playlist_key(&mut self, key: KeyEvent) -> bool {
        let Some(ref dialog) = self.save_playlist_dialog else { return false; };
        match &dialog.stage {
            SavePlaylistStage::EnterName => {
                match key.code {
                    KeyCode::Esc => { self.save_playlist_dialog = None; self.force_clear = true; }
                    KeyCode::Backspace => {
                        if let Some(d) = &mut self.save_playlist_dialog { d.input.pop(); }
                    }
                    KeyCode::Char(c) if key.modifiers == crossterm::event::KeyModifiers::NONE
                                     || key.modifiers == crossterm::event::KeyModifiers::SHIFT => {
                        if let Some(d) = &mut self.save_playlist_dialog { d.input.push(c); }
                    }
                    KeyCode::Enter => {
                        let name = dialog.input.trim().to_string();
                        if name.is_empty() { return false; }
                        let playlists = {
                            let c = self.client.lock().unwrap();
                            c.get_playlists().unwrap_or_default()
                        };
                        let existing = playlists.into_iter()
                            .find(|p| p.name.to_lowercase() == name.to_lowercase());
                        if let Some(existing) = existing {
                            self.save_playlist_dialog = Some(SavePlaylistDialog {
                                input: name,
                                stage: SavePlaylistStage::ConfirmOverwrite { existing_id: existing.id },
                            });
                        } else {
                            let ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                            let result = {
                                let c = self.client.lock().unwrap();
                                c.create_playlist(&name, &ids)
                            };
                            self.save_playlist_dialog = None;
                            self.force_clear = true;
                            match result {
                                Ok(id) => {
                                    self.queue_source = crate::config::QueueSource::Playlist { id: Some(id), name: name.clone() };
                                    self.queue_dirty = false;
                                    self.save_queue_state();
                                    self.flash_status(format!("Saved as playlist \"{name}\""));
                                }
                                Err(e) => self.flash_status(format!("Error: {e}")),
                            }
                        }
                    }
                    _ => {}
                }
            }
            SavePlaylistStage::ConfirmOverwrite { existing_id } => {
                let existing_id = existing_id.clone();
                match key.code {
                    KeyCode::Char('y') => {
                        let name = dialog.input.clone();
                        let ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                        let result = {
                            let c = self.client.lock().unwrap();
                            c.delete_playlist(&existing_id)
                                .and_then(|_| c.create_playlist(&name, &ids))
                        };
                        self.save_playlist_dialog = None;
                        self.force_clear = true;
                        match result {
                            Ok(id) => {
                                self.queue_source = crate::config::QueueSource::Playlist { id: Some(id), name: name.clone() };
                                self.queue_dirty = false;
                                self.flash_status(format!("Saved as playlist \"{name}\""));
                            }
                            Err(e) => self.flash_status(format!("Error: {e}")),
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

    fn handle_log_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => { return self.try_quit(); }
            KeyCode::Tab | KeyCode::BackTab => { self.set_tab(0); }
            KeyCode::Left  => { self.log_pane = LogPane::Sources; }
            KeyCode::Right => { self.log_pane = LogPane::Log; }
            KeyCode::Up => {
                match self.log_pane {
                    LogPane::Log     => { self.log_scroll += 1; }
                    LogPane::Sources => { self.log_source_cursor = self.log_source_cursor.saturating_sub(1); }
                }
            }
            KeyCode::Down => {
                match self.log_pane {
                    LogPane::Log     => { self.log_scroll = self.log_scroll.saturating_sub(1); }
                    LogPane::Sources => { self.log_source_cursor += 1; }
                }
            }
            KeyCode::PageUp   => { self.log_scroll += 20; }
            KeyCode::PageDown => { self.log_scroll = self.log_scroll.saturating_sub(20); }
            KeyCode::Char(' ') => {
                let sources = self.log_sources();
                let src_cursor = self.log_source_cursor.min(sources.len().saturating_sub(1));
                if let Some(src) = sources.get(src_cursor) {
                    if self.log_disabled_sources.contains(src) {
                        self.log_disabled_sources.remove(src);
                    } else {
                        self.log_disabled_sources.insert(src.clone());
                    }
                }
            }
            KeyCode::Char('c') => {
                let entries = self.visible_log_entries();
                let text = entries.iter()
                    .map(|e| format!("{}│{}│{}", e.level.label(), e.source, e.msg))
                    .collect::<Vec<_>>().join("\n");
                let n = entries.len();
                let copied = std::process::Command::new("wl-copy")
                    .arg(&text).status().map(|s| s.success()).unwrap_or(false)
                    || std::process::Command::new("xclip")
                        .args(["-selection", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut c| {
                            use std::io::Write;
                            c.stdin.take().unwrap().write_all(text.as_bytes())?;
                            c.wait()
                        })
                        .map(|s| s.success()).unwrap_or(false);
                if copied { self.flash_status(format!("Copied {n} log lines to clipboard")); }
                else      { self.flash_status("Copy failed — wl-copy/xclip not found".into()); }
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() { self.set_tab(idx); }
            }
            _ => {}
        }
        false
    }

    pub(super) fn log_sources(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut sources: Vec<String> = Vec::new();
        for e in &crate::applog::global().map(|l| l.snapshot()).unwrap_or_default() {
            if seen.insert(e.source.clone()) { sources.push(e.source.clone()); }
        }
        sources.sort_unstable();
        sources
    }

    pub(super) fn visible_log_entries(&self) -> Vec<crate::applog::LogEntry> {
        crate::applog::global().map(|l| l.snapshot()).unwrap_or_default().into_iter()
            .filter(|e| !self.log_disabled_sources.contains(&e.source))
            .collect()
    }

    pub(super) fn visible_tab_range(&self, avail_w: u16) -> (usize, usize) {
        let widths = self.tab_title_widths();
        let n = widths.len();
        let start = self.tab_scroll.min(if n > 0 { n - 1 } else { 0 });
        let left_w: u16 = if start > 0 { 2 } else { 0 };
        let mut budget = avail_w.saturating_sub(left_w);
        let mut end = start;
        while end < n {
            let tab_w: u16 = widths[end] + 2;
            let right_w: u16 = if end + 1 < n { 2 } else { 0 };
            if budget < tab_w + right_w && end > start { break; }
            budget = budget.saturating_sub(tab_w);
            end += 1;
        }
        (start, end)
    }

    pub(super) fn ensure_tab_visible(&mut self) {
        let n = self.tab_count();
        if n == 0 { return; }
        if self.tab_idx < self.tab_scroll {
            self.tab_scroll = self.tab_idx;
            return;
        }
        const RIGHT_W: u16 = 14 + 1 + 2;
        let tab_w = self.terminal_width.saturating_sub(RIGHT_W);
        loop {
            let (_, end) = self.visible_tab_range(tab_w);
            if self.tab_idx < end { break; }
            self.tab_scroll += 1;
        }
    }

    fn tab_title_widths(&self) -> Vec<u16> {
        let pad: u16 = 2;
        let mut w = vec![
            "Home".chars().count() as u16 + pad,
            "Queue".chars().count() as u16 + pad,
        ];
        for l in &self.libs {
            w.push(l.library.name.chars().count() as u16 + pad);
        }
        if self.show_log_tab {
            w.push("Log".chars().count() as u16 + pad);
        }
        w
    }

    fn tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout_tabs_area;
        if col < area.x || col >= area.x + area.width { return None; }
        let rel = col - area.x;
        let (vis_start, vis_end) = self.visible_tab_range(area.width);
        let has_left  = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let left_w:  u16 = if has_left  { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left && rel < left_w  { return Some(usize::MAX - 1); }
        if has_right && rel >= area.width - right_w { return Some(usize::MAX); }
        let rel = rel - left_w;
        let widths = self.tab_title_widths();
        let pad = 1u16;
        let mut x = 0u16;
        for i in vis_start..vis_end {
            let w = widths[i];
            let end = x + pad + w + pad;
            if rel < end { return Some(i); }
            x = end;
        }
        None
    }

    pub(super) fn handle_button_click(&mut self, btn: usize) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let pos_s = self.connected_session_state.as_ref().map(|s| s.position_s).unwrap_or(0);
            let id = conn_id.clone();
            match btn {
                0 => { self.session_jump_track(&id, -1, "PreviousTrack"); }
                1 => { let t = (pos_s - 5).max(0) * crate::api::TICKS_PER_SECOND; self.do_session_command(move |c| c.session_seek(&id, t)); }
                2 => { self.do_session_command(move |c| c.session_transport(&id, "PlayPause")); }
                3 => { self.do_session_command(move |c| c.session_transport(&id, "Stop")); }
                4 => { let t = (pos_s + 5) * crate::api::TICKS_PER_SECOND; self.do_session_command(move |c| c.session_seek(&id, t)); }
                5 => { self.session_jump_track(&id, 1, "NextTrack"); }
                _ => {}
            }
            return;
        }
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        match btn {
            0 if active && current_idx > 0 => { self.player.send_command(PlayerCommand::JumpTo(current_idx - 1)); }
            1 => { self.player.send_command(PlayerCommand::Seek(-5.0)); }
            2 => { self.player.send_command(PlayerCommand::TogglePause); }
            3 => { self.player.stop(); }
            4 => { self.player.send_command(PlayerCommand::Seek(5.0)); }
            5 if active && current_idx + 1 < self.player_tab.items.len() => { self.player.send_command(PlayerCommand::JumpTo(current_idx + 1)); }
            _ => {}
        }
    }

    pub(super) fn open_context_menu(&mut self) {
        let mut items: Vec<&'static str> = vec![];
        let mut actions: Vec<ContextAction> = vec![];

        let current_item = if self.tab_idx == 0 {
            self.current_home_item()
        } else if self.tab_idx == 1 {
            self.player_tab.items.get(self.player_tab.playlist_cursor).cloned()
        } else if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            self.current_lib_item()
        } else {
            None
        };

        if let Some(ref item) = current_item {
            if item.is_folder {
                items.push("Play All");
                actions.push(ContextAction::PlayFolder(item.id.clone()));
                items.push("Shuffle");
                actions.push(ContextAction::ShuffleFolder(item.id.clone()));
                items.push("Add to Queue");
                actions.push(ContextAction::EnqueueFolder(item.clone()));
                items.push("Mark Watched");
                actions.push(ContextAction::MarkPlayed(item.id.clone()));
                items.push("Mark Unwatched");
                actions.push(ContextAction::MarkUnplayed(item.id.clone()));
            } else {
                items.push("Play");
                actions.push(ContextAction::Play);
                if self.tab_idx != 1 {
                    items.push("Add to Queue");
                    actions.push(ContextAction::Enqueue);
                }
                let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
                if !is_audio {
                    if item.played {
                        items.push("Mark Unwatched");
                        actions.push(ContextAction::MarkUnplayed(item.id.clone()));
                    } else {
                        items.push("Mark Watched");
                        actions.push(ContextAction::MarkPlayed(item.id.clone()));
                    }
                }
                if self.tab_idx == 0 && self.home.section == 0 {
                    items.push("Remove from Continue Watching");
                    actions.push(ContextAction::RemoveFromContinueWatching);
                }
                if self.tab_idx == 1 {
                    items.push("Remove from Playlist");
                    actions.push(ContextAction::RemoveFromPlaylist(self.player_tab.playlist_cursor));
                    items.push("Go to Library");
                    actions.push(ContextAction::GoToLibrary(item.id.clone()));
                }
            }
        }

        if items.is_empty() { return; }

        let (x, y) = self.context_menu_spawn_point();
        self.context_menu = Some(ContextMenu { x, y, items, actions, cursor: 0 });
    }

    pub(super) fn open_context_menu_at(&mut self, x: u16, y: u16) {
        self.open_context_menu();
        if let Some(ref mut menu) = self.context_menu {
            menu.x = x;
            menu.y = y;
        }
    }

    fn context_menu_spawn_point(&self) -> (u16, u16) {
        if (self.tab_idx == 0 && self.home_card_view) || (self.tab_idx == 1 && self.playlist_view == 1) {
            let center = self.layout_carousel_slots[1].1;
            return (center.x + center.width / 2, center.y + center.height / 2);
        }
        if self.tab_idx == 0 {
            let sec = self.home.section;
            if let Some(area) = self.layout_section_areas.get(sec) {
                let scroll = self.layout_home_scrolls.get(sec).copied().unwrap_or(0);
                let cursor = match sec {
                    0 => self.home.continue_cursor,
                    n => self.home.latest.get(n - 1).map(|(_, _, _, c)| *c).unwrap_or(0),
                };
                let row = cursor.saturating_sub(scroll) as u16;
                return (self.terminal_width / 2, area.y + 1 + row);
            }
        } else if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let lib = &self.libs[lib_idx];
            let cursor = lib.nav_stack.last().map(|lvl| {
                lib.search.as_ref()
                    .and_then(|s| s.results.get(s.cursor).copied())
                    .unwrap_or(lvl.cursor)
            }).unwrap_or(0);
            let scroll = self.layout_lib_scroll;
            let row = cursor.saturating_sub(scroll) as u16;
            let tbl = self.layout_lib_table_area;
            return (self.terminal_width / 2, tbl.y + row * 3);
        }
        (4, 4)
    }

    pub(super) fn load_prefs() -> serde_json::Value {
        let path = crate::config::prefs_path();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_default()
    }

    pub(super) fn load_playlist_view() -> u8 {
        let prefs = Self::load_prefs();
        if let Some(v) = prefs["playlist_view"].as_u64() {
            return v.min(2) as u8;
        }
        prefs["playlist_card_view"].as_bool().unwrap_or(false) as u8
    }

    pub(super) fn load_home_card_view() -> bool {
        Self::load_prefs()["home_card_view"].as_bool().unwrap_or(false)
    }

    pub(super) fn load_ui_volume() -> u8 {
        Self::load_prefs()["ui_volume"].as_u64().unwrap_or(100).min(200) as u8
    }

    pub(super) fn load_subs_off() -> bool {
        Self::load_prefs()["subs_off"].as_bool().unwrap_or(true)
    }

    pub(super) fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        let subs_off = self.player.subs_off.load(std::sync::atomic::Ordering::Relaxed);
        let v = serde_json::json!({
            "playlist_view": self.playlist_view,
            "home_card_view": self.home_card_view,
            "ui_volume": self.ui_volume,
            "subs_off": subs_off,
        });
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }

    pub(super) fn save_playlist_view(&self) { self.save_prefs(); }
    pub(super) fn save_home_card_view(&self) { self.save_prefs(); }

    fn seek_to_col(&mut self, col: u16) {
        let bar = self.layout_seekbar_area;
        if bar.width == 0 { return; }
        let fraction = (col.saturating_sub(bar.x)) as f64 / bar.width as f64;
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let runtime_s = self.connected_session_state.as_ref().map(|s| s.runtime_s).unwrap_or(0);
            if runtime_s == 0 { return; }
            let ticks = (fraction * (runtime_s * crate::api::TICKS_PER_SECOND) as f64) as i64;
            let id = conn_id.clone();
            self.do_session_command(move |c| c.session_seek(&id, ticks));
            return;
        }
        let runtime_ticks = self.player.status.lock().unwrap().runtime_ticks;
        if runtime_ticks == 0 { return; }
        let target_secs = (fraction * runtime_ticks as f64) / TICKS_PER_SECOND as f64;
        self.player.send_command(PlayerCommand::SeekAbsolute(target_secs));
    }

    fn click_set_cursor(&mut self, col: u16, row: u16) -> bool {
        if self.tab_idx == 1 {
            let inner = self.layout_playlist_inner;
            if inner.contains((col, row).into()) {
                let row_idx = (row - inner.y) as usize;
                if row_idx > 0 {
                    let data_row = row_idx - 1;
                    let visible = inner.height.saturating_sub(1) as usize;
                    let n = self.player_tab.items.len();
                    let cur = self.player_tab.playlist_cursor;
                    let scroll_start = cur.saturating_sub(visible.saturating_sub(1))
                        .min(n.saturating_sub(visible));
                    let clicked = scroll_start + data_row;
                    if clicked < n {
                        self.player_tab.playlist_cursor = clicked;
                        return true;
                    }
                }
            }
        } else if self.tab_idx == 0 {
            if self.home_rect.contains((col, row).into()) {
                let n_secs = self.layout_section_areas.len();
                let mut found_sec: Option<(usize, Rect)> = None;
                for sec in 0..n_secs {
                    let sect_area = self.layout_section_areas[sec];
                    if sect_area.contains((col, row).into()) {
                        found_sec = Some((sec, sect_area));
                        break;
                    }
                }
                if let Some((sec, sect_area)) = found_sec {
                    self.home.section = sec;
                    let inner = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).inner(sect_area);
                    if inner.contains((col, row).into()) {
                        let row_idx = (row - inner.y) as usize;
                        let scroll_start = self.layout_home_scrolls.get(sec).copied().unwrap_or(0);
                        let inner_h = inner.height as usize;
                        let inner_w = inner.width.max(1) as usize;
                        let item_texts: Vec<String> = {
                            let items_slice: &[MediaItem] = if sec == 0 {
                                &self.home.continue_items
                            } else {
                                self.home.latest.get(sec - 1).map(|c| c.2.as_slice()).unwrap_or(&[])
                            };
                            items_slice.iter().skip(scroll_start)
                                .map(|item| { let (t, _) = item_text_and_style(item, false); t })
                                .collect()
                        };
                        let mut line_acc = 0usize;
                        let mut found_item = None;
                        for (i, text) in item_texts.iter().enumerate() {
                            let n_lines = wrap(text, inner_w).len().max(1);
                            if row_idx < line_acc + n_lines {
                                found_item = Some(scroll_start + i);
                                break;
                            }
                            line_acc += n_lines;
                            if line_acc >= inner_h { break; }
                        }
                        if let Some(clicked) = found_item {
                            let (len, _) = self.home_section_len_cur(sec);
                            if clicked < len {
                                self.set_home_cursor(sec, clicked);
                                return true;
                            }
                        }
                    }
                }
            }
        } else if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let tbl = self.layout_lib_table_area;
            if tbl.contains((col, row).into()) {
                let click_y = row - tbl.y;
                let display_pos = {
                    let mut y = 0u16;
                    let mut found = self.layout_lib_scroll;
                    for (vi, &h) in self.layout_lib_row_heights.iter().enumerate() {
                        if click_y < y + h { found = self.layout_lib_scroll + vi; break; }
                        y += h;
                    }
                    found
                };
                let lib_off = self.lib_tab_offset();
                let lib = &mut self.libs[self.tab_idx - lib_off];
                let hit = if let Some(s) = &mut lib.search {
                    if display_pos < s.results.len() { s.cursor = display_pos; true } else { false }
                } else if let Some(lvl) = lib.nav_stack.last_mut() {
                    if display_pos < lvl.items.len() { lvl.cursor = display_pos; true } else { false }
                } else { false };
                return hit;
            }
        }
        false
    }

    pub(super) fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseEventKind, MouseButton};
        let col = mouse.column;
        let row = mouse.row;
        if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
            let now = Instant::now();
            if now.duration_since(self.last_scroll_at) < Duration::from_millis(30) {
                return;
            }
            self.last_scroll_at = now;
        }

        let panel_w: u16 = if self.show_help      { HELP_PANEL_W }
                           else if self.show_settings  { SETTINGS_PANEL_W }
                           else if self.show_sessions  { SESSIONS_PANEL_W }
                           else if self.show_playlists { PLAYLISTS_PANEL_W }
                           else { 0 };
        if panel_w > 0 {
            let pw = panel_w.min(self.terminal_width);
            let inside_panel = col < pw;
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) && !inside_panel {
                if self.show_settings { self.close_settings(); } else { self.show_help = false; self.show_sessions = false; self.show_playlists = false; }
                return;
            }
            if self.show_help {
                match mouse.kind {
                    MouseEventKind::ScrollDown => { self.help_scroll += 3; }
                    MouseEventKind::ScrollUp   => { self.help_scroll = self.help_scroll.saturating_sub(3); }
                    _ => {}
                }
                return;
            }
            if self.show_settings && self.multiselect_popup.is_none() {
                let content_top: u16 = 1;
                let content_bottom = self.terminal_height.saturating_sub(2);
                match mouse.kind {
                    MouseEventKind::ScrollDown => { self.settings_scroll += 3; }
                    MouseEventKind::ScrollUp   => { self.settings_scroll = self.settings_scroll.saturating_sub(3); }
                    MouseEventKind::Down(MouseButton::Left) if row >= content_top && row < content_bottom => {
                        let lines_idx = (row - content_top) as usize + self.settings_scroll;
                        if let Some(cur) = self.settings_line_of_cursor.iter().position(|&l| l == lines_idx) {
                            self.settings_cursor = cur;
                            self.settings_scroll_follow();
                            self.handle_settings_activate();
                        }
                    }
                    _ => {}
                }
                return;
            }
            if self.show_sessions {
                const ENTRY_H: u16 = 4;
                let content_top: u16 = 1;
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if !self.sessions.is_empty() {
                            self.sessions_cursor = (self.sessions_cursor + 1).min(self.sessions.len() - 1);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        self.sessions_cursor = self.sessions_cursor.saturating_sub(1);
                    }
                    MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                        let idx = ((row - content_top) / ENTRY_H) as usize;
                        if idx < self.sessions.len() {
                            self.sessions_cursor = idx;
                        }
                    }
                    _ => {}
                }
                return;
            }
            if self.show_playlists {
                let content_top: u16 = 1;
                if self.playlists_open.is_some() {
                    match mouse.kind {
                        MouseEventKind::ScrollDown => {
                            if !self.playlists_open_items.is_empty() {
                                self.playlists_open_cursor = (self.playlists_open_cursor + 1).min(self.playlists_open_items.len() - 1);
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
                                let pw = PLAYLISTS_PANEL_W.min(self.terminal_width) as usize;
                                let h = if i.display_name().len() <= pw.saturating_sub(6) { 1 } else { 2 };
                                if click_line < y + h { break; }
                                y += h;
                                idx += 1;
                            }
                            if idx < self.playlists_open_items.len() {
                                if self.playlists_open_cursor == idx {
                                    let selected_id = self.playlists_open_items.get(idx).map(|i| i.id.clone());
                                    let pl_source = crate::config::QueueSource::Playlist {
                                        id: self.playlists_open.as_ref().map(|p| p.id.clone()),
                                        name: self.playlists_open.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                                    };
                                    let items: Vec<MediaItem> = self.playlists_open_items.iter().filter(|i| !i.is_folder).cloned().collect();
                                    if !items.is_empty() {
                                        let start = selected_id.as_deref()
                                            .and_then(|id| items.iter().position(|i| i.id == id))
                                            .unwrap_or(0);
                                        let action = PendingQueueAction::PlayItems {
                                            items, start_idx: start, source: pl_source,
                                        };
                                        self.replace_queue_or_prompt(action);
                                        if !self.show_save_playlist_modal {
                                            self.show_playlists = false;
                                            self.set_tab(1);
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
                                self.playlists_cursor = (self.playlists_cursor + 1).min(self.playlists.len() - 1);
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
                return;
            }
            return;
        }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout_tabs_area.contains((col, row).into()) {
                if let Some(idx) = self.tab_idx_at(col) {
                    if idx == usize::MAX - 1 {
                        self.tab_scroll = self.tab_scroll.saturating_sub(1);
                    } else if idx == usize::MAX {
                        let max_scroll = self.tab_count().saturating_sub(1);
                        self.tab_scroll = (self.tab_scroll + 1).min(max_scroll);
                    } else {
                        self.set_tab(idx);
                    }
                }
                return;
            }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout_settings_area.contains((col, row).into()) {
            self.show_settings = !self.show_settings;
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollDown) { 1 } else { -1 };
                if self.layout_tabbar_vol_area.contains((col, row).into())
                    || self.layout_vol_area.contains((col, row).into()) {
                    self.adjust_volume(-delta * 5);
                    return;
                }
                if self.tab_idx == 0 {
                    let sb = self.layout_home_scrollbar;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        let active = self.player.status.lock().unwrap().active;
                        let chrome: u16 = if active { 6 } else { 3 };
                        let panel_h = self.terminal_height.saturating_sub(chrome);
                        let n_sections = 1 + self.home.latest.len();
                        let visible = ((panel_h / HOME_MIN_SECTION_H) as usize).max(1).min(n_sections);
                        let max_offset = n_sections.saturating_sub(visible);
                        self.home_panel_section_offset =
                            (self.home_panel_section_offset as i64 + delta).clamp(0, max_offset as i64) as usize;
                    } else if self.home_rect.contains((col, row).into()) {
                        if self.home_card_view {
                            let n = 1 + self.home.latest.len();
                            self.home.section = (self.home.section as i64 + delta)
                                .clamp(0, n as i64 - 1) as usize;
                            self.ensure_home_section_visible();
                            if !self.card_image_states.is_empty() { self.force_clear = true; }
                        } else {
                            self.move_home_cursor(delta);
                        }
                    }
                } else if self.tab_idx == 1 {
                    if self.playlist_view == 2 {
                        let sb = self.layout_presentation_sb;
                        if sb.width > 0 && sb.contains((col, row).into()) {
                            let n = self.player_tab.items.len();
                            if n > 0 {
                                self.player_tab.playlist_cursor =
                                    (self.player_tab.playlist_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                            }
                            return;
                        }
                    }
                    let n = self.player_tab.items.len();
                    if n > 0 {
                        self.player_tab.playlist_cursor =
                            (self.player_tab.playlist_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    }
                } else if self.tab_idx == self.log_tab_idx() {
                    if delta > 0 { self.log_scroll += 1; }
                    else { self.log_scroll = self.log_scroll.saturating_sub(1); }
                } else {
                    self.move_lib_cursor(delta);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.context_menu.is_some() {
                    if let Some(rect) = self.context_menu_rect {
                        if rect.contains((col, row).into()) {
                            let inner_y = rect.y + 1;
                            if row >= inner_y && (row - inner_y) < self.context_menu.as_ref().unwrap().items.len() as u16 {
                                let idx = (row - inner_y) as usize;
                                let action = self.context_menu.as_ref().unwrap().actions.get(idx).cloned();
                                self.context_menu = None;
                                self.context_menu_rect = None;
                                self.force_clear = true;
                                self.execute_context_action(action);
                            } else {
                                self.context_menu = None;
                                self.force_clear = true;
                            }
                            return;
                        }
                    }
                    self.context_menu = None;
                    self.force_clear = true;
                    return;
                }

                let now = Instant::now();

                if let Some(r) = self.layout_carousel_left_arrow {
                    if r.contains((col, row).into()) {
                        if self.tab_idx == 0 { self.move_home_cursor(-1); }
                        else { if self.player_tab.playlist_cursor > 0 { self.player_tab.playlist_cursor -= 1; } }
                        return;
                    }
                }
                if let Some(r) = self.layout_carousel_right_arrow {
                    if r.contains((col, row).into()) {
                        if self.tab_idx == 0 { self.move_home_cursor(1); }
                        else { let n = self.player_tab.items.len(); if self.player_tab.playlist_cursor + 1 < n { self.player_tab.playlist_cursor += 1; } }
                        return;
                    }
                }
                if self.tab_idx == 0 && self.home_card_view {
                    let strips = self.layout_home_card_strips.clone();
                    for (sec_idx, strip_rect) in &strips {
                        if strip_rect.contains((col, row).into()) && *sec_idx != self.home.section {
                            self.home.section = *sec_idx;
                            if !self.card_image_states.is_empty() { self.force_clear = true; }
                            return;
                        }
                    }
                }
                if let Some(r) = self.layout_carousel_up_arrow {
                    if r.contains((col, row).into()) {
                        if self.home.section > 0 {
                            self.home.section -= 1;
                            self.ensure_home_section_visible();
                        }
                        return;
                    }
                }
                if let Some(r) = self.layout_carousel_down_arrow {
                    if r.contains((col, row).into()) {
                        let n_sections = 1 + self.home.latest.len();
                        if self.home.section + 1 < n_sections {
                            self.home.section += 1;
                            self.ensure_home_section_visible();
                        }
                        return;
                    }
                }

                if self.tab_idx == 1 && self.playlist_view == 1 {
                    let slots = self.layout_carousel_slots;
                    log::info!(target: "mouse",
                        "carousel click ({col},{row}): slots=[({:?},{:?}),({:?},{:?}),({:?},{:?})]",
                        slots[0].0, slots[0].1, slots[1].0, slots[1].1, slots[2].0, slots[2].1
                    );
                    for (slot_idx, (maybe_item_idx, card_rect)) in slots.iter().enumerate() {
                        if card_rect.contains((col, row).into()) {
                            let elapsed_ms = now.duration_since(self.last_carousel_click_time).as_millis();
                            let is_double_slot = self.last_carousel_click_slot == Some(slot_idx)
                                && now.duration_since(self.last_carousel_click_time) < Duration::from_millis(400);
                            log::info!(target: "mouse",
                                "carousel hit slot={slot_idx} item={maybe_item_idx:?} is_double={is_double_slot} elapsed={elapsed_ms}ms last_slot={:?}",
                                self.last_carousel_click_slot);
                            self.last_carousel_click_slot = Some(slot_idx);
                            self.last_carousel_click_time = now;
                            if slot_idx == 1 {
                                if is_double_slot {
                                    if let Some(item_idx) = maybe_item_idx {
                                        let (active, active_idx) = {
                                            let s = self.player.status.lock().unwrap();
                                            (s.active, s.current_idx)
                                        };
                                        log::info!(target: "mouse",
                                            "carousel dbl-center: active={active} active_idx={active_idx} item_idx={item_idx}");
                                        if active && active_idx == *item_idx {
                                            self.player.send_command(PlayerCommand::TogglePause);
                                        } else if active {
                                            self.player.send_command(PlayerCommand::JumpTo(*item_idx))
                                        } else if !self.player_tab.items.is_empty() {
                                            let items = self.player_tab.items.clone();
                                            let item_idx = *item_idx;
                                            self.play_items_routed(items, item_idx);
                                        }
                                    }
                                }
                            } else if let Some(item_idx) = maybe_item_idx {
                                self.player_tab.playlist_cursor = *item_idx;
                            }
                            return;
                        }
                    }
                    let strip_slots = self.layout_queue_strip_slots.clone();
                    for (item_idx, rect) in &strip_slots {
                        if rect.contains((col, row).into()) {
                            self.player_tab.playlist_cursor = *item_idx;
                            if !self.card_image_states.is_empty() { self.force_clear = true; }
                            return;
                        }
                    }
                    log::info!(target: "mouse", "carousel click ({col},{row}): no slot hit");
                    if self.layout_playlist_inner.contains((col, row).into()) {
                        return;
                    }
                }

                let is_double = now.duration_since(self.last_click_time) < Duration::from_millis(400)
                    && self.last_click_pos == (col, row);
                self.last_click_time = now;
                self.last_click_pos = (col, row);

                if is_double {
                    if self.layout_seekbar_area.contains((col, row).into()) {
                        self.seek_to_col(col);
                        return;
                    }
                    if self.tab_idx == 0 {
                        if self.home_rect.contains((col, row).into()) { self.select_home(); }
                    } else if self.tab_idx == 1 {
                        let t = self.player_tab.playlist_cursor;
                        if t < self.player_tab.items.len() && self.layout_playlist_inner.contains((col, row).into()) {
                            if let Some(ref conn_id) = self.connected_session_id.clone() {
                                let item = self.player_tab.items[t].clone();
                                let id = conn_id.clone();
                                let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                                let start_ticks = item.playback_position_ticks;
                                let label = item.playback_label();
                                self.flash_status(format!("Playing on remote: {label}"));
                                self.do_session_command(move |c| c.session_play_items(&id, &item_ids, t, start_ticks));
                            } else {
                                self.player.send_command(PlayerCommand::JumpTo(t));
                            }
                        }
                    } else if self.tab_idx != self.log_tab_idx() {
                        self.select();
                    }
                    return;
                }

                if self.layout_button_area.contains((col, row).into()) {
                    let btn = (col.saturating_sub(self.layout_button_area.x) / 5) as usize;
                    if btn < 6 { self.handle_button_click(btn); }
                    return;
                }

                if self.layout_sub_area.contains((col, row).into()) {
                    self.toggle_sub();
                    return;
                }
                if self.layout_audio_area.contains((col, row).into()) {
                    if self.is_audio_item() { self.toggle_mute(); } else { self.cycle_audio(); }
                    return;
                }
                if self.layout_vol_area.contains((col, row).into()) {
                    self.adjust_volume(-5);
                    return;
                }

                if self.tab_idx == 0 {
                    let sb = self.layout_home_scrollbar;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        self.home_scrollbar_seek(row);
                        return;
                    }
                }

                if self.tab_idx == 1 && self.playlist_view == 2 {
                    let sb = self.layout_presentation_sb;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        self.presentation_scrollbar_seek(row);
                        return;
                    }
                }

                if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
                    let crumbs = self.layout_breadcrumbs.clone();
                    let lib_off = self.lib_tab_offset();
                    for (x_start, x_end, crumb_row, target_depth) in crumbs {
                        if row == crumb_row && col >= x_start && col < x_end {
                            let lib = &mut self.libs[self.tab_idx - lib_off];
                            lib.nav_stack.truncate(target_depth);
                            lib.search = None;
                            return;
                        }
                    }
                }
                self.click_set_cursor(col, row);
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.layout_vol_area.contains((col, row).into()) {
                    self.adjust_volume(5);
                    return;
                }
                if (self.tab_idx == 1 && self.playlist_view == 1) || (self.tab_idx == 0 && self.home_card_view) {
                    let slots = self.layout_carousel_slots;
                    for (maybe_item_idx, card_rect) in slots.iter() {
                        if card_rect.contains((col, row).into()) {
                            if let Some(item_idx) = maybe_item_idx {
                                if self.tab_idx == 1 {
                                    self.player_tab.playlist_cursor = *item_idx;
                                } else {
                                    let sec = self.home.section;
                                    self.set_home_cursor(sec, *item_idx);
                                }
                                let cx = card_rect.x + card_rect.width / 2;
                                let cy = card_rect.y + card_rect.height / 2;
                                self.open_context_menu_at(cx, cy);
                            }
                            return;
                        }
                    }
                    return;
                }
                if self.click_set_cursor(col, row) {
                    self.open_context_menu_at(col, row);
                }
            }
            MouseEventKind::Drag(MouseButton::Left)
                if self.tab_idx == 0 && {
                    let sb = self.layout_home_scrollbar;
                    sb.width > 0 && sb.contains((col, row).into())
                } => {
                    self.home_scrollbar_seek(row);
                }
            MouseEventKind::Drag(MouseButton::Left)
                if self.tab_idx == 1 && self.playlist_view == 2 && {
                    let sb = self.layout_presentation_sb;
                    sb.width > 0 && sb.contains((col, row).into())
                } => {
                    self.presentation_scrollbar_seek(row);
                }
            MouseEventKind::Drag(MouseButton::Left)
                if self.layout_seekbar_area.contains((col, row).into())
                    && self.last_drag_seek.elapsed() >= Duration::from_millis(150)
                => {
                    self.last_drag_seek = Instant::now();
                    self.seek_to_col(col);
                }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Right) => {
                if let (Some(ref mut menu), Some(rect)) = (&mut self.context_menu, self.context_menu_rect) {
                    let inner_y = rect.y + 1;
                    if rect.contains((col, row).into()) && row >= inner_y {
                        let idx = (row - inner_y) as usize;
                        if idx < menu.items.len() {
                            menu.cursor = idx;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
