use super::settings::settings_total_rows;
use super::ui_util::item_text_and_style;
use super::{
    App, ContextAction, ContextMenu, HomeSearch, LibSearch, PendingQueueAction, QueueScope,
    SavePlaylistDialog, SavePlaylistStage, HELP_PANEL_W, HOME_MIN_SECTION_H, PLAYLISTS_PANEL_W,
    POWER_LEFT_WIDTH_DEFAULT, POWER_LEFT_WIDTH_STEP, SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use super::{PowerFocus, QUEUE_VIEW_COUNT, QUEUE_VIEW_POWER};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Borders};
use std::sync::Arc;
use std::time::{Duration, Instant};
use textwrap::wrap;

impl App {
    fn context_menu_play_state(&self, item: &MediaItem) -> bool {
        if item.is_folder {
            item.unplayed_item_count == 0
        } else {
            item.played
        }
    }

    fn context_menu_power_lib_idx(&self) -> Option<usize> {
        if self.tab_idx == 1
            && self.queue_view == QUEUE_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab > 0
        {
            Some(self.power_left_tab - 1)
        } else {
            None
        }
    }

    fn context_menu_lib_idx(&self) -> Option<usize> {
        if let Some(lib_idx) = self.context_menu_power_lib_idx() {
            Some(lib_idx)
        } else if self.tab_idx >= self.lib_tab_offset() {
            Some(self.tab_idx - self.lib_tab_offset())
        } else {
            None
        }
    }

    fn podcast_mark_all_ids(&self, lib_idx: usize) -> Vec<String> {
        let mut ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in self.feed_home_video_selected_items(lib_idx) {
            if item.is_folder || item.played {
                continue;
            }
            if seen.insert(item.id.clone()) {
                ids.push(item.id);
            }
        }
        ids
    }

    fn podcast_mark_all_unplayed_ids(&self, lib_idx: usize) -> Vec<String> {
        let mut ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in self.feed_home_video_selected_items(lib_idx) {
            if item.is_folder || !item.played {
                continue;
            }
            if seen.insert(item.id.clone()) {
                ids.push(item.id);
            }
        }
        ids
    }

    fn push_context_action(
        entries: &mut Vec<super::ContextMenuEntry>,
        label: &'static str,
        action: ContextAction,
    ) {
        entries.push(super::ContextMenuEntry {
            label,
            action: Some(action),
        });
    }

    fn push_context_separator(entries: &mut Vec<super::ContextMenuEntry>) {
        entries.push(super::ContextMenuEntry {
            label: "────────",
            action: None,
        });
    }

    pub(super) fn tab_count(&self) -> usize {
        2 + self.libs.len()
    }
    pub(super) fn lib_tab_offset(&self) -> usize {
        2
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
        for entry in super::input_resolver::CONTEXT_STACK {
            if let Some(quit) = (entry.handler)(self, key) {
                return quit;
            }
        }
        false
    }

    pub(super) fn handle_key_save_playlist_entry(&mut self, key: KeyEvent) -> Option<bool> {
        if self.save_playlist_dialog.is_some() {
            Some(self.handle_save_playlist_key(key))
        } else {
            None
        }
    }

    pub(super) fn handle_key_global_overlay_open(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code == KeyCode::F(1) {
            self.show_help = true;
            return Some(false);
        }
        if key.code == KeyCode::F(2) {
            self.show_settings = !self.show_settings;
            return Some(false);
        }
        if key.code == KeyCode::F(3) {
            self.show_sessions = true;
            self.spawn_sessions_load();
            return Some(false);
        }
        if key.code == KeyCode::F(4) {
            self.open_playlists_panel();
            return Some(false);
        }
        None
    }

    pub(super) fn handle_key_legacy_tail(&mut self, key: KeyEvent) -> Option<bool> {
        if self.handle_power_left_width_key(key) {
            return Some(false);
        }
        // Alt+Left/Right cycle type filter when home search is active
        if (self.tab_idx == 0 || self.tab_idx == 1)
            && key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self.home_search.is_some()
            && self.context_menu.is_none()
        {
            match key.code {
                KeyCode::Left | KeyCode::Right => {
                    if let Some(ref mut hs) = self.home_search {
                        let n = hs.available_types().len() + 1; // +1 for "All"
                        if n > 1 {
                            hs.type_filter = if key.code == KeyCode::Right {
                                (hs.type_filter + 1) % n
                            } else {
                                (hs.type_filter + n - 1) % n
                            };
                            hs.cursor = 0;
                            hs.scroll = 0;
                        }
                    }
                    return Some(false);
                }
                _ => {}
            }
        }
        // When home search is active, unmodified keys feed the search input
        if (self.tab_idx == 0 || self.tab_idx == 1)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self.home_search.is_some()
            && self.context_menu.is_none()
        {
            let input_focused = self.home_search.as_ref().is_none_or(|s| s.input_focused);
            match key.code {
                KeyCode::Esc => {
                    self.home_search = None;
                }
                KeyCode::Tab => {
                    if let Some(ref mut hs) = self.home_search {
                        hs.input_focused = !hs.input_focused;
                    }
                }
                KeyCode::Backspace if input_focused => {
                    let empty = self.home_search.as_ref().is_none_or(|s| s.query.is_empty());
                    if empty {
                        self.home_search = None;
                    } else {
                        self.home_search.as_mut().unwrap().query.pop();
                    }
                }
                KeyCode::Up => {
                    if let Some(ref mut hs) = self.home_search {
                        hs.cursor = hs.cursor.saturating_sub(1);
                        if hs.cursor < hs.scroll {
                            hs.scroll = hs.cursor;
                        }
                    }
                }
                KeyCode::Down => {
                    if let Some(ref mut hs) = self.home_search {
                        let max = hs.filtered_count().saturating_sub(1);
                        hs.cursor = (hs.cursor + 1).min(max);
                    }
                }
                KeyCode::Enter => {
                    let (query, last_query, loading, has_results) = self
                        .home_search
                        .as_ref()
                        .map(|hs| {
                            (
                                hs.query.clone(),
                                hs.last_query.clone(),
                                hs.loading,
                                !hs.results.is_empty(),
                            )
                        })
                        .unwrap_or_default();
                    if loading {
                        return Some(false);
                    }
                    if !input_focused {
                        if has_results {
                            self.select_home();
                        }
                        return Some(false);
                    }
                    if query.is_empty() {
                        return Some(false);
                    }
                    if query != last_query {
                        if let Some(ref mut hs) = self.home_search {
                            hs.last_query = query.clone();
                            hs.loading = true;
                            hs.results.clear();
                            hs.cursor = 0;
                            hs.scroll = 0;
                        }
                        self.spawn_global_search(query);
                    } else if has_results {
                        self.select_home();
                    }
                }
                KeyCode::Char('q') if !input_focused && key.modifiers.is_empty() => {
                    return Some(self.try_quit());
                }
                KeyCode::Char(c) => {
                    if let Some(ref mut hs) = self.home_search {
                        hs.input_focused = true;
                        hs.query.push(c);
                    }
                }
                _ => {}
            }
            return Some(false);
        }
        // Power-view: when the left panel is focused on a library with active search, intercept keys
        if self.queue_view == QUEUE_VIEW_POWER
            && !key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self.context_menu.is_none()
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab > 0
        {
            let lib_idx = self.power_left_tab - 1;
            if self.libs[lib_idx].search.is_some() {
                self.handle_lib_search_key(lib_idx, key);
                return Some(false);
            }
        }
        // When library search is active, unmodified keys feed the search
        if self.tab_idx > 1
            && !key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self
                .libs
                .get(self.tab_idx - self.lib_tab_offset())
                .is_some_and(|l| l.search.is_some())
            && self.context_menu.is_none()
        {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            self.handle_lib_search_key(lib_idx, key);
            return Some(false);
        }
        if key.code == KeyCode::Char('h') {
            let active = self.player.status.lock().unwrap().active;
            let show_controls = active || self.connected_session_id.is_some();
            if show_controls {
                self.panel_mode = self.panel_mode.next();
            }
            return Some(false);
        }
        let in_lib_search = self.tab_idx > 1
            && self
                .libs
                .get(self.tab_idx - self.lib_tab_offset())
                .is_some_and(|l| l.search.is_some());
        if self.confirm_clear_queue {
            self.confirm_clear_queue = false;
            if matches!(
                key.code,
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
            ) {
                self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
            } else {
                self.status.clear();
            }
            return Some(false);
        }
        if self.confirm_rescan {
            self.confirm_rescan = false;
            self.status.clear();
            if matches!(
                key.code,
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
            ) {
                let lib_idx = self.tab_idx - self.lib_tab_offset();
                self.trigger_lib_rescan(lib_idx);
            }
            return Some(false);
        }
        if self.skip_intro_end_ticks.is_some() {
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
            return Some(false);
        }
        if self.next_up_item.is_some() {
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
            return Some(false);
        }
        if key.code == KeyCode::Char('c')
            && !key.modifiers.contains(KeyModifiers::ALT)
            && !in_lib_search
        {
            if self.tab_idx == 1 && self.visible_queue_scope() == QueueScope::Remote {
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
            self.status = "Clear queue? (Y/n)".into();
            self.confirm_clear_queue = true;
            return Some(false);
        }
        None
    }

    pub(super) fn handle_key_power_queue_alt_m(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code == KeyCode::Char('m')
            && key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && self.tab_idx == 1
            && self.queue_view == QUEUE_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab > 0
        {
            Some(self.handle_queue_key(key))
        } else {
            None
        }
    }

    pub(super) fn handle_key_ctrl_l(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.force_clear = true;
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn handle_key_f5_refresh(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code == KeyCode::F(5) {
            self.refresh_current_view();
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn handle_key_view_dispatch(&mut self, key: KeyEvent) -> Option<bool> {
        Some(if self.tab_idx == 0 {
            self.handle_combined_key(key)
        } else if self.tab_idx == 1 {
            self.handle_queue_key(key)
        } else {
            self.handle_lib_key(key)
        })
    }

    fn handle_lib_search_key(&mut self, lib_idx: usize, key: KeyEvent) {
        let saved = self.tab_idx;
        self.tab_idx = self.lib_tab_offset() + lib_idx;
        match key.code {
            KeyCode::Esc => {
                self.libs[lib_idx].search = None;
            }
            KeyCode::Backspace => {
                let empty = self.libs[lib_idx]
                    .search
                    .as_ref()
                    .is_none_or(|s| s.query.is_empty());
                if empty {
                    self.libs[lib_idx].search = None;
                } else {
                    self.libs[lib_idx].search.as_mut().unwrap().query.pop();
                    self.update_lib_search(lib_idx);
                }
            }
            KeyCode::Up => self.move_lib_cursor(-1),
            KeyCode::Down => self.move_lib_cursor(1),
            KeyCode::PageUp => {
                let p = self.lib_page_size();
                self.move_lib_cursor(-(p as i64));
            }
            KeyCode::PageDown => {
                let p = self.lib_page_size();
                self.move_lib_cursor(p as i64);
            }
            KeyCode::Home => self.jump_lib_cursor(false),
            KeyCode::End => self.jump_lib_cursor(true),
            KeyCode::Enter => self.select(),
            KeyCode::Char(c) => {
                self.libs[lib_idx].search.as_mut().unwrap().query.push(c);
                self.update_lib_search(lib_idx);
            }
            _ => {}
        }
        self.tab_idx = saved;
    }

    pub(super) fn handle_key_save_modal(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.show_save_playlist_modal {
            return None;
        }
        let quit_after = matches!(self.pending_queue_action, Some(PendingQueueAction::Quit));
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
                    self.set_tab(1);
                }
                if quit_after {
                    return Some(true);
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.show_save_playlist_modal = false;
                if let Some(action) = self.pending_queue_action.take() {
                    self.execute_pending_queue_action(action);
                }
                if play_after {
                    self.show_playlists = false;
                    self.set_tab(1);
                }
                if quit_after {
                    return Some(true);
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
        let snapshot = self.input_snapshot();
        match super::input_resolver::resolve_key(
            super::input_resolver::InputContext::Help,
            &snapshot,
            super::input_resolver::KeyChord::from_key(key),
        ) {
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
        Some(false)
    }

    pub(super) fn handle_key_context_menu(&mut self, key: KeyEvent) -> Option<bool> {
        self.context_menu.as_ref()?;
        match key.code {
            KeyCode::Esc => {
                self.context_menu = None;
                self.force_clear = true;
            }
            KeyCode::Up => {
                if let Some(m) = &mut self.context_menu {
                    m.move_cursor(-1);
                }
            }
            KeyCode::Down => {
                if let Some(m) = &mut self.context_menu {
                    m.move_cursor(1);
                }
            }
            KeyCode::Enter => {
                if let Some(m) = self.context_menu.take() {
                    self.force_clear = true;
                    let action = m
                        .entries
                        .get(m.cursor)
                        .and_then(|entry| entry.action.clone());
                    self.execute_context_action(action);
                }
            }
            _ => {}
        }
        Some(false)
    }

    fn handle_lib_key(&mut self, key: KeyEvent) -> bool {
        let lib_idx = self.tab_idx - self.lib_tab_offset();

        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enqueue_selected()
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::ALT => self.enqueue_selected(),
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                return self.try_quit();
            }
            KeyCode::Tab => {
                let n = (self.tab_idx + 1) % self.tab_count();
                self.set_tab(n);
            }
            KeyCode::BackTab => {
                let n = self.tab_count();
                self.set_tab((self.tab_idx + n - 1) % n);
            }
            KeyCode::Esc | KeyCode::Backspace => self.go_back(),
            KeyCode::Up => self.move_lib_cursor(if self.is_viewing_season_grid(lib_idx) {
                -4
            } else {
                -1
            }),
            KeyCode::Down => self.move_lib_cursor(if self.is_viewing_season_grid(lib_idx) {
                4
            } else {
                1
            }),
            KeyCode::Left if self.is_viewing_season_grid(lib_idx) => self.move_lib_cursor(-1),
            KeyCode::Right if self.is_viewing_season_grid(lib_idx) => self.move_lib_cursor(1),
            KeyCode::PageUp => {
                let p = self.lib_page_size();
                self.move_lib_cursor(-(p as i64));
            }
            KeyCode::PageDown => {
                let p = self.lib_page_size();
                self.move_lib_cursor(p as i64);
            }
            KeyCode::Home => self.jump_lib_cursor(false),
            KeyCode::End => self.jump_lib_cursor(true),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let item = self.current_lib_item();
                if let Some(item) = item {
                    if item.is_folder {
                        let ct = self.libs[self.tab_idx - self.lib_tab_offset()]
                            .library
                            .collection_type
                            .clone();
                        self.queue_source = crate::config::QueueSource::Collection {
                            collection_type: ct,
                        };
                        self.play_folder(&item.id.clone());
                        self.save_queue_state();
                    } else {
                        self.select();
                    }
                }
            }
            KeyCode::Enter => self.select(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_watched()
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.shuffle_play()
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let name = self.libs[lib_idx].library.name.clone();
                self.status = format!("Rescan '{name}'? (Y/n)");
                self.confirm_rescan = true;
            }
            KeyCode::Char('r') => self.refresh_lib(),
            KeyCode::Char('.') => self.open_context_menu(),
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() {
                    self.set_tab(idx);
                }
            }
            KeyCode::Char('/') => {
                let (items, needs_full_load) = if self.is_feed_home_video_group_view(lib_idx) {
                    (self.feed_home_video_selected_items(lib_idx), false)
                } else {
                    self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| {
                            let all = l.all_items.clone().unwrap_or_else(|| l.items.clone());
                            let needs = l.all_items.is_none() && l.items.len() < l.total_count;
                            (all, needs)
                        })
                        .unwrap_or_default()
                };
                let n = items.len();
                self.libs[lib_idx].search = Some(LibSearch {
                    query: String::new(),
                    items,
                    results: (0..n).collect(),
                    cursor: 0,
                    scroll: 0,
                    loading: needs_full_load,
                });
                if needs_full_load {
                    self.spawn_search_items_load(lib_idx);
                }
                self.update_lib_search(lib_idx);
            }
            _ => {
                return false;
            }
        }
        false
    }

    fn handle_combined_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::ALT => {
                self.enqueue_selected();
                return false;
            }
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                return self.try_quit();
            }
            KeyCode::Tab => {
                let n = (self.tab_idx + 1) % self.tab_count();
                self.set_tab(n);
                return false;
            }
            KeyCode::BackTab => {
                let n = self.tab_count();
                self.set_tab((self.tab_idx + n - 1) % n);
                return false;
            }
            KeyCode::Left | KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                let n = 1 + self.home.latest.len();
                self.home.section = (self.home.section + n - 1) % n;
                self.ensure_home_section_visible();
                if self.home_card_view && !self.card_image_states.is_empty() {
                    self.force_clear = true;
                }
                return false;
            }
            KeyCode::Right | KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                let n = 1 + self.home.latest.len();
                self.home.section = (self.home.section + 1) % n;
                self.ensure_home_section_visible();
                if self.home_card_view && !self.card_image_states.is_empty() {
                    self.force_clear = true;
                }
                return false;
            }
            KeyCode::Char('v') => {
                if self.images_enabled() {
                    self.home_card_view = !self.home_card_view;
                    if !self.card_image_states.is_empty() {
                        self.force_clear = true;
                    }
                }
                return false;
            }
            KeyCode::Char('.') => {
                self.open_context_menu();
                return false;
            }
            KeyCode::Char('/') => {
                self.home_search = Some(HomeSearch {
                    query: String::new(),
                    last_query: String::new(),
                    results: Vec::new(),
                    cursor: 0,
                    loading: false,
                    scroll: 0,
                    type_filter: 0,
                    input_focused: true,
                });
                return false;
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() {
                    self.set_tab(idx);
                }
                return false;
            }
            _ => {}
        }
        match key.code {
            KeyCode::Up => {
                if self.home_card_view {
                    self.home.section = self.home.section.saturating_sub(1);
                    self.ensure_home_section_visible();
                    if !self.card_image_states.is_empty() {
                        self.force_clear = true;
                    }
                } else {
                    self.move_home_cursor(-1);
                }
            }
            KeyCode::Down => {
                if self.home_card_view {
                    let n = 1 + self.home.latest.len();
                    self.home.section = (self.home.section + 1).min(n.saturating_sub(1));
                    self.ensure_home_section_visible();
                    if !self.card_image_states.is_empty() {
                        self.force_clear = true;
                    }
                } else {
                    self.move_home_cursor(1);
                }
            }
            KeyCode::Left => {
                if self.home_card_view {
                    self.move_home_cursor(-1);
                }
            }
            KeyCode::Right => {
                if self.home_card_view {
                    self.move_home_cursor(1);
                }
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enqueue_selected()
            }
            KeyCode::Enter => self.select_home(),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_watched_home()
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enqueue_selected()
            }
            KeyCode::Delete if self.home.section == 0 => self.remove_from_continue_watching(),
            _ => {}
        }
        false
    }

    pub(super) fn adjust_volume(&mut self, delta: i64) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let vol = self
                .connected_session_state
                .as_ref()
                .map(|s| s.volume)
                .unwrap_or(50);
            let new_vol = (vol + delta).clamp(0, 100);
            let id = conn_id.clone();
            self.do_session_command(move |c| c.session_set_volume(&id, new_vol));
            return;
        }
        let active = self.player.status.lock().unwrap().active;
        if active {
            let st = self.player.status.lock().unwrap();
            let v = (st.volume + delta).clamp(0, st.volume_max) as u8;
            drop(st);
            self.player.send_command(PlayerCommand::SetVolume(v as i64));
            self.ui_volume = v;
        } else {
            self.ui_volume = (self.ui_volume as i64 + delta).clamp(0, 200) as u8;
        }
        self.save_prefs();
    }

    pub(super) fn handle_playback_key(&mut self, key: KeyEvent) -> Option<bool> {
        let snapshot = self.input_snapshot();
        match super::input_resolver::resolve_key(
            super::input_resolver::InputContext::Playback,
            &snapshot,
            super::input_resolver::KeyChord::from_key(key),
        ) {
            super::input_resolver::KeyResolution::Command(cmd) => Some(self.dispatch(cmd)),
            // Swallow is unreachable for Playback today; both non-command outcomes
            // mean "not a playback key" → let it fall through (`None`).
            super::input_resolver::KeyResolution::FallThrough
            | super::input_resolver::KeyResolution::Swallow => None,
        }
    }

    /// Handle a key for the focused power-view home list (all groups: CW + library latest).
    /// Returns true if the key was consumed (others fall through to focus nav).
    fn handle_power_cw_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Up => {
                self.power_home_move_up();
                true
            }
            KeyCode::Down => {
                self.power_home_move_down();
                true
            }
            KeyCode::Char('[') if !ctrl => {
                self.power_home_move_section(-1);
                true
            }
            KeyCode::Char(']') if !ctrl => {
                self.power_home_move_section(1);
                true
            }
            KeyCode::PageUp => {
                self.power_home_move_cursor(-(self.power_cw_page() as i64));
                true
            }
            KeyCode::PageDown => {
                self.power_home_move_cursor(self.power_cw_page() as i64);
                true
            }
            KeyCode::Home => {
                self.home.power_home_cursor = 0;
                true
            }
            KeyCode::End => {
                let total = self.home.continue_items.len()
                    + self
                        .home
                        .latest
                        .iter()
                        .map(|(_, _, v, _)| v.len())
                        .sum::<usize>();
                if total > 0 {
                    self.home.power_home_cursor = total - 1;
                }
                true
            }
            KeyCode::Enter if ctrl => {
                self.power_home_enqueue();
                true
            }
            KeyCode::Enter => {
                self.power_home_play();
                true
            }
            KeyCode::Char('q') if ctrl => {
                self.power_home_enqueue();
                true
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::ALT => {
                self.power_home_enqueue();
                true
            }
            KeyCode::Char('w') if ctrl => {
                self.power_cw_toggle_watched();
                true
            }
            KeyCode::Char('.') => {
                self.open_context_menu();
                true
            }
            KeyCode::Delete => {
                let cursor = self.home.power_home_cursor;
                let cw_len = self.home.continue_items.len();
                if cursor < cw_len {
                    let saved = self.home.continue_cursor;
                    self.home.continue_cursor = cursor;
                    self.remove_from_continue_watching();
                    self.home.continue_cursor = saved;
                }
                true
            }
            _ => false,
        }
    }

    fn power_cw_page(&self) -> usize {
        (self.layout.power.left_area.height as usize).max(1)
    }

    fn is_power_left_width_resize_key(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Left | KeyCode::Right) && key.modifiers == KeyModifiers::SHIFT
    }

    fn power_view_active(&self) -> bool {
        self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER
    }

    fn handle_power_left_width_key(&mut self, key: KeyEvent) -> bool {
        if !self.power_view_active()
            || self.context_menu.is_some()
            || !Self::is_power_left_width_resize_key(key)
        {
            return false;
        }

        let max_width = Self::power_left_width_max_for_terminal(self.terminal_width);
        let next_width = if key.code == KeyCode::Left {
            self.power_left_width.saturating_sub(POWER_LEFT_WIDTH_STEP)
        } else {
            self.power_left_width.saturating_add(POWER_LEFT_WIDTH_STEP)
        };
        let normalized = Self::normalize_power_left_width(next_width, self.terminal_width);
        if normalized == self.power_left_width {
            let limit = if key.code == KeyCode::Left {
                format!("Power view width already at minimum ({POWER_LEFT_WIDTH_DEFAULT} cols)")
            } else {
                format!("Power view width already at maximum ({max_width} cols)")
            };
            self.flash_status(limit);
            return true;
        }

        self.power_left_width = normalized;
        self.save_prefs();
        self.flash_status(format!("Power view width: {} cols", self.power_left_width));
        true
    }

    fn handle_queue_key(&mut self, key: KeyEvent) -> bool {
        if let Some(t) = self.confirm_remove_idx {
            self.confirm_remove_idx = None;
            self.status.clear();
            if matches!(key.code, KeyCode::Char('y')) {
                // Defer the actual removal until PlayerEvent::Stopped arrives so the
                // Stopped handler finds the correct item at index t, not the next item
                // (which would have its playback_position_ticks corrupted otherwise).
                self.pending_delete_idx = Some(t);
                self.player.stop();
                if self.local_queue_metadata_applies(self.visible_queue_scope()) {
                    self.queue_dirty = true;
                }
            }
            return false;
        }

        // In power view, bare Left/Right switch focus between the two panels.
        // Queue is on the left; library is on the right.
        if self.queue_view == QUEUE_VIEW_POWER && key.modifiers.is_empty() {
            if key.code == KeyCode::Right && matches!(self.power_focus, PowerFocus::Queue) {
                self.power_focus = PowerFocus::Left;
                self.last_card_height = 0; // reset stale image height for new view
                return false;
            }
            if key.code == KeyCode::Left && matches!(self.power_focus, PowerFocus::Left) {
                self.power_focus = PowerFocus::Queue;
                self.last_card_height = 0;
                return false;
            }
        }

        // Power view bracket keys are panel-scoped; the queue panel owns
        // Local/Remote scope switching, while the left panel keeps its
        // section/season/group bracket actions.
        if self.tab_idx == 1
            && self.queue_view == QUEUE_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Queue)
        {
            match key.code {
                KeyCode::Char('[')
                    if self.has_direct_remote_queue()
                        && !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.set_queue_scope(QueueScope::Local);
                    return false;
                }
                KeyCode::Char(']')
                    if self.has_direct_remote_queue()
                        && !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return false;
                }
                _ => {}
            }
        }

        // In power view, route nav keys to the focused library panel.
        if self.queue_view == QUEUE_VIEW_POWER && matches!(self.power_focus, PowerFocus::Left) {
            if self.power_left_tab == 0 && self.handle_power_cw_key(key) {
                return false;
            }
            if self.power_left_tab > 0 {
                let lib_idx = self.power_left_tab - 1;

                // Detail mode: scroll overview, Enter plays, Backspace/Esc dismisses.
                // Nav keys are consumed to prevent cursor movement in the underlying list.
                // All other keys (q, /, Tab, etc.) fall through to their normal handlers.
                if self.libs[lib_idx].power_detail_item.is_some() {
                    match key.code {
                        KeyCode::Enter => {
                            let saved = self.tab_idx;
                            self.tab_idx = self.lib_tab_offset() + lib_idx;
                            self.select();
                            self.tab_idx = saved;
                            return false;
                        }
                        KeyCode::Backspace | KeyCode::Esc => {
                            self.libs[lib_idx].power_detail_item = None;
                            return false;
                        }
                        KeyCode::Char('m')
                            if key.modifiers.contains(KeyModifiers::ALT)
                                && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            self.libs[lib_idx].power_detail_item = None;
                            return false;
                        }
                        KeyCode::Up => {
                            self.libs[lib_idx].power_detail_scroll =
                                self.libs[lib_idx].power_detail_scroll.saturating_sub(1);
                            return false;
                        }
                        KeyCode::Down => {
                            self.libs[lib_idx].power_detail_scroll =
                                (self.libs[lib_idx].power_detail_scroll + 1)
                                    .min(self.layout.power.detail_max_scroll);
                            return false;
                        }
                        KeyCode::PageUp => {
                            self.libs[lib_idx].power_detail_scroll = self.libs[lib_idx]
                                .power_detail_scroll
                                .saturating_sub(self.layout.power.detail_page_h);
                            return false;
                        }
                        KeyCode::PageDown => {
                            self.libs[lib_idx].power_detail_scroll = (self.libs[lib_idx]
                                .power_detail_scroll
                                + self.layout.power.detail_page_h)
                                .min(self.layout.power.detail_max_scroll);
                            return false;
                        }
                        // Left/Right/Home/End: swallow to block underlying list nav.
                        KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                            return false;
                        }
                        // Everything else (q, /, Tab, …) falls through to normal handlers.
                        _ => {}
                    }
                }

                if key.code == KeyCode::Char('m')
                    && key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    if let Some(item) = self.power_selected_movie_item(lib_idx) {
                        self.libs[lib_idx].power_detail_item = Some(item);
                        self.libs[lib_idx].power_detail_scroll = 0;
                    }
                    return false;
                }

                // Season switching: [ = previous season, ] = next season.
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    if key.code == KeyCode::Char('[') && self.is_series_view(lib_idx) {
                        self.switch_season(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.is_series_view(lib_idx) {
                        self.switch_season(lib_idx, 1);
                        return false;
                    }
                    if key.code == KeyCode::Char('[') && self.is_music_group_view(lib_idx) {
                        self.switch_music_group(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.is_music_group_view(lib_idx) {
                        self.switch_music_group(lib_idx, 1);
                        return false;
                    }
                    if key.code == KeyCode::Char('[') && self.is_feed_home_video_group_view(lib_idx)
                    {
                        self.switch_feed_folder_group(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.is_feed_home_video_group_view(lib_idx)
                    {
                        self.switch_feed_folder_group(lib_idx, 1);
                        return false;
                    }
                }

                let is_power_nav = matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                ) && key.modifiers.contains(KeyModifiers::ALT);
                let is_lib_key = !is_power_nav
                    && match key.code {
                        KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::PageUp
                        | KeyCode::PageDown
                        | KeyCode::Home
                        | KeyCode::End
                        | KeyCode::Enter
                        | KeyCode::Esc
                        | KeyCode::Backspace => true,
                        KeyCode::Char('/') => true,
                        KeyCode::Char('q') => {
                            key.modifiers.is_empty()
                                || key.modifiers == KeyModifiers::CONTROL
                                || key.modifiers == KeyModifiers::ALT
                        }
                        KeyCode::Char('.') => true,
                        KeyCode::Char('r') => true,
                        KeyCode::Char('1'..='9') => true,
                        KeyCode::Char(_)
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                || key.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            true
                        }
                        _ => false,
                    };
                if is_lib_key {
                    let is_quit_key =
                        matches!(key.code, KeyCode::Char('q') if key.modifiers.is_empty());
                    let saved = self.tab_idx;
                    self.tab_idx = self.lib_tab_offset() + lib_idx;
                    let result = self.handle_lib_key(key);
                    self.tab_idx = saved;
                    if is_quit_key {
                        return result;
                    }
                    return false;
                }
            }
        }

        // Power view queue focus: PageUp/PageDown use the actual queue panel height.
        if self.queue_view == QUEUE_VIEW_POWER && matches!(self.power_focus, PowerFocus::Queue) {
            let page = self.layout.power.queue_area.height.saturating_sub(1).max(1) as usize;
            match key.code {
                KeyCode::PageUp => {
                    self.last_nav_at = Instant::now();
                    let queue = self.displayed_queue_mut();
                    queue.queue_cursor = queue.queue_cursor.saturating_sub(page);
                    return false;
                }
                KeyCode::PageDown => {
                    self.last_nav_at = Instant::now();
                    let queue = self.displayed_queue_mut();
                    let n = queue.items.len();
                    queue.queue_cursor = (queue.queue_cursor + page).min(n.saturating_sub(1));
                    return false;
                }
                _ => {}
            }
        }

        // Non-power queue view: scope switching via [ / ].
        if self.tab_idx == 1 && self.queue_view != QUEUE_VIEW_POWER {
            match key.code {
                KeyCode::Char('[')
                    if self.has_direct_remote_queue()
                        && !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.set_queue_scope(QueueScope::Local);
                    return false;
                }
                KeyCode::Char(']')
                    if self.has_direct_remote_queue()
                        && !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return false;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                return self.try_quit();
            }
            KeyCode::Tab => {
                if self.queue_view == QUEUE_VIEW_POWER {
                    self.power_left_tab_next();
                } else {
                    let n = (self.tab_idx + 1) % self.tab_count();
                    self.set_tab(n);
                }
            }
            KeyCode::BackTab => {
                if self.queue_view == QUEUE_VIEW_POWER {
                    self.power_left_tab_prev();
                } else {
                    let n = self.tab_count();
                    self.set_tab((self.tab_idx + n - 1) % n);
                }
            }

            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_queue_item_up();
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_queue_item_down();
            }
            KeyCode::Up if self.displayed_queue().queue_cursor > 0 => {
                self.last_nav_at = Instant::now();
                self.displayed_queue_mut().queue_cursor -= 1;
            }
            KeyCode::Down
                if self.displayed_queue().queue_cursor + 1 < self.displayed_queue().items.len() =>
            {
                self.last_nav_at = Instant::now();
                self.displayed_queue_mut().queue_cursor += 1;
            }
            KeyCode::PageUp => {
                let p = self.queue_page_size();
                let queue = self.displayed_queue_mut();
                queue.queue_cursor = queue.queue_cursor.saturating_sub(p);
            }
            KeyCode::PageDown => {
                let p = self.queue_page_size();
                let queue = self.displayed_queue_mut();
                let n = queue.items.len();
                queue.queue_cursor = (queue.queue_cursor + p).min(n.saturating_sub(1));
            }
            KeyCode::Home => {
                self.displayed_queue_mut().queue_cursor = 0;
            }
            KeyCode::End => {
                let n = self.displayed_queue().items.len();
                if n > 0 {
                    self.displayed_queue_mut().queue_cursor = n - 1;
                }
            }
            KeyCode::Enter => {
                let scope = self.visible_queue_scope();
                let queue = self.displayed_queue();
                let t = queue.queue_cursor;
                let n = queue.items.len();
                if t < n {
                    if let Some(ref conn_id) = self.connected_session_id.clone() {
                        let item = queue.items[t].clone();
                        let id = conn_id.clone();
                        let item_ids: Vec<String> =
                            queue.items.iter().map(|i| i.id.clone()).collect();
                        let start_ticks = item.playback_position_ticks;
                        let label = item.playback_label();
                        self.flash_status(format!("Playing on remote: {label}"));
                        self.do_session_command(move |c| {
                            c.session_play_items(&id, &item_ids, t, start_ticks)
                        });
                    } else {
                        let st = self.player.status.lock().unwrap();
                        let active = st.active;
                        let current_idx = st.current_idx;
                        drop(st);
                        if active && self.queue_scope_is_playback(scope) {
                            let is_audio =
                                queue.items.get(t).map(|i| i.is_audio()).unwrap_or(false);
                            if t == current_idx && is_audio {
                                self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
                            } else if t != current_idx {
                                self.player.send_command(PlayerCommand::JumpTo(t));
                            }
                        } else if !queue.items.is_empty() {
                            let items = queue.items.clone();
                            let c = Arc::new(self.client.lock().unwrap().clone());
                            self.replace_playback_queue(items.clone(), t);
                            self.player.play_queue(
                                items,
                                t,
                                self.queue_source.clone(),
                                c,
                                self.ui_volume,
                            );
                        }
                    }
                }
            }
            KeyCode::Delete => {
                let queue = self.displayed_queue();
                let t = queue.queue_cursor;
                if t < queue.items.len() {
                    self.remove_from_queue(t);
                }
            }
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let scope = self.visible_queue_scope();
                if scope == QueueScope::Remote {
                    self.flash_status_high("Undo is not supported for remote queue edits".into());
                    return false;
                }
                self.undo_last_queue_edit(scope);
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() {
                    self.set_tab(idx);
                }
            }
            KeyCode::Char('i') => {
                let queue = self.displayed_queue();
                let cursor = queue.queue_cursor;
                if let Some(item) = queue.items.get(cursor) {
                    let item_id = item.id.clone();
                    let item_type = item.item_type.clone();
                    let libs: Vec<(usize, String, String)> = self
                        .libs
                        .iter()
                        .enumerate()
                        .map(|(i, lib)| {
                            (
                                i,
                                lib.library.id.clone(),
                                lib.library.collection_type.clone(),
                            )
                        })
                        .collect();
                    self.spawn_navigate_to_item(item_id, item_type, libs);
                }
            }
            KeyCode::Char('.') => {
                self.open_context_menu();
            }
            KeyCode::Char('/') => {
                self.home_search = Some(HomeSearch {
                    query: String::new(),
                    last_query: String::new(),
                    results: Vec::new(),
                    cursor: 0,
                    loading: false,
                    scroll: 0,
                    type_filter: 0,
                    input_focused: true,
                });
                return false;
            }
            KeyCode::Char('v') => {
                self.queue_view = (self.queue_view + 1) % QUEUE_VIEW_COUNT;
                if self.queue_view == QUEUE_VIEW_POWER {
                    self.power_focus = PowerFocus::Left;
                }
                if !self.card_image_states.is_empty() {
                    self.force_clear = true;
                }
            }
            KeyCode::Char('g') if self.tab_idx == 1 && self.queue_view != QUEUE_VIEW_POWER => {
                self.queue_group = !self.queue_group;
            }
            KeyCode::Char('p') => {
                let (active, current_idx) = {
                    let s = self.player.status.lock().unwrap();
                    (s.active, s.current_idx)
                };
                if active {
                    self.playback_queue_mut().queue_cursor = current_idx;
                    if self.player.is_remote() {
                        self.set_queue_scope(QueueScope::Remote);
                    }
                } else {
                    self.flash_status_high("Nothing is playing".into());
                }
            }
            KeyCode::Char('s')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                if !self.player_tab.items.is_empty() {
                    self.save_playlist_dialog = Some(SavePlaylistDialog {
                        input: self.queue_playlist_name().to_string(),
                        stage: SavePlaylistStage::EnterName,
                    });
                }
            }
            KeyCode::Left | KeyCode::Up
                if self.queue_view == QUEUE_VIEW_POWER
                    && key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.power_left_tab_prev();
            }
            KeyCode::Right | KeyCode::Down
                if self.queue_view == QUEUE_VIEW_POWER
                    && key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.power_left_tab_next();
            }
            _ => {}
        }
        false
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
            if budget < tab_w + right_w && end > start {
                break;
            }
            budget = budget.saturating_sub(tab_w);
            end += 1;
        }
        (start, end)
    }

    pub(super) fn ensure_tab_visible(&mut self) {
        let n = self.tab_count();
        if n == 0 {
            return;
        }
        if self.tab_idx < self.tab_scroll {
            self.tab_scroll = self.tab_idx;
            return;
        }
        let tab_w = self
            .terminal_width
            .saturating_sub(super::TABBAR_LEFT_RESERVE + super::TABBAR_RIGHT_RESERVE);
        loop {
            let (_, end) = self.visible_tab_range(tab_w);
            if self.tab_idx < end {
                break;
            }
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
        w
    }

    fn tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout.tabs_area;
        if col < area.x || col >= area.x + area.width {
            return None;
        }
        let rel = col - area.x;
        let (vis_start, vis_end) = self.visible_tab_range(area.width);
        let has_left = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let left_w: u16 = if has_left { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left && rel < left_w {
            return Some(usize::MAX - 1);
        }
        if has_right && rel >= area.width - right_w {
            return Some(usize::MAX);
        }
        let rel = rel - left_w;
        let widths = self.tab_title_widths();
        let pad = 1u16;
        let mut x = 0u16;
        for (i, &w) in widths
            .iter()
            .enumerate()
            .skip(vis_start)
            .take(vis_end - vis_start)
        {
            let end = x + pad + w + pad;
            if rel < end {
                return Some(i);
            }
            x = end;
        }
        None
    }

    /// Map a column click to a power-view left-panel tab index (0=Home, 1+=library).
    fn power_tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout.tabs_area;
        if col < area.x || col >= area.x + area.width {
            return None;
        }
        let rel = col - area.x;
        let n = self.power_left_tab_count();
        let pad = 4u16; // rendered as "  NAME  " (2 leading + 2 trailing spaces)
        let mut x = 0u16;
        for i in 0..n {
            let name_w = if i == 0 {
                "Home".len() as u16
            } else {
                self.libs[i - 1].library.name.chars().count() as u16
            };
            let w = name_w + pad;
            if rel < x + w {
                return Some(i);
            }
            x += w;
        }
        None
    }

    pub(super) fn open_context_menu(&mut self) {
        let mut entries: Vec<super::ContextMenuEntry> = vec![];

        let cw_focused = self.queue_view == QUEUE_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab == 0;
        let power_lib_idx = self.context_menu_power_lib_idx();
        let context_lib_idx = self.context_menu_lib_idx();
        let in_podcast = power_lib_idx.is_some_and(|idx| self.is_podcast_library(idx))
            || self.is_in_podcast_library();
        let podcast_bulk_ids = context_lib_idx.and_then(|lib_idx| {
            if in_podcast && self.is_feed_home_video_group_view(lib_idx) {
                Some((
                    self.podcast_mark_all_ids(lib_idx),
                    self.podcast_mark_all_unplayed_ids(lib_idx),
                ))
            } else {
                None
            }
        });

        let current_item = if cw_focused {
            self.home
                .continue_items
                .get(self.home.continue_cursor)
                .cloned()
        } else if let Some(lib_idx) = power_lib_idx {
            let saved = self.tab_idx;
            self.tab_idx = self.lib_tab_offset() + lib_idx;
            let item = self.current_lib_item();
            self.tab_idx = saved;
            item
        } else if self.home_search.is_some() || self.tab_idx == 0 {
            self.current_home_item()
        } else if self.tab_idx == 1 {
            let queue = self.displayed_queue();
            queue.items.get(queue.queue_cursor).cloned()
        } else if self.tab_idx > 1 {
            self.current_lib_item()
        } else {
            None
        };

        if let Some(ref item) = current_item {
            if item.is_folder {
                Self::push_context_action(
                    &mut entries,
                    "Play All",
                    ContextAction::PlayFolder(item.id.clone()),
                );
                Self::push_context_action(
                    &mut entries,
                    "Shuffle",
                    ContextAction::ShuffleFolder(item.id.clone()),
                );
                Self::push_context_action(
                    &mut entries,
                    "Add to Queue",
                    ContextAction::EnqueueFolder(Box::new(item.clone())),
                );
                let (played_label, unplayed_label) = if in_podcast {
                    ("Mark Played", "Mark Unplayed")
                } else {
                    ("Mark Watched", "Mark Unwatched")
                };
                if self.context_menu_play_state(item) {
                    Self::push_context_action(
                        &mut entries,
                        unplayed_label,
                        ContextAction::MarkUnplayed(item.id.clone()),
                    );
                } else {
                    Self::push_context_action(
                        &mut entries,
                        played_label,
                        ContextAction::MarkPlayed(item.id.clone()),
                    );
                }
                if self.home_search.is_some() {
                    Self::push_context_action(
                        &mut entries,
                        "Go to Library",
                        ContextAction::GoToLibrary(item.id.clone(), item.item_type.clone()),
                    );
                }
            } else {
                Self::push_context_action(&mut entries, "Play", ContextAction::Play);
                if cw_focused
                    || power_lib_idx.is_some()
                    || self.home_search.is_some()
                    || self.tab_idx != 1
                {
                    Self::push_context_action(&mut entries, "Add to Queue", ContextAction::Enqueue);
                }
                // Audio items (music tracks) don't get mark-played, but podcast
                // episodes (Audio inside a Channel library) do.
                let is_music_audio =
                    (item.media_type == "Audio" || item.item_type == "Audio") && !in_podcast;
                if !is_music_audio {
                    let (played_label, unplayed_label) = if in_podcast {
                        ("Mark Played", "Mark Unplayed")
                    } else {
                        ("Mark Watched", "Mark Unwatched")
                    };
                    if self.context_menu_play_state(item) {
                        Self::push_context_action(
                            &mut entries,
                            unplayed_label,
                            ContextAction::MarkUnplayed(item.id.clone()),
                        );
                    } else {
                        Self::push_context_action(
                            &mut entries,
                            played_label,
                            ContextAction::MarkPlayed(item.id.clone()),
                        );
                    }
                }
                if cw_focused
                    || (self.home_search.is_none() && self.tab_idx == 0 && self.home.section == 0)
                {
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Continue Watching",
                        ContextAction::RemoveFromContinueWatching,
                    );
                }
                if !cw_focused && self.home_search.is_none() && self.tab_idx == 1 {
                    let pos = self.displayed_queue().queue_cursor;
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Queue",
                        ContextAction::RemoveFromQueue(pos),
                    );
                }
                if self.home_search.is_some() || self.tab_idx == 1 {
                    Self::push_context_action(
                        &mut entries,
                        "Go to Library",
                        ContextAction::GoToLibrary(item.id.clone(), item.item_type.clone()),
                    );
                }
            }
        }

        if let Some((played_ids, unplayed_ids)) = podcast_bulk_ids {
            if !played_ids.is_empty() || !unplayed_ids.is_empty() {
                Self::push_context_separator(&mut entries);
                Self::push_context_action(
                    &mut entries,
                    "Mark All Played",
                    ContextAction::MarkItemsPlayed(played_ids),
                );
                Self::push_context_action(
                    &mut entries,
                    "Mark All Unplayed",
                    ContextAction::MarkItemsUnplayed(unplayed_ids),
                );
            }
        }

        if entries.iter().all(|entry| entry.action.is_none()) {
            return;
        }

        let (x, y) = self.context_menu_spawn_point();
        self.context_menu = Some(ContextMenu {
            x,
            y,
            cursor: ContextMenu::first_selectable(&entries),
            entries,
        });
    }

    pub(super) fn open_context_menu_at(&mut self, x: u16, y: u16) {
        self.open_context_menu();
        if let Some(ref mut menu) = self.context_menu {
            menu.x = x;
            menu.y = y;
        }
    }

    fn context_menu_spawn_point(&self) -> (u16, u16) {
        if self.tab_idx == 0 && self.home_card_view {
            let center = self.layout.home.carousel_slots[1].1;
            return (center.x + center.width / 2, center.y + center.height / 2);
        }
        if self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER {
            match self.power_focus {
                PowerFocus::Left => {
                    let area = self.layout.power.left_area;
                    if area.width > 0 {
                        let y = self.layout.power.cursor_screen_y.unwrap_or(area.y);
                        let x = area.x + 2;
                        // Avoid inline image overlap (detail/episode poster).
                        if let Some(img) = self.layout.power.inline_image_rect {
                            if y >= img.y && y < img.y + img.height {
                                let below = img.y + img.height;
                                if below < area.y + area.height {
                                    return (x, below);
                                }
                            }
                        }
                        return (x, y);
                    }
                }
                PowerFocus::Queue => {
                    let area = self.layout.power.queue_area;
                    if area.width > 0 {
                        let y = self.layout.power.queue_cursor_screen_y.unwrap_or(area.y);
                        return (area.x + 2, y);
                    }
                }
            }
        }
        if self.tab_idx == 0 {
            let sec = self.home.section;
            if let Some(area) = self.layout.home.section_areas.get(sec) {
                let scroll = self.layout.home.home_scrolls.get(sec).copied().unwrap_or(0);
                let cursor = match sec {
                    0 => self.home.continue_cursor,
                    n => self
                        .home
                        .latest
                        .get(n - 1)
                        .map(|(_, _, _, c)| *c)
                        .unwrap_or(0),
                };
                let row = cursor.saturating_sub(scroll) as u16;
                return (self.terminal_width / 2, area.y + 1 + row);
            }
        } else if self.tab_idx > 1 {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let lib = &self.libs[lib_idx];
            let cursor = lib
                .nav_stack
                .last()
                .map(|lvl| {
                    lib.search
                        .as_ref()
                        .and_then(|s| s.results.get(s.cursor).copied())
                        .unwrap_or(lvl.cursor)
                })
                .unwrap_or(0);
            let scroll = self
                .layout
                .library
                .lib_scroll
                .get(lib_idx)
                .copied()
                .unwrap_or(0);
            let row = cursor.saturating_sub(scroll) as u16;
            let tbl = self
                .layout
                .library
                .lib_table_area
                .get(lib_idx)
                .copied()
                .unwrap_or_default();
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

    pub(super) fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        let v = serde_json::json!({
            "ui_volume": self.ui_volume,
            "mute_on": self.mute_on,
            "pre_mute_volume": self.pre_mute_volume,
            "tab_idx": self.tab_idx,
            "playlist_view": self.queue_view,
            "power_left_tab": self.power_left_tab,
            "power_left_width": self.power_left_width,
        });
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }

    fn seek_to_col(&mut self, col: u16) {
        let bar = self.layout.playback.seekbar_area;
        if bar.width == 0 {
            return;
        }
        let fraction = (col.saturating_sub(bar.x)) as f64 / bar.width as f64;
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let runtime_s = self
                .connected_session_state
                .as_ref()
                .map(|s| s.runtime_s)
                .unwrap_or(0);
            if runtime_s == 0 {
                return;
            }
            let ticks = (fraction * (runtime_s * mbv_core::api::TICKS_PER_SECOND) as f64) as i64;
            let id = conn_id.clone();
            self.remote_pos_s = (fraction * runtime_s as f64) as i64;
            self.remote_pos_at = Instant::now();
            self.remote_seek_pending_until = Instant::now() + Duration::from_secs(4);
            self.do_session_command(move |c| c.session_seek(&id, ticks));
            return;
        }
        let runtime_ticks = self.player.status.lock().unwrap().runtime_ticks;
        if runtime_ticks == 0 {
            return;
        }
        let target_secs = (fraction * runtime_ticks as f64) / TICKS_PER_SECOND as f64;
        self.player
            .send_command(PlayerCommand::SeekAbsolute(target_secs));
    }

    fn click_set_cursor(&mut self, col: u16, row: u16) -> bool {
        if self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER {
            if self.has_direct_remote_queue() {
                if self
                    .layout
                    .power
                    .queue_scope_local_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Local);
                    return true;
                }
                if self
                    .layout
                    .power
                    .queue_scope_remote_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return true;
                }
            }
            // Click in queue area: focus queue and move cursor.
            let qa = self.layout.power.queue_area;
            if qa.contains((col, row).into()) {
                if !matches!(self.power_focus, PowerFocus::Queue) {
                    self.last_card_height = 0;
                }
                self.power_focus = PowerFocus::Queue;
                let content_y = (row - qa.y) as usize;
                if let Some(&Some(item_idx)) = self.layout.power.queue_row_map.get(content_y) {
                    self.displayed_queue_mut().queue_cursor = item_idx;
                }
                return true;
            }
            // Click in the left panel: focus it and set its cursor.
            let la = self.layout.power.left_area;
            if la.contains((col, row).into()) {
                if !matches!(self.power_focus, PowerFocus::Left) {
                    self.last_card_height = 0;
                }
                self.power_focus = PowerFocus::Left;
                if self.power_left_tab == 0 {
                    // Home tab: rectangle hit-test the two-column card grid.
                    let pos = (col, row).into();
                    if let Some((_, flat_idx)) = self
                        .layout
                        .power
                        .home
                        .hitmap
                        .iter()
                        .find(|(rect, _)| rect.contains(pos))
                    {
                        self.home.power_home_cursor = *flat_idx;
                    }
                } else {
                    let lib_idx = self.power_left_tab - 1;
                    if self.is_music_group_view(lib_idx)
                        || self.is_feed_home_video_group_view(lib_idx)
                    {
                        for (rect, target) in self.layout.power.selector_tabs.clone() {
                            if rect.contains((col, row).into()) {
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else {
                                    self.select_feed_folder_group(lib_idx, target);
                                }
                                return true;
                            }
                        }
                    }
                    let click_y = (row - la.y) as usize;
                    // Read the row map before taking a mutable borrow on libs (borrow checker).
                    let use_row_map = !self.layout.power.left_row_map.is_empty();
                    let row_map_item = if use_row_map {
                        self.layout.power.left_row_map.get(click_y).copied()
                    } else {
                        None
                    };
                    let is_feed_group = self.is_feed_home_video_group_view(lib_idx);
                    let lib = &mut self.libs[lib_idx];
                    if let Some(s) = &mut lib.search {
                        let visible = la.height as usize;
                        let offset = if s.cursor >= visible {
                            s.cursor - visible + 1
                        } else {
                            0
                        };
                        let clicked = offset + click_y;
                        if clicked < s.results.len() {
                            s.cursor = clicked;
                        }
                    } else if is_feed_group {
                        let visible = la.height as usize;
                        if let Some(state) = lib.feed_home_video.as_mut() {
                            let items_len = state.selected_len();
                            if use_row_map {
                                if let Some(Some(item_idx)) = row_map_item {
                                    if item_idx < items_len {
                                        state.video_cursor = item_idx;
                                    }
                                }
                            } else {
                                let offset = if state.video_cursor >= visible {
                                    state.video_cursor - visible + 1
                                } else {
                                    0
                                };
                                let clicked = offset + click_y;
                                if clicked < items_len {
                                    state.video_cursor = clicked;
                                }
                            }
                        }
                    } else if let Some(lvl) = lib.nav_stack.last_mut() {
                        if use_row_map {
                            // Letter-grouped mode: row map gives item index (None = header row).
                            if let Some(Some(item_idx)) = row_map_item {
                                if item_idx < lvl.items.len() {
                                    lvl.cursor = item_idx;
                                }
                            }
                        } else {
                            let visible = la.height as usize;
                            let offset = if lvl.cursor >= visible {
                                lvl.cursor - visible + 1
                            } else {
                                0
                            };
                            let clicked = offset + click_y;
                            if clicked < lvl.items.len() {
                                lvl.cursor = clicked;
                            }
                        }
                    }
                }
                return true;
            }
        } else if self.tab_idx == 1 {
            if self.has_direct_remote_queue() {
                if self
                    .layout
                    .queue
                    .scope_local_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Local);
                    return true;
                }
                if self
                    .layout
                    .queue
                    .scope_remote_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return true;
                }
            }
            let inner = self.layout.queue.inner;
            if inner.contains((col, row).into()) {
                let click_y = (row - inner.y) as usize;
                if let Some(&Some(idx)) = self.layout.queue.row_map.get(click_y) {
                    self.displayed_queue_mut().queue_cursor = idx;
                    return true;
                }
            }
        } else if self.tab_idx == 0 {
            if self.layout.home.home_rect.contains((col, row).into()) {
                let n_secs = self.layout.home.section_areas.len();
                let mut found_sec: Option<(usize, Rect)> = None;
                for sec in 0..n_secs {
                    let sect_area = self.layout.home.section_areas[sec];
                    if sect_area.contains((col, row).into()) {
                        found_sec = Some((sec, sect_area));
                        break;
                    }
                }
                if let Some((sec, sect_area)) = found_sec {
                    self.home.section = sec;
                    let inner = Block::default()
                        .borders(Borders::TOP | Borders::BOTTOM)
                        .border_type(BorderType::Rounded)
                        .inner(sect_area);
                    if inner.contains((col, row).into()) {
                        let row_idx = (row - inner.y) as usize;
                        let scroll_start =
                            self.layout.home.home_scrolls.get(sec).copied().unwrap_or(0);
                        let inner_h = inner.height as usize;
                        let inner_w = inner.width.max(1) as usize;
                        let item_texts: Vec<String> = {
                            let items_slice: &[MediaItem] = if sec == 0 {
                                &self.home.continue_items
                            } else {
                                self.home
                                    .latest
                                    .get(sec - 1)
                                    .map(|c| c.2.as_slice())
                                    .unwrap_or(&[])
                            };
                            items_slice
                                .iter()
                                .skip(scroll_start)
                                .map(|item| {
                                    let (t, _) = item_text_and_style(item, false);
                                    t
                                })
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
                            if line_acc >= inner_h {
                                break;
                            }
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
        } else if self.tab_idx > 1 {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let lib = &self.layout.library;
            let tbl = lib.lib_table_area.get(lib_idx).copied().unwrap_or_default();
            if tbl.contains((col, row).into()) {
                let click_y = row - tbl.y;
                let scroll = lib.lib_scroll.get(lib_idx).copied().unwrap_or(0);
                let display_pos = {
                    let mut y = 0u16;
                    let mut found = scroll;
                    for (vi, &h) in lib
                        .lib_row_heights
                        .get(lib_idx)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[])
                        .iter()
                        .enumerate()
                    {
                        if click_y < y + h {
                            found = scroll + vi;
                            break;
                        }
                        y += h;
                    }
                    found
                };
                let lib = &mut self.libs[lib_idx];
                let hit = if let Some(s) = &mut lib.search {
                    if display_pos < s.results.len() {
                        s.cursor = display_pos;
                        true
                    } else {
                        false
                    }
                } else if let Some(lvl) = lib.nav_stack.last_mut() {
                    if display_pos < lvl.items.len() {
                        lvl.cursor = display_pos;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                return hit;
            }
        }
        false
    }

    /// Handle a mouse event when a panel overlay (help/settings/sessions/playlists) is open.
    /// Returns true if the event was consumed.
    fn handle_mouse_panels(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
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
        let pw = panel_w.min(self.terminal_width);
        let inside_panel = col < pw;
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
            let content_top: u16 = 1;
            let content_bottom = self.terminal_height.saturating_sub(2);
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
            let content_top: u16 = 1;
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
            let content_top: u16 = 1;
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
                            let pw2 = PLAYLISTS_PANEL_W.min(self.terminal_width) as usize;
                            let h = if i.display_name().len() <= pw2.saturating_sub(6) {
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

    pub(super) fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};
        let col = mouse.column;
        let row = mouse.row;
        // Always track mouse position so hover rendering is up to date.
        self.mouse_col = col;
        self.mouse_row = row;
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            let now = Instant::now();
            if now.duration_since(self.last_scroll_at) < Duration::from_millis(30) {
                return;
            }
            self.last_scroll_at = now;
        }

        if self.handle_mouse_panels(mouse) {
            return;
        }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout.tabs_area.contains((col, row).into())
        {
            if self.queue_view == QUEUE_VIEW_POWER {
                // In power view, tab clicks change the left-panel selection, not the app tab.
                if let Some(idx) = self.power_tab_idx_at(col) {
                    self.power_left_tab = idx;
                    if idx > 0 {
                        self.ensure_lib_loaded_for(idx - 1);
                    }
                    self.save_prefs();
                }
            } else if let Some(idx) = self.tab_idx_at(col) {
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
            && self.layout.settings_area.contains((col, row).into())
        {
            self.show_settings = !self.show_settings;
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollDown) {
                    1
                } else {
                    -1
                };
                if self.layout.tabbar_vol_area.contains((col, row).into()) {
                    self.adjust_volume(-delta * 5);
                    return;
                }
                if self.tab_idx == 0 {
                    let sb = self.layout.home.home_scrollbar;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        let active = self.player.status.lock().unwrap().active;
                        let chrome: u16 = if active { 6 } else { 3 };
                        let panel_h = self.terminal_height.saturating_sub(chrome);
                        let n_sections = 1 + self.home.latest.len();
                        let visible = ((panel_h / HOME_MIN_SECTION_H) as usize)
                            .max(1)
                            .min(n_sections);
                        let max_offset = n_sections.saturating_sub(visible);
                        self.home_panel_section_offset =
                            (self.home_panel_section_offset as i64 + delta)
                                .clamp(0, max_offset as i64) as usize;
                    } else if self.layout.home.home_rect.contains((col, row).into()) {
                        if self.home_card_view {
                            let n = 1 + self.home.latest.len();
                            self.home.section =
                                (self.home.section as i64 + delta).clamp(0, n as i64 - 1) as usize;
                            self.ensure_home_section_visible();
                            if !self.card_image_states.is_empty() {
                                self.force_clear = true;
                            }
                        } else {
                            self.move_home_cursor(delta);
                        }
                    }
                } else if self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER {
                    // Scroll in whichever power-view panel the mouse is over.
                    let queue_area = self.layout.power.queue_area;
                    let left_area = self.layout.power.left_area;
                    if queue_area.contains((col, row).into()) {
                        let n = self.displayed_queue().items.len();
                        if n > 0 {
                            let delta = delta * 3;
                            let queue = self.displayed_queue_mut();
                            queue.queue_cursor =
                                (queue.queue_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                        }
                    } else if left_area.contains((col, row).into()) {
                        if self.power_left_tab == 0 {
                            self.power_cw_move_cursor(delta);
                        } else {
                            let lib_idx = self.power_left_tab - 1;
                            let saved = self.tab_idx;
                            self.tab_idx = self.lib_tab_offset() + lib_idx;
                            self.move_lib_cursor(delta);
                            self.tab_idx = saved;
                        }
                    }
                } else if self.tab_idx == 1 {
                    let n = self.displayed_queue().items.len();
                    if n > 0 {
                        let queue = self.displayed_queue_mut();
                        queue.queue_cursor =
                            (queue.queue_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    }
                } else {
                    self.move_lib_cursor(delta);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.context_menu.is_some() {
                    if let Some(rect) = self.layout.context_menu_rect {
                        if rect.contains((col, row).into()) {
                            let inner_y = rect.y + 1;
                            if row >= inner_y
                                && (row - inner_y)
                                    < self.context_menu.as_ref().unwrap().entries.len() as u16
                            {
                                let idx = (row - inner_y) as usize;
                                let action = self
                                    .context_menu
                                    .as_ref()
                                    .unwrap()
                                    .entries
                                    .get(idx)
                                    .and_then(|entry| entry.action.clone());
                                if action.is_some() {
                                    self.context_menu = None;
                                    self.layout.context_menu_rect = None;
                                    self.force_clear = true;
                                    self.execute_context_action(action);
                                }
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

                if let Some(r) = self.layout.home.carousel_left_arrow {
                    if r.contains((col, row).into()) {
                        if self.tab_idx == 0 {
                            self.move_home_cursor(-1);
                        } else {
                            if self.displayed_queue().queue_cursor > 0 {
                                self.displayed_queue_mut().queue_cursor -= 1;
                            }
                        }
                        return;
                    }
                }
                if let Some(r) = self.layout.home.carousel_right_arrow {
                    if r.contains((col, row).into()) {
                        if self.tab_idx == 0 {
                            self.move_home_cursor(1);
                        } else {
                            let n = self.displayed_queue().items.len();
                            if self.displayed_queue().queue_cursor + 1 < n {
                                self.displayed_queue_mut().queue_cursor += 1;
                            }
                        }
                        return;
                    }
                }
                if self.tab_idx == 0 && self.home_card_view {
                    let strips = self.layout.home.home_card_strips.clone();
                    for (sec_idx, strip_rect) in &strips {
                        if strip_rect.contains((col, row).into()) && *sec_idx != self.home.section {
                            self.home.section = *sec_idx;
                            if !self.card_image_states.is_empty() {
                                self.force_clear = true;
                            }
                            return;
                        }
                    }
                }
                if let Some(r) = self.layout.home.carousel_up_arrow {
                    if r.contains((col, row).into()) {
                        if self.home.section > 0 {
                            self.home.section -= 1;
                            self.ensure_home_section_visible();
                        }
                        return;
                    }
                }
                if let Some(r) = self.layout.home.carousel_down_arrow {
                    if r.contains((col, row).into()) {
                        let n_sections = 1 + self.home.latest.len();
                        if self.home.section + 1 < n_sections {
                            self.home.section += 1;
                            self.ensure_home_section_visible();
                        }
                        return;
                    }
                }

                let is_double = now.duration_since(self.last_click_time)
                    < Duration::from_millis(400)
                    && self.last_click_pos == (col, row);
                self.last_click_time = now;
                self.last_click_pos = (col, row);

                if self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER {
                    for (rect, target) in self.layout.power.selector_tabs.clone() {
                        if rect.contains((col, row).into()) {
                            if self.power_left_tab > 0 {
                                let lib_idx = self.power_left_tab - 1;
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else if self.is_feed_home_video_group_view(lib_idx) {
                                    self.select_feed_folder_group(lib_idx, target);
                                }
                            }
                            return;
                        }
                    }
                }

                if is_double {
                    if self
                        .layout
                        .playback
                        .seekbar_area
                        .contains((col, row).into())
                    {
                        self.seek_to_col(col);
                        return;
                    }
                    if self.tab_idx == 0 {
                        if self.layout.home.home_rect.contains((col, row).into()) {
                            self.select_home();
                        }
                    } else if self.tab_idx == 1 {
                        let queue = self.displayed_queue();
                        let t = queue.queue_cursor;
                        if t < queue.items.len()
                            && self.layout.queue.inner.contains((col, row).into())
                        {
                            let scope = self.visible_queue_scope();
                            if let Some(ref conn_id) = self.connected_session_id.clone() {
                                let item = queue.items[t].clone();
                                let id = conn_id.clone();
                                let item_ids: Vec<String> =
                                    queue.items.iter().map(|i| i.id.clone()).collect();
                                let start_ticks = item.playback_position_ticks;
                                let label = item.playback_label();
                                self.flash_status(format!("Playing on remote: {label}"));
                                self.do_session_command(move |c| {
                                    c.session_play_items(&id, &item_ids, t, start_ticks)
                                });
                            } else {
                                let st = self.player.status.lock().unwrap();
                                let active = st.active;
                                let current_idx = st.current_idx;
                                drop(st);
                                if active && self.queue_scope_is_playback(scope) {
                                    let is_audio =
                                        queue.items.get(t).map(|i| i.is_audio()).unwrap_or(false);
                                    if t == current_idx && is_audio {
                                        self.player.send_command(PlayerCommand::SeekAbsolute(0.0));
                                    } else if t != current_idx {
                                        self.player.send_command(PlayerCommand::JumpTo(t));
                                    }
                                } else {
                                    let items = queue.items.clone();
                                    let c = Arc::new(self.client.lock().unwrap().clone());
                                    self.replace_playback_queue(items.clone(), t);
                                    self.player.play_queue(
                                        items,
                                        t,
                                        self.queue_source.clone(),
                                        c,
                                        self.ui_volume,
                                    );
                                }
                            }
                        }
                    } else if self
                        .current_lib_item()
                        .map(|i| !i.is_folder)
                        .unwrap_or(false)
                    {
                        self.select();
                    }
                    return;
                }

                if self.layout.playback.ind_rc.contains((col, row).into()) {
                    self.show_sessions = !self.show_sessions;
                    if self.show_sessions {
                        self.spawn_sessions_load();
                    }
                    return;
                }
                if self.layout.playback.ind_mu.contains((col, row).into()) {
                    // The "m" pill renders `self.mute_on` (see
                    // render_control_pill) and the `m` key flips it via
                    // `Command::ToggleMute` -- dispatch the same action here
                    // rather than calling `toggle_mute()` (the *other*,
                    // ui_volume-based mechanism used by the `a` key; see
                    // `Command::ToggleMute`'s doc comment in action.rs).
                    // Calling the wrong one here predates #88, but #88 makes
                    // it worse: `toggle_mute()` now falls back to
                    // `cycle_audio()` for a connected remote session, so
                    // clicking this pill while attached to a session used to
                    // be a harmless no-op and would otherwise start silently
                    // cycling that session's audio track instead of muting
                    // anything.
                    self.dispatch(super::action::Command::ToggleMute);
                    return;
                }
                if self
                    .layout
                    .playback
                    .play_pause_area
                    .contains((col, row).into())
                {
                    self.dispatch(super::action::Command::TogglePlayPause);
                    return;
                }
                if self.layout.playback.next_area.contains((col, row).into()) {
                    if self.transport_prev_next_available().1 {
                        self.dispatch(super::action::Command::NextTrack);
                    }
                    return;
                }
                if self.tab_idx == 0 {
                    let sb = self.layout.home.home_scrollbar;
                    if sb.width > 0 && sb.contains((col, row).into()) {
                        self.home_scrollbar_seek(row);
                        return;
                    }
                }

                // Power-view header breadcrumb clicks.
                if self.tab_idx == 1
                    && self.queue_view == QUEUE_VIEW_POWER
                    && self.power_left_tab > 0
                {
                    let crumbs = self.layout.power.breadcrumbs.clone();
                    let lib_idx = self.power_left_tab - 1;
                    for (x_start, x_end, crumb_row, target_depth) in crumbs {
                        if row == crumb_row && col >= x_start && col < x_end {
                            self.libs[lib_idx].nav_stack.truncate(target_depth);
                            self.libs[lib_idx].search = None;
                            return;
                        }
                    }
                }

                if self.tab_idx > 1 {
                    let crumbs = self.layout.library.breadcrumbs.clone();
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
                let hit = self.click_set_cursor(col, row);
                if hit
                    && self.tab_idx > 1
                    && self
                        .current_lib_item()
                        .map(|i| i.is_folder)
                        .unwrap_or(false)
                {
                    self.select();
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.tab_idx == 0 && self.home_card_view {
                    let slots = self.layout.home.carousel_slots;
                    for (maybe_item_idx, card_rect) in slots.iter() {
                        if card_rect.contains((col, row).into()) {
                            if let Some(item_idx) = maybe_item_idx {
                                let sec = self.home.section;
                                self.set_home_cursor(sec, *item_idx);
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
                    let sb = self.layout.home.home_scrollbar;
                    sb.width > 0 && sb.contains((col, row).into())
                } =>
            {
                self.home_scrollbar_seek(row);
            }
            MouseEventKind::Drag(MouseButton::Left)
                if self
                    .layout
                    .playback
                    .seekbar_area
                    .contains((col, row).into())
                    && self.last_drag_seek.elapsed() >= Duration::from_millis(150) =>
            {
                self.last_drag_seek = Instant::now();
                self.seek_to_col(col);
            }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Right) => {
                if let (Some(ref mut menu), Some(rect)) =
                    (&mut self.context_menu, self.layout.context_menu_rect)
                {
                    let inner_y = rect.y + 1;
                    if rect.contains((col, row).into()) && row >= inner_y {
                        let idx = (row - inner_y) as usize;
                        if idx < menu.entries.len() && menu.entries[idx].action.is_some() {
                            menu.cursor = idx;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod playback_header_mouse_tests {
    //! Mouse hit-testing for the one-row playback header's transport
    //! controls (issue #112): the play/pause glyph and the next glyph,
    //! both of which must reuse the existing playback actions. The next
    //! control must not fire when the queue is already at that boundary.
    use super::*;
    use crate::app::tests::make_app_stub;
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    fn left_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn click_play_pause_area_sends_toggle_pause() {
        let mut app = make_app_stub();
        app.layout.playback.play_pause_area = Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        };
        let rx = app.player.spy_on_commands();
        app.handle_mouse(left_down(0, 0));
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    }

    #[test]
    fn click_next_area_jumps_forward_when_not_last_item() {
        let mut app = make_app_stub();
        app.layout.playback.next_area = Rect {
            x: 5,
            y: 0,
            width: 2,
            height: 1,
        };
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
        }
        let rx = app.player.spy_on_commands();
        app.handle_mouse(left_down(5, 0));
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
    }

    #[test]
    fn click_second_cell_of_wide_next_area_also_jumps_forward() {
        let mut app = make_app_stub();
        app.layout.playback.next_area = Rect {
            x: 5,
            y: 0,
            width: 2,
            height: 1,
        };
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
        }
        let rx = app.player.spy_on_commands();
        app.handle_mouse(left_down(6, 0));
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
    }

    #[test]
    fn click_next_area_is_a_no_op_on_last_item() {
        let mut app = make_app_stub();
        app.layout.playback.next_area = Rect {
            x: 5,
            y: 0,
            width: 2,
            height: 1,
        };
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 1;
        }
        let rx = app.player.spy_on_commands();
        app.handle_mouse(left_down(5, 0));
        assert!(rx.try_recv().is_err(), "last item: next must not fire");
    }

    #[test]
    fn click_next_is_a_no_op_on_single_item_queue() {
        let mut app = make_app_stub();
        app.layout.playback.next_area = Rect {
            x: 5,
            y: 0,
            width: 2,
            height: 1,
        };
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 1;
            st.current_idx = 0;
        }
        let rx = app.player.spy_on_commands();
        app.handle_mouse(left_down(5, 0));
        assert!(
            rx.try_recv().is_err(),
            "single-item queue: next must not fire"
        );
    }
}

#[cfg(test)]
mod power_movie_detail_tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{
        BrowseLevel, LibraryTab, PowerFocus, POWER_LEFT_WIDTH_DEFAULT, QUEUE_VIEW_POWER,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_power_movie_app() -> App {
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut first = make_item("First Movie", "Movie");
        first.id = "movie-1".into();
        first.overview =
            "A compact banner overview that should not require the expanded detail mode.".into();
        first.director = "Jane Director".into();

        let mut second = make_item("Second Movie", "Movie");
        second.id = "movie-2".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![first, second],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,
        });

        app
    }

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    #[test]
    fn enter_on_power_view_movie_plays_without_opening_detail() {
        let mut app = make_power_movie_app();

        let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(!handled);
        assert!(app.libs[0].power_detail_item.is_none());
        assert_eq!(app.player_tab.items.len(), 1);
        assert_eq!(app.player_tab.items[0].id, "movie-1");
    }

    #[test]
    fn alt_m_toggles_power_movie_detail() {
        let mut app = make_power_movie_app();

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));

        assert!(!handled);
        assert_eq!(
            app.libs[0]
                .power_detail_item
                .as_ref()
                .map(|item| item.id.as_str()),
            Some("movie-1")
        );
        assert_eq!(app.libs[0].power_detail_scroll, 0);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT));

        assert!(!handled);
        assert!(app.libs[0].power_detail_item.is_none());
    }

    #[test]
    fn shift_right_resizes_power_view_without_switching_focus() {
        let mut app = make_power_movie_app();
        app.power_focus = PowerFocus::Queue;
        app.terminal_width = 100;

        let handled = app.handle_key(shift(KeyCode::Right));

        assert!(!handled);
        assert!(matches!(app.power_focus, PowerFocus::Queue));
        assert_eq!(app.power_left_width, 45);
        assert!(app.status.contains("45"), "status was {:?}", app.status);
        assert_eq!(App::load_prefs()["power_left_width"].as_u64(), Some(45));
    }

    #[test]
    fn shift_resize_is_ignored_outside_power_view() {
        let mut app = make_app_stub();

        let handled = app.handle_key(shift(KeyCode::Right));

        assert!(!handled);
        assert_eq!(app.power_left_width, POWER_LEFT_WIDTH_DEFAULT);
        assert!(app.status.is_empty(), "status was {:?}", app.status);
    }

    #[test]
    fn help_overlay_blocks_power_resize_shortcuts() {
        let mut app = make_power_movie_app();
        app.show_help = true;
        app.terminal_width = 100;

        let handled = app.handle_key(shift(KeyCode::Right));

        assert!(!handled);
        assert_eq!(app.power_left_width, POWER_LEFT_WIDTH_DEFAULT);
    }

    #[test]
    fn shift_resize_clamps_at_min_and_max_without_resaving_on_noop() {
        let mut app = make_power_movie_app();
        app.terminal_width = 80;

        app.handle_key(shift(KeyCode::Left));
        assert_eq!(app.power_left_width, POWER_LEFT_WIDTH_DEFAULT);
        assert!(
            app.status.contains("minimum"),
            "expected minimum toast, got {:?}",
            app.status
        );

        app.handle_key(shift(KeyCode::Right));
        assert_eq!(app.power_left_width, 45);
        app.handle_key(shift(KeyCode::Right));
        assert_eq!(app.power_left_width, 48);
        let saved = App::load_prefs()["power_left_width"].as_u64();
        assert_eq!(saved, Some(48));

        app.handle_key(shift(KeyCode::Right));
        assert_eq!(app.power_left_width, 48);
        assert!(
            app.status.contains("maximum"),
            "expected maximum toast, got {:?}",
            app.status
        );
        assert_eq!(App::load_prefs()["power_left_width"].as_u64(), saved);
    }

    #[test]
    fn render_normalizes_oversized_saved_power_width_and_persists_it() {
        let mut app = make_power_movie_app();
        app.power_left_width = 80;

        let backend = TestBackend::new(70, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| app.render(f)).unwrap();

        assert_eq!(app.power_left_width, 42);
        assert_eq!(App::load_prefs()["power_left_width"].as_u64(), Some(42));
    }
}
