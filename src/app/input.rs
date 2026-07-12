use super::settings::settings_total_rows;
use super::ui_util::item_text_and_style;
use super::{
    App, ContextAction, ContextMenu, LibSearch, PendingQueueAction, QueueScope, SavePlaylistDialog,
    SavePlaylistStage, HELP_PANEL_W, HOME_MIN_SECTION_H, PLAYLISTS_PANEL_W,
    POWER_LEFT_WIDTH_DEFAULT, POWER_LEFT_WIDTH_STEP, SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use super::{PowerFocus, QUEUE_VIEW_COUNT, QUEUE_VIEW_POWER};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Borders};
use std::time::{Duration, Instant};
use textwrap::wrap;

impl App {
    /// Whether a context menu is currently open. Shared by every
    /// CONTEXT_STACK layer above `context_menu` that must yield to it
    /// (`panel_toggle_h`, `home_search`, `power_lib_search`, `lib_search`,
    /// `clear_queue_prompt_c`, `power_left_width`) — see
    /// docs/adr/0002-centralized-input-handling.md phase 6 (#135).
    fn context_menu_open(&self) -> bool {
        self.context_menu.is_some()
    }

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

    pub(super) fn handle_key_power_left_width(&mut self, key: KeyEvent) -> Option<bool> {
        if self.handle_power_left_width_key(key) {
            Some(false)
        } else {
            None
        }
    }

    // Correctness note: in the pre-phase-2 source, this check ran *after*
    // home-search/power-lib-search/lib-search (source order: power-left-width,
    // home-search Alt-cycle, home-search char-capture, power-lib-search,
    // lib-search, `h`-toggle, confirms...). It must stay positioned after
    // `lib_search` in CONTEXT_STACK — not bundled with power-left-width above
    // — otherwise `h` would win over an active search box typing the literal
    // character 'h', which is a real behavior change, not just a structural
    // one. (Caught during Task 4's self-review; fixed here rather than left
    // for Task 5, since leaving it in the wrong slot even temporarily would
    // ship a regression.)
    pub(super) fn handle_key_panel_toggle(&mut self, key: KeyEvent) -> Option<bool> {
        // Behavior change (phase 6, #135): gate on an open context menu. Before
        // this fix, `h` sat above `context_menu` in CONTEXT_STACK with no guard,
        // so pressing 'h' while a context menu was open silently toggled the
        // panel instead of being swallowed by the menu (which has no 'h'
        // binding of its own). See docs/adr/0002-centralized-input-handling.md
        // phase 6 and phase-2's `home_search`, which already guards the same way.
        if key.code != KeyCode::Char('h') || self.context_menu_open() {
            return None;
        }
        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        if show_controls {
            self.panel_mode = self.panel_mode.next();
        }
        Some(false)
    }

    pub(super) fn handle_key_home_search(&mut self, key: KeyEvent) -> Option<bool> {
        if !(self.tab_idx == 0 || self.tab_idx == 1)
            || !self.search.is_open()
            || self.context_menu_open()
        {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Left | KeyCode::Right => {
                    if let Some(hs) = self.search.state_mut() {
                        let n = hs.available_types().len() + 1;
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
                _ => return None,
            }
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return None;
        }
        let input_focused = self.search.state().is_none_or(|s| s.input_focused);
        match key.code {
            KeyCode::Esc => {
                self.search.close();
            }
            KeyCode::Tab => {
                if let Some(hs) = self.search.state_mut() {
                    hs.input_focused = !hs.input_focused;
                }
            }
            KeyCode::Backspace if input_focused => {
                let empty = self.search.state().is_none_or(|s| s.query.is_empty());
                if empty {
                    self.search.close();
                } else {
                    self.search.state_mut().unwrap().query.pop();
                }
            }
            KeyCode::Up => {
                if let Some(hs) = self.search.state_mut() {
                    hs.cursor = hs.cursor.saturating_sub(1);
                    if hs.cursor < hs.scroll {
                        hs.scroll = hs.cursor;
                    }
                }
            }
            KeyCode::Down => {
                if let Some(hs) = self.search.state_mut() {
                    let max = hs.filtered_count().saturating_sub(1);
                    hs.cursor = (hs.cursor + 1).min(max);
                }
            }
            KeyCode::Enter => {
                let (query, last_query, loading, has_results) = self
                    .search
                    .state()
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
                    self.search.prepare_query(&query);
                    self.spawn_global_search(query);
                } else if has_results {
                    self.select_home();
                }
            }
            KeyCode::Char('q') if !input_focused && key.modifiers.is_empty() => {
                return Some(self.try_quit());
            }
            KeyCode::Char(c) => {
                if let Some(hs) = self.search.state_mut() {
                    hs.input_focused = true;
                    hs.query.push(c);
                }
            }
            _ => {}
        }
        Some(false)
    }

    pub(super) fn handle_key_power_lib_search(&mut self, key: KeyEvent) -> Option<bool> {
        if self.queue_view != QUEUE_VIEW_POWER
            || key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || self.context_menu_open()
            || !matches!(self.power_focus, PowerFocus::Left)
            || self.power_left_tab == 0
        {
            return None;
        }
        let lib_idx = self.power_left_tab - 1;
        if self.libs[lib_idx].search.is_some() {
            self.handle_lib_search_key(lib_idx, key);
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn handle_key_lib_search(&mut self, key: KeyEvent) -> Option<bool> {
        if self.tab_idx <= 1
            || key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || self.context_menu_open()
        {
            return None;
        }
        if self
            .libs
            .get(self.tab_idx - self.lib_tab_offset())
            .is_none_or(|l| l.search.is_none())
        {
            return None;
        }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        self.handle_lib_search_key(lib_idx, key);
        Some(false)
    }

    pub(super) fn handle_key_confirm_clear_queue(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.confirm_clear_queue {
            return None;
        }
        self.confirm_clear_queue = false;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
        } else {
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_rescan(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.confirm_rescan {
            return None;
        }
        self.confirm_rescan = false;
        self.status.clear();
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            self.trigger_lib_rescan(lib_idx);
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
        let in_lib_search = self.tab_idx > 1
            && self
                .libs
                .get(self.tab_idx - self.lib_tab_offset())
                .is_some_and(|l| l.search.is_some());
        if in_lib_search {
            return None;
        }
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
        Some(false)
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
        if self.tab_idx == 0 {
            Some(self.handle_combined_key(key))
        } else if self.tab_idx == 1 {
            Some(self.handle_queue_key(key))
        } else {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            Some(self.handle_lib_key(lib_idx, key).unwrap_or(false))
        }
    }

    /// Global view keys shared by all three top-level view handlers
    /// (`handle_combined_key`, `handle_lib_key`, `handle_queue_key`): quit,
    /// tab cycling (incl. the power-queue-view override, since `self.tab_idx`
    /// and `self.queue_view` are read directly instead of being faked by the
    /// caller), digit tab-jump, and the context-menu key. Each handler calls
    /// this at the point in its own precedence order where these keys used
    /// to be independently matched; genuinely per-view behavior (`/` search,
    /// `Ctrl+q`/`Alt+q` enqueue) stays local. See
    /// docs/adr/0002-centralized-input-handling.md, phase 3 (#132).
    fn handle_global_view_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => Some(self.try_quit()),
            KeyCode::Tab => {
                if self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER {
                    self.power_left_tab_next();
                } else {
                    let n = (self.tab_idx + 1) % self.tab_count();
                    self.set_tab(n);
                }
                Some(false)
            }
            KeyCode::BackTab => {
                if self.tab_idx == 1 && self.queue_view == QUEUE_VIEW_POWER {
                    self.power_left_tab_prev();
                } else {
                    let n = self.tab_count();
                    self.set_tab((self.tab_idx + n - 1) % n);
                }
                Some(false)
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() {
                    self.set_tab(idx);
                }
                Some(false)
            }
            KeyCode::Char('.') => {
                self.open_context_menu();
                Some(false)
            }
            _ => None,
        }
    }

    /// `Ctrl+q`/`Alt+q`: enqueue the current selection. Shared by
    /// `handle_combined_key` and `handle_lib_key` — the queue view has no
    /// "enqueue selected" concept, so `handle_queue_key` does not call this.
    fn handle_enqueue_selected_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Char('q')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers == KeyModifiers::ALT =>
            {
                self.enqueue_selected();
                Some(false)
            }
            _ => None,
        }
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

    fn handle_lib_key(&mut self, lib_idx: usize, key: KeyEvent) -> Option<bool> {
        if let Some(quit) = self.handle_enqueue_selected_key(key) {
            return Some(quit);
        }
        if let Some(quit) = self.handle_global_view_key(key) {
            return Some(quit);
        }

        match key.code {
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
                        let ct = self.libs[lib_idx].library.collection_type.clone();
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
            // Any other Ctrl/Alt-modified character is claimed here as a
            // no-op. This mirrors the pre-phase-3 `is_lib_key` mirror's
            // broad catch-all in `handle_queue_key`'s power-left-panel
            // routing: unmapped Ctrl/Alt combos are swallowed while a
            // library sub-panel is focused, rather than leaking through to
            // an unrelated queue-view shortcut with the same bare key
            // (e.g. `Ctrl+z` must not trigger queue-undo while the library
            // panel has focus). Harmless at the other call site
            // (`handle_key_view_dispatch`), which already swallows any
            // unmatched key as the last entry in `CONTEXT_STACK`.
            KeyCode::Char(_)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) => {}
            _ => {
                return None;
            }
        }
        Some(false)
    }

    fn handle_combined_key(&mut self, key: KeyEvent) -> bool {
        if let Some(quit) = self.handle_enqueue_selected_key(key) {
            return quit;
        }
        if let Some(quit) = self.handle_global_view_key(key) {
            return quit;
        }
        match key.code {
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
            KeyCode::Char('/') => {
                self.search.open(true);
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
            KeyCode::Delete if self.home.section == 0 => self.remove_from_continue_watching(),
            _ => {}
        }
        false
    }

    pub(super) fn adjust_volume(&mut self, delta: i64) {
        self.playback_target().adjust_volume(self, delta);
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
            || self.context_menu_open()
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
                // Route non-power-nav keys to the library handler for this
                // panel. `handle_lib_key`'s own `Some`/`None` is now the
                // single source of truth for "did the library view claim
                // this key" — no more hand-maintained mirror of its key set.
                //
                // The `tab_idx` swap stays: many action methods
                // `handle_lib_key` calls into (`current_lib_item`, `select`,
                // `move_lib_cursor`, `refresh_lib`, `shuffle_play`,
                // `play_folder`, `go_back`, ...) derive their own lib index
                // from `self.tab_idx` rather than taking it as a parameter.
                // Impact analysis on `current_lib_item` alone showed 6
                // affected symbols across `execute_context_action`,
                // `enqueue_selected`, `select`, and `toggle_watched`
                // (HIGH risk) — parameterizing all of them is a separate,
                // larger follow-up, not in scope for #132.
                // Track-selection mode (#145 task 3): while the power-left
                // panel is sitting on the album-folder-listing nav level
                // (the level `render_power_library` shows inline album
                // detail for, per task 2), Enter/Escape/Up/Down are
                // reinterpreted for moving a track focus within the
                // currently-displayed album instead of drilling into
                // `nav_stack` (`select`) or moving the album cursor
                // (`move_lib_cursor`). Scoped strictly to `!is_power_nav`
                // (so Alt+arrow pane-switching is untouched) and to
                // `is_viewing_album_folders` (so movies/series/home-video
                // panels, non-power tabs, and the legacy drilled-in
                // `is_album_level` state are completely unaffected).
                if !is_power_nav && self.is_viewing_album_folders(lib_idx) {
                    match key.code {
                        KeyCode::Enter => {
                            if self.libs[lib_idx].album_track_focus.is_none() {
                                self.libs[lib_idx].album_track_focus = Some(0);
                            } else {
                                // Track already focused (#145 task 4): play
                                // it. Reuses `select()` (now track-focus
                                // aware via `current_lib_item()`) rather
                                // than duplicating queue-build logic here.
                                let saved = self.tab_idx;
                                self.tab_idx = self.lib_tab_offset() + lib_idx;
                                self.select();
                                self.tab_idx = saved;
                            }
                            return false;
                        }
                        KeyCode::Esc | KeyCode::Backspace => {
                            if self.libs[lib_idx].album_track_focus.is_some() {
                                self.libs[lib_idx].album_track_focus = None;
                                return false;
                            }
                        }
                        KeyCode::Up | KeyCode::Down => {
                            if let Some(idx) = self.libs[lib_idx].album_track_focus {
                                let track_count = self
                                    .selected_album_item(lib_idx)
                                    .and_then(|item| self.album_tracks_cache.get(&item.id))
                                    .map(|tracks| tracks.len())
                                    .unwrap_or(0);
                                if track_count > 0 {
                                    let delta: i64 = if key.code == KeyCode::Up { -1 } else { 1 };
                                    let new_idx = (idx as i64 + delta)
                                        .clamp(0, track_count as i64 - 1)
                                        as usize;
                                    self.libs[lib_idx].album_track_focus = Some(new_idx);
                                }
                                return false;
                            }
                        }
                        _ => {}
                    }
                }

                if !is_power_nav {
                    let saved = self.tab_idx;
                    self.tab_idx = self.lib_tab_offset() + lib_idx;
                    let outcome = self.handle_lib_key(lib_idx, key);
                    self.tab_idx = saved;
                    if let Some(quit) = outcome {
                        return quit;
                    }
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

        if let Some(quit) = self.handle_global_view_key(key) {
            return quit;
        }

        match key.code {
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
                self.dispatch(super::action::Command::QueuePlayCursor);
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
            KeyCode::Char('/') => {
                self.search.open(true);
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
        } else if self.search.is_open() || self.tab_idx == 0 {
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
                if self.search.is_open() {
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
                    || self.search.is_open()
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
                    || (!self.search.is_open() && self.tab_idx == 0 && self.home.section == 0)
                {
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Continue Watching",
                        ContextAction::RemoveFromContinueWatching,
                    );
                }
                if !cw_focused && !self.search.is_open() && self.tab_idx == 1 {
                    let pos = self.displayed_queue().queue_cursor;
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Queue",
                        ContextAction::RemoveFromQueue(pos),
                    );
                }
                if self.search.is_open() || self.tab_idx == 1 {
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
                        if use_row_map {
                            // Letter-grouped or banner-adjacent mode: row map gives the
                            // result index directly (None = header/banner-filler row).
                            if let Some(Some(item_idx)) = row_map_item {
                                if item_idx < s.results.len() {
                                    s.cursor = item_idx;
                                }
                            }
                        } else {
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
                    // Same `Command` the `-`/`+` keys dispatch (issue #134);
                    // only the hit-test and the wheel-to-delta mapping are
                    // mouse-specific.
                    self.dispatch(super::action::Command::AdjustVolume(-delta * 5));
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
                        // Spatial hit-test stays local (issue #134); the
                        // activation itself is the same `Command` the queue
                        // tab's `Enter` key dispatches, so double-click and
                        // `Enter` can't drift again the way they did before
                        // a70ad7a.
                        if t < queue.items.len()
                            && self.layout.queue.inner.contains((col, row).into())
                        {
                            self.dispatch(super::action::Command::QueuePlayCursor);
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

    // ── issue #134: mouse regions onto the shared `Command` vocabulary ──────

    #[test]
    fn double_click_on_queue_row_dispatches_the_same_command_as_enter() {
        use crate::app::tests::make_item;

        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.player_tab.set_items(
            vec![
                make_item("Track One", "Audio"),
                make_item("Track Two", "Audio"),
            ],
            1, // cursor already on the second item, as if arrow-keyed there
        );
        app.layout.queue.inner = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        };
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }
        // Prime the double-click detector: a prior click already landed at
        // this exact cell within the timing window.
        app.last_click_time = Instant::now();
        app.last_click_pos = (2, 2);

        let rx = app.player.spy_on_commands();
        app.handle_mouse(left_down(2, 2));

        assert!(
            matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))),
            "double-click on a queue row must dispatch Command::QueuePlayCursor, \
             the same command the queue tab's Enter key uses"
        );
    }

    #[test]
    fn scroll_wheel_on_volume_pill_dispatches_the_same_command_as_the_keys() {
        use crossterm::event::MouseEventKind;

        let mut app = make_app_stub();
        app.layout.tabbar_vol_area = Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 1,
        };
        let before = app.ui_volume;

        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        // ScrollUp maps to a +5 volume delta (mirrors the `+`/`=` keys, which
        // the scroll-wheel path now shares `Command::AdjustVolume` with).
        // Idle (`active == false`) clamps at 200, matching `adjust_volume`'s
        // existing idle-vs-active clamp split -- unrelated to this issue.
        assert_eq!(app.ui_volume, (before as i64 + 5).clamp(0, 200) as u8);
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

            album_track_focus: None,
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

    #[test]
    fn search_result_click_uses_left_row_map_past_banner_filler_rows() {
        let mut app = make_power_movie_app();

        // Replace the plain nav-stack browsing state with an active search
        // over three leaf movies, cursor on the first result -- this is what
        // triggers the inline compact banner (and its filler rows) in the
        // plain (non-grouped) render_power_list branch.
        let mut movie1 = make_item("First Movie", "Movie");
        movie1.id = "movie-1".into();
        let mut movie2 = make_item("Second Movie", "Movie");
        movie2.id = "movie-2".into();
        let mut movie3 = make_item("Third Movie", "Movie");
        movie3.id = "movie-3".into();
        let items = vec![movie1, movie2, movie3];
        app.libs[0].search = Some(LibSearch {
            query: "movie".into(),
            items,
            results: vec![0, 1, 2],
            cursor: 0,
            scroll: 0,
            loading: false,
        });

        // Render for real so layout.power.left_row_map / left_area reflect the
        // actual banner-filler rows inserted after the selected (cursor=0) row.
        let backend = TestBackend::new(100, 40);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| app.render(f)).unwrap();

        let row_map = app.layout.power.left_row_map.clone();
        assert!(
            row_map.iter().any(|r| r.is_none()),
            "expected banner filler (None) rows in left_row_map, got {:?}",
            row_map
        );

        // Find a row mapping to search result index 1 (the "Second Movie"
        // row) that sits after at least one filler row -- this is the row
        // whose click target would be computed wrong by the old naive
        // offset + click_y arithmetic, which ignored the banner filler rows
        // entirely.
        let click_row_idx = row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected a row mapping to search result index 1");
        assert!(
            click_row_idx > 1,
            "expected the row for result index 1 to be pushed down by filler rows, got index {}",
            click_row_idx
        );

        let la = app.layout.power.left_area;
        let row = la.y + click_row_idx as u16;
        let col = la.x + 1;

        let handled = app.click_set_cursor(col, row);

        assert!(handled);
        assert_eq!(
            app.libs[0].search.as_ref().unwrap().cursor,
            1,
            "click should select the row-map item index, not a naive offset + click_y index"
        );
    }

    // ── Phase 3 (#132) view-routing boundary tests ─────────────────────
    // These exercise the shared `handle_global_view_key` /
    // `handle_enqueue_selected_key` front doors and the queue view's
    // `handle_lib_key(lib_idx, key)` routing (no more `is_lib_key` mirror).

    #[test]
    fn period_key_opens_context_menu_from_all_three_view_handlers() {
        // Home (combined), library, and queue views all route '.' through
        // the shared `handle_global_view_key`.
        let mut home = make_app_stub();
        home.home
            .continue_items
            .push(crate::app::tests::make_item("Continuing", "Movie"));
        assert!(home.context_menu.is_none());
        home.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert!(home.context_menu.is_some(), "combined (home) view");

        let mut lib = make_app_stub();
        lib.tab_idx = 2;
        let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        lib.libs.push(crate::app::LibraryTab {
            library,
            nav_stack: vec![crate::app::BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![crate::app::tests::make_item("A Movie", "Movie")],
                total_count: 1,
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

            album_track_focus: None,
        });
        lib.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert!(lib.context_menu.is_some(), "library view");

        let mut queue = make_power_movie_app();
        queue.power_focus = PowerFocus::Queue;
        queue
            .player_tab
            .items
            .push(crate::app::tests::make_item("Queued", "Movie"));
        queue.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert!(queue.context_menu.is_some(), "queue view");
    }

    #[test]
    fn alt_q_enqueues_selected_from_library_view() {
        // `Ctrl+q`/`Alt+q` enqueue is shared by combined and library views
        // via `handle_enqueue_selected_key` (the queue view has no
        // "enqueue selected" concept and does not wire this in).
        let mut app = make_app_stub();
        app.tab_idx = 2;
        let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        let mut movie = crate::app::tests::make_item("A Movie", "Movie");
        movie.id = "movie-1".into();
        app.libs.push(crate::app::LibraryTab {
            library,
            nav_stack: vec![crate::app::BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![movie],
                total_count: 1,
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

            album_track_focus: None,
        });

        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));

        assert_eq!(
            app.player_tab.items.len(),
            1,
            "Alt+q enqueues from library view"
        );
        assert_eq!(app.player_tab.items[0].id, "movie-1");
    }

    #[test]
    fn ctrl_z_while_power_library_panel_focused_does_not_leak_to_queue_undo() {
        // Preserved quirk from the pre-phase-3 `is_lib_key` mirror: while a
        // library sub-panel has focus in power view, an unmapped
        // Ctrl/Alt-modified key (library has no Ctrl+z binding) must be
        // swallowed by the library routing, not fall through to the
        // queue's own Ctrl+z undo binding below it in `handle_queue_key`.
        let mut app = make_power_movie_app();
        app.queue_undo_stack.push(crate::app::UndoEntry::Remove(
            0,
            Box::new(crate::app::tests::make_item("removed", "Movie")),
        ));
        let stack_len_before = app.queue_undo_stack.len();

        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));

        assert_eq!(
            app.queue_undo_stack.len(),
            stack_len_before,
            "Ctrl+z must not pop the queue undo stack while the library panel is focused"
        );
    }

    #[test]
    fn ctrl_z_while_power_queue_panel_focused_does_trigger_undo() {
        // Positive counterpart: with queue focus (not library focus), the
        // same Ctrl+z reaches `handle_queue_key`'s own binding.
        let mut app = make_power_movie_app();
        app.power_focus = PowerFocus::Queue;
        app.queue_undo_stack.push(crate::app::UndoEntry::Remove(
            0,
            Box::new(crate::app::tests::make_item("removed", "Movie")),
        ));

        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));

        assert!(
            app.queue_undo_stack.is_empty(),
            "Ctrl+z pops the queue undo stack when the queue panel is focused"
        );
    }
}

#[cfg(test)]
mod power_music_track_focus_tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{BrowseLevel, LibraryTab, PowerFocus, QUEUE_VIEW_POWER};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Power-view music library sitting on the album-folder-listing nav
    /// level (`is_viewing_album_folders` holds): a grouped `["group",
    /// "album"]` config, mirroring `render_power_library`'s inline-detail
    /// tests, with two albums at the album level and `album-1` selected.
    fn make_power_music_album_app() -> App {
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.queue_view = QUEUE_VIEW_POWER;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;
        app.music_levels = vec!["group".into(), "album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        let mut group = make_item("Alpha", "MusicArtist");
        group.id = "group-0".into();
        group.is_folder = true;

        let mut album1 = make_item("First Album", "MusicAlbum");
        album1.id = "album-1".into();
        album1.is_folder = true;
        let mut album2 = make_item("Second Album", "MusicAlbum");
        album2.id = "album-2".into();
        album2.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-music".into(),
                    title: "Music".into(),
                    items: vec![group],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                },
                BrowseLevel {
                    parent_id: "group-0".into(),
                    title: "Alpha".into(),
                    items: vec![album1, album2],
                    total_count: 2,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                },
            ],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,
            album_track_focus: None,
        });

        app
    }

    fn push_tracks(app: &mut App, album_id: &str, count: usize) {
        let tracks: Vec<_> = (0..count)
            .map(|i| {
                let mut t = make_item(&format!("Track {i}"), "Audio");
                t.id = format!("{album_id}-track-{i}");
                t
            })
            .collect();
        app.album_tracks_cache.insert(album_id.to_string(), tracks);
    }

    #[test]
    fn enter_at_album_folder_listing_enters_track_mode_without_nav_push() {
        let mut app = make_power_music_album_app();
        let nav_len_before = app.libs[0].nav_stack.len();
        assert!(app.is_viewing_album_folders(0));
        assert!(app.libs[0].album_track_focus.is_none());

        let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(!handled);
        assert_eq!(app.libs[0].album_track_focus, Some(0));
        assert_eq!(app.libs[0].nav_stack.len(), nav_len_before);
    }

    #[test]
    fn up_down_in_track_mode_move_only_track_focus_and_clamp() {
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        app.libs[0].album_track_focus = Some(1);
        let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.libs[0].album_track_focus, Some(2));
        // Clamp at the end -- no wrap.
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.libs[0].album_track_focus, Some(2));

        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.libs[0].album_track_focus, Some(0));
        // Clamp at the start -- no wrap.
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.libs[0].album_track_focus, Some(0));

        assert_eq!(
            app.libs[0].nav_stack.last().unwrap().cursor,
            album_cursor_before,
            "track-mode Up/Down must not move the album cursor"
        );
    }

    #[test]
    fn up_down_in_track_mode_with_no_cached_tracks_is_noop() {
        let mut app = make_power_music_album_app();
        // No `push_tracks` call -- album_tracks_cache has no entry for
        // "album-1", mirroring "not yet loaded".
        app.libs[0].album_track_focus = Some(0);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        assert!(!handled);
        assert_eq!(app.libs[0].album_track_focus, Some(0));
    }

    #[test]
    fn escape_in_track_mode_clears_focus_without_go_back() {
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        app.libs[0].album_track_focus = Some(2);
        let nav_len_before = app.libs[0].nav_stack.len();
        let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

        let handled = app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(!handled);
        assert!(app.libs[0].album_track_focus.is_none());
        assert_eq!(
            app.libs[0].nav_stack.len(),
            nav_len_before,
            "Escape in track mode must not pop nav_stack (not a go_back)"
        );
        assert_eq!(
            app.libs[0].nav_stack.last().unwrap().cursor,
            album_cursor_before
        );
    }

    #[test]
    fn up_down_outside_track_mode_still_move_album_cursor() {
        let mut app = make_power_music_album_app();
        assert!(app.libs[0].album_track_focus.is_none());

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        assert!(app.libs[0].album_track_focus.is_none());
        assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
    }

    #[test]
    fn escape_outside_track_mode_still_calls_go_back_unchanged() {
        // `make_power_music_album_app`'s grouped `["group","album"]` fixture
        // sits at the *root* of the synthetic music-group view (nav_stack
        // len == 2), which `go_back`'s own pre-existing guard already
        // no-ops on ("don't pop when already at the root of a synthetic
        // group view" -- see `go_back`'s doc comment in actions.rs). The
        // regression this proves is narrower than "pops": Task 3 must route
        // Escape to the exact same `go_back()` call as before when
        // `album_track_focus` is `None`, whatever `go_back()` itself does --
        // demonstrated by comparing `handle_key(Esc)` against calling
        // `go_back()` directly on an identical, freshly-built app.
        let mut via_go_back = make_power_music_album_app();
        via_go_back.go_back();

        let mut via_escape_key = make_power_music_album_app();
        assert!(via_escape_key.libs[0].album_track_focus.is_none());
        let handled = via_escape_key.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(!handled);
        assert_eq!(
            via_escape_key.libs[0].nav_stack.len(),
            via_go_back.libs[0].nav_stack.len()
        );
        assert_eq!(
            via_escape_key.libs[0].nav_stack.last().unwrap().cursor,
            via_go_back.libs[0].nav_stack.last().unwrap().cursor
        );
    }

    fn buffer_to_string(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn render_inline_album_detail_uses_track_focus_as_cursor() {
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        app.libs[0].album_track_focus = Some(2);

        let backend = ratatui::backend::TestBackend::new(100, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| app.render(f)).unwrap();
        let out = buffer_to_string(&term);

        // The cursor marker (U+258C) must land on "Track 2"'s row -- proof
        // that the inline call site in `render_power_library` passed
        // `album_track_focus` (2) through as `cursor`, not a hardcoded 0.
        let track_line = out
            .lines()
            .find(|l| l.contains("Track 2"))
            .unwrap_or_else(|| panic!("no 'Track 2' row found in rendered output:\n{out}"));
        assert!(
            track_line.contains('\u{258c}'),
            "expected cursor marker on the focused track's row, got: {track_line:?}\nfull output:\n{out}"
        );
    }

    // ── Task 4: scope-correct actions (#145) ─────────────────────────────

    #[test]
    fn current_lib_item_in_list_mode_returns_album_folder_not_a_track() {
        // Regression: album-list mode (`album_track_focus == None`) must
        // keep resolving to the selected album folder itself, exactly as
        // before Task 4.
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        assert!(app.libs[0].album_track_focus.is_none());

        let saved = app.tab_idx;
        app.tab_idx = app.lib_tab_offset();
        let item = app.current_lib_item();
        app.tab_idx = saved;

        let item = item.expect("current_lib_item should resolve the selected album");
        assert_eq!(item.id, "album-1");
        assert!(item.is_folder, "list mode must resolve to the album folder");
    }

    #[test]
    fn current_lib_item_in_track_mode_returns_focused_track() {
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        app.libs[0].album_track_focus = Some(1);

        let saved = app.tab_idx;
        app.tab_idx = app.lib_tab_offset();
        let item = app.current_lib_item();
        app.tab_idx = saved;

        let item = item.expect("current_lib_item should resolve the focused track");
        assert_eq!(item.id, "album-1-track-1");
        assert!(
            !item.is_folder,
            "track mode must resolve to the track, not the album folder"
        );
    }

    #[test]
    fn current_lib_item_in_track_mode_falls_back_safely_when_cache_missing() {
        // Async fetch still in flight: `album_tracks_cache` has no entry for
        // "album-1" yet. Must not panic and must not index out of bounds.
        let mut app = make_power_music_album_app();
        app.libs[0].album_track_focus = Some(0);
        assert!(!app.album_tracks_cache.contains_key("album-1"));

        let saved = app.tab_idx;
        app.tab_idx = app.lib_tab_offset();
        let item = app.current_lib_item();
        app.tab_idx = saved;

        let item = item.expect("must fall back to the album folder item, not None");
        assert_eq!(item.id, "album-1");
        assert!(item.is_folder);
    }

    #[test]
    fn enter_again_in_track_mode_plays_focused_track_from_cached_queue() {
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        app.libs[0].album_track_focus = Some(1);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(!handled);
        // Queue built from the cached album tracks, starting at the focused
        // track (index 1 -> "album-1-track-1").
        let ids: Vec<_> = app.player_tab.items.iter().map(|i| i.id.clone()).collect();
        assert_eq!(
            ids,
            vec!["album-1-track-0", "album-1-track-1", "album-1-track-2"]
        );
        assert_eq!(app.player_tab.queue_cursor, 1);
        // Note: `app.queue_source` is not asserted here -- `play_items_routed`
        // (pre-existing, out of Task 4's scope) calls
        // `on_queue_replace_silent` as its first statement, which
        // unconditionally resets `queue_source` to `Unknown` immediately
        // after `select()` sets it to `Album`. That happens identically on
        // the legacy `is_album_level` path (see the regression test below),
        // so it is not a Task-4 regression -- the queue *contents* (ids +
        // cursor, asserted above) are the correct observable here.
    }

    #[test]
    fn enter_again_in_track_mode_with_missing_cache_does_not_panic() {
        let mut app = make_power_music_album_app();
        // No `push_tracks` -- cache miss, async fetch still in flight.
        app.libs[0].album_track_focus = Some(0);

        let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(!handled);
    }

    #[test]
    fn context_menu_in_list_mode_offers_folder_scoped_actions_for_selected_album() {
        // Regression: album-list mode's context menu must still target the
        // selected ALBUM's id via the folder-scoped actions.
        let mut app = make_power_music_album_app();
        assert!(app.libs[0].album_track_focus.is_none());

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu should open");
        let actions: Vec<_> = menu
            .entries
            .iter()
            .filter_map(|e| e.action.clone())
            .collect();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ContextAction::PlayFolder(id) if id == "album-1")),
            "expected PlayFolder(\"album-1\"), got: {actions:?}"
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ContextAction::ShuffleFolder(id) if id == "album-1")),
            "expected ShuffleFolder(\"album-1\"), got: {actions:?}"
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ContextAction::EnqueueFolder(item) if item.id == "album-1")),
            "expected EnqueueFolder(album-1), got: {actions:?}"
        );
    }

    #[test]
    fn context_menu_in_track_mode_offers_track_scoped_actions_not_folder_actions() {
        let mut app = make_power_music_album_app();
        push_tracks(&mut app, "album-1", 3);
        app.libs[0].album_track_focus = Some(1);

        app.open_context_menu();

        let menu = app.context_menu.as_ref().expect("context menu should open");
        let actions: Vec<_> = menu
            .entries
            .iter()
            .filter_map(|e| e.action.clone())
            .collect();
        assert!(
            actions.iter().any(|a| matches!(a, ContextAction::Play)),
            "track mode must offer the generic per-item Play action, got: {actions:?}"
        );
        assert!(
            actions.iter().any(|a| matches!(a, ContextAction::Enqueue)),
            "track mode must offer the generic per-item Enqueue action, got: {actions:?}"
        );
        assert!(
            !actions.iter().any(|a| matches!(
                a,
                ContextAction::PlayFolder(_)
                    | ContextAction::ShuffleFolder(_)
                    | ContextAction::EnqueueFolder(_)
            )),
            "track mode must not offer album-folder-scoped actions, got: {actions:?}"
        );
    }

    #[test]
    fn legacy_is_album_level_drilldown_enter_to_play_is_unaffected() {
        // The pre-#145 drilldown (`is_album_level`, reached elsewhere in the
        // app by pushing a nav_stack level of tracks) must keep working
        // exactly as before -- Task 4 must not change its behavior.
        let mut app = make_app_stub();
        app.tab_idx = 2;
        app.music_levels = vec!["album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        let mut album = make_item("An Album", "MusicAlbum");
        album.id = "album-legacy".into();
        album.is_folder = true;

        let mut track0 = make_item("Track 0", "Audio");
        track0.id = "legacy-track-0".into();
        let mut track1 = make_item("Track 1", "Audio");
        track1.id = "legacy-track-1".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-music".into(),
                    title: "Music".into(),
                    items: vec![album],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                },
                BrowseLevel {
                    parent_id: "album-legacy".into(),
                    title: "An Album".into(),
                    items: vec![track0, track1],
                    total_count: 2,
                    cursor: 1,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                },
            ],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,
            album_track_focus: None,
        });

        assert!(app.is_album_level(0));
        assert!(!app.is_viewing_album_folders(0));

        app.select();

        let ids: Vec<_> = app.player_tab.items.iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids, vec!["legacy-track-0", "legacy-track-1"]);
        assert_eq!(app.player_tab.queue_cursor, 1);
        // See the note on `enter_again_in_track_mode_plays_focused_track_
        // from_cached_queue` above -- `app.queue_source` gets reset to
        // `Unknown` by `play_items_routed`'s call to
        // `on_queue_replace_silent` regardless of what `select()` set it to
        // just before, on both the legacy and new track-focus paths. Not a
        // Task-4 regression; queue contents are the correct observable.
    }
}
