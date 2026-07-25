use super::action::{power_album_track_command_for_key, Command};
use super::input_resolver::KeyChord;
use super::layout::LibraryRowTarget;
use super::settings::settings_total_rows;
use super::PanelFocus;
use super::{
    App, ContextAction, ContextMenu, LibSearch, PendingQueueAction, QueueScope, SavePlaylistDialog,
    SavePlaylistStage, HELP_PANEL_W, PLAYLISTS_PANEL_W, POWER_LEFT_WIDTH_DEFAULT,
    POWER_LEFT_WIDTH_STEP, SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

impl App {
    /// Whether a context menu is currently open. Shared by every
    /// CONTEXT_STACK layer above `context_menu` that must yield to it
    /// (`power_sidebar_toggle_h`, `home_search`, `power_lib_search`, `lib_search`,
    /// `clear_queue_prompt_c`, `queue_column_width`) — see
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
        if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
            Some(self.library_tab - 1)
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

    /// Home + one tab per library (no Queue tab -- the queue is the
    /// always-visible left column, not a tab).
    pub(super) fn tab_count(&self) -> usize {
        1 + self.libs.len()
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

    pub(super) fn active_power_album_track_lib_idx(&self) -> Option<usize> {
        if self.library_tab == 0 {
            return None;
        }
        let lib_idx = self.library_tab - 1;
        let lib = self.libs.get(lib_idx)?;
        if lib.album_track_focus.is_some() && self.is_viewing_album_folders(lib_idx) {
            Some(lib_idx)
        } else {
            None
        }
    }

    pub(super) fn handle_key_power_album_track_mode(&mut self, key: KeyEvent) -> Option<bool> {
        let lib_idx = self.active_power_album_track_lib_idx()?;
        let command = power_album_track_command_for_key(KeyChord::from_key(key), lib_idx)?;
        Some(self.dispatch(command))
    }

    pub(super) fn handle_key_queue_column_width(&mut self, key: KeyEvent) -> Option<bool> {
        if self.handle_queue_column_width_key(key) {
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn handle_key_power_sidebar_toggle(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code != KeyCode::Char('h') || !key.modifiers.is_empty() || self.context_menu_open() {
            return None;
        }
        Some(self.dispatch(Command::TogglePowerSidebar))
    }

    pub(super) fn handle_key_home_search(&mut self, key: KeyEvent) -> Option<bool> {
        if self.library_tab != 0 || !self.search.is_open() || self.context_menu_open() {
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
        if key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || self.context_menu_open()
            || !matches!(self.panel_focus, PanelFocus::Library)
            || self.library_tab == 0
        {
            return None;
        }
        // Let Power View's shared Tab/BackTab cycling path claim these keys.
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return None;
        }
        let lib_idx = self.library_tab - 1;
        if key.code == KeyCode::Enter && self.power_selected_series_item(lib_idx).is_some() {
            return None;
        }
        if self.libs[lib_idx].search.is_some() {
            self.handle_lib_search_key(lib_idx, key);
            Some(false)
        } else {
            None
        }
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
        let pending_lib_idx = self.pending_rescan_lib_idx.take();
        self.status.clear();
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            let lib_idx = pending_lib_idx.unwrap_or_else(|| {
                if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
                    self.library_tab - 1
                } else {
                    0
                }
            });
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
        self.status = "Clear queue? (Y/n)".into();
        self.confirm_clear_queue = true;
        Some(false)
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
        // `handle_queue_key` (despite its name -- a holdover from when this
        // was Standard's Queue-tab handler) is Power's single left-column
        // dispatch: it branches internally on `panel_focus`/`library_tab`
        // to route to Home (`handle_power_cw_key`), a library
        // (`handle_lib_key`), or genuine queue-cursor movement.
        Some(self.handle_queue_key(key))
    }

    /// Global view keys shared by the left-column handlers (`handle_lib_key`,
    /// `handle_queue_key`, and Home nav via `handle_power_cw_key`): quit, tab
    /// cycling, digit tab-jump, and the context-menu key. Each handler calls
    /// this at the point in its own precedence order where these keys used
    /// to be independently matched; genuinely per-view behavior (`/` search,
    /// `Ctrl+a` enqueue) stays local. See
    /// docs/adr/0002-centralized-input-handling.md, phase 3 (#132).
    fn handle_global_view_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => Some(self.try_quit()),
            KeyCode::Tab => {
                self.library_tab_next();
                Some(false)
            }
            KeyCode::BackTab => {
                self.library_tab_prev();
                Some(false)
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() {
                    self.set_library_tab(idx);
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

    /// `Ctrl+a`: enqueue the current selection. Shared by `handle_lib_key`
    /// (Home nav has its own enqueue binding) -- the queue view has no
    /// "enqueue selected" concept, so `handle_queue_key` does not call this.
    ///
    /// Issue #209: `a` (no modifier) is the playback-context audio-track
    /// binding (`playback_command_for_key`'s `ToggleMuteOrCycleAudio`,
    /// guarded `!ctrl` there so Ctrl+a never reaches it). Previously Ctrl+a
    /// fell through this front door unbound and hit that playback binding
    /// unguarded, muting/cycling audio instead of enqueuing. This arm is
    /// the actual fix: Ctrl+a now means "enqueue" here, before the playback
    /// context even sees the key. Replaces the old `Ctrl+q`/`Alt+q`
    /// bindings, which no longer enqueue.
    fn handle_enqueue_selected_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enqueue_selected();
                Some(false)
            }
            _ => None,
        }
    }

    fn handle_lib_search_key(&mut self, lib_idx: usize, key: KeyEvent) {
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
            KeyCode::Enter => {
                if self.activate_recursive_album(lib_idx) {
                    // active-search jump; unchanged
                } else if self.is_viewing_album_folders(lib_idx) {
                    self.activate_album_folder_row(lib_idx);
                } else {
                    self.select();
                }
            }
            KeyCode::Char(c) => {
                self.libs[lib_idx].search.as_mut().unwrap().query.push(c);
                self.update_lib_search(lib_idx);
            }
            _ => {}
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
                if !self.page_power_grouped_album_cursor(lib_idx, false) {
                    let p = self.lib_page_size();
                    self.move_lib_cursor(-(p as i64));
                }
            }
            KeyCode::PageDown => {
                if !self.page_power_grouped_album_cursor(lib_idx, true) {
                    let p = self.lib_page_size();
                    self.move_lib_cursor(p as i64);
                }
            }
            KeyCode::Home => self.jump_lib_cursor(false),
            KeyCode::End => self.jump_lib_cursor(true),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.play_selected_artist_header(false) {
                    return Some(false);
                }
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
                if self.play_selected_artist_header(true) {
                    return Some(false);
                }
                self.shuffle_play()
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let name = self.libs[lib_idx].library.name.clone();
                self.status = format!("Rescan '{name}'? (Y/n)");
                self.confirm_rescan = true;
                self.pending_rescan_lib_idx = Some(lib_idx);
            }
            KeyCode::Char('r') => self.refresh_lib(),
            KeyCode::Char('/') => {
                if self.open_recursive_album_search(lib_idx) {
                    return Some(false);
                }
                let (items, needs_full_load) = if self.is_feed_home_video_group_view(lib_idx) {
                    (self.feed_home_video_selected_items(lib_idx), false)
                } else {
                    self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| {
                            let all = l.all_items.clone().unwrap_or_else(|| l.items.clone());
                            // With a letter-range pill active, `l.total_count`
                            // is the FILTERED range's count, not the whole
                            // library's -- `l.items.len() < l.total_count`
                            // alone would read a fully-loaded small range as
                            // "nothing more to fetch" and search would run
                            // over just that range. Force the full-library
                            // fetch whenever a filter is active and it
                            // hasn't already happened (`all_items` still
                            // unset); `spawn_search_items_load` always fetches
                            // the whole library unfiltered (see there).
                            let needs = l.all_items.is_none()
                                && (l.letter_filter.is_some() || l.items.len() < l.total_count);
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
            super::input_resolver::KeyResolution::Command(
                cmd @ super::action::Command::TogglePlayPause,
            ) => {
                let now = Instant::now();
                let double_tap = self
                    .last_space_press
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                self.last_space_press = Some(now);
                if double_tap {
                    self.last_space_press = None;
                    Some(self.dispatch(cmd))
                } else {
                    None
                }
            }
            super::input_resolver::KeyResolution::Command(cmd @ super::action::Command::Stop) => {
                let now = Instant::now();
                let double_tap = self
                    .last_esc_press
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                self.last_esc_press = Some(now);
                if double_tap {
                    self.last_esc_press = None;
                    Some(self.dispatch(cmd))
                } else {
                    None
                }
            }
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
                self.power_home_select_start();
                true
            }
            KeyCode::End => {
                self.power_home_select_end();
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
            // Ctrl+a: enqueue (issue #209). Replaces the old Ctrl+q/Alt+q
            // bindings, which no longer enqueue — see
            // `handle_enqueue_selected_key`'s doc comment for why Ctrl+a
            // specifically had to become the enqueue key here.
            KeyCode::Char('a') if ctrl => {
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
                let cursor = self.home.home_cursor;
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
        (self.layout.main.left_area.height as usize).max(1)
    }

    fn is_queue_column_width_resize_key(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Left | KeyCode::Right) && key.modifiers == KeyModifiers::SHIFT
    }

    fn handle_queue_column_width_key(&mut self, key: KeyEvent) -> bool {
        if self.context_menu_open()
            || self.queue_column_collapsed
            || !Self::is_queue_column_width_resize_key(key)
        {
            return false;
        }

        let max_width = Self::queue_column_width_max_for_terminal(self.terminal_width);
        let next_width = if key.code == KeyCode::Left {
            self.queue_column_width
                .saturating_sub(POWER_LEFT_WIDTH_STEP)
        } else {
            self.queue_column_width
                .saturating_add(POWER_LEFT_WIDTH_STEP)
        };
        let normalized = Self::normalize_queue_column_width(next_width, self.terminal_width);
        if normalized == self.queue_column_width {
            let limit = if key.code == KeyCode::Left {
                format!("Power view width already at minimum ({POWER_LEFT_WIDTH_DEFAULT} cols)")
            } else {
                format!("Power view width already at maximum ({max_width} cols)")
            };
            self.flash_status(limit);
            return true;
        }

        self.queue_column_width = normalized;
        self.save_prefs();
        self.flash_status(format!(
            "Power view width: {} cols",
            self.queue_column_width
        ));
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

        // Bare Left/Right switch focus between the two panels. Queue is on
        // the left; library is on the right.
        if key.modifiers.is_empty() {
            if key.code == KeyCode::Right && matches!(self.panel_focus, PanelFocus::Queue) {
                self.set_panel_focus(PanelFocus::Library);
                self.last_card_height = 0; // reset stale image height for new view
                return false;
            }
            if key.code == KeyCode::Left
                && matches!(self.panel_focus, PanelFocus::Library)
                && !self.queue_column_collapsed
            {
                self.set_panel_focus(PanelFocus::Queue);
                self.last_card_height = 0;
                return false;
            }
        }

        // Bracket keys are panel-scoped; the queue panel owns Local/Remote
        // scope switching, while the left panel keeps its section/season/
        // group bracket actions.
        if matches!(self.panel_focus, PanelFocus::Queue) {
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

        // Route nav keys to the focused library panel.
        if matches!(self.panel_focus, PanelFocus::Library) {
            if self.library_tab == 0 && self.handle_power_cw_key(key) {
                return false;
            }
            if self.library_tab > 0 {
                let lib_idx = self.library_tab - 1;

                // Season switching: [ = previous season, ] = next season.
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
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
                    // Letter-range pill cycling for large non-music libraries
                    // (`[`/`]` are otherwise free at the top browse level).
                    if key.code == KeyCode::Char('[') && self.should_show_letter_pills(lib_idx) {
                        self.cycle_letter_pill(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.should_show_letter_pills(lib_idx) {
                        self.cycle_letter_pill(lib_idx, 1);
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
                // The `library_tab` derivation stays: many action methods
                // `handle_lib_key` calls into (`current_lib_item`, `select`,
                // `move_lib_cursor`, `refresh_lib`, `shuffle_play`,
                // `play_folder`, `go_back`, ...) derive their own lib index
                // from `self.library_tab` rather than taking it as a parameter.
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
                // panels and non-power tabs are completely unaffected; the
                // legacy `is_album_level` drilldown this used to also
                // exclude was removed entirely -- Enter now always routes
                // here via `activate_album_folder_row`).
                if !is_power_nav && self.is_viewing_album_folders(lib_idx) {
                    match key.code {
                        KeyCode::Enter => {
                            self.activate_album_folder_row(lib_idx);
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

                // Series-selection mode: while the power-left panel has a
                // Series item selected and selection mode is active, Enter/
                // Escape/Up/Down/[/] are intercepted for navigating within
                // the inline series detail (season pills + episode list)
                // instead of drilling into `nav_stack` (`select`) or
                // moving the list cursor (`move_lib_cursor`).
                if !is_power_nav && self.libs[lib_idx].series_selection.is_some() {
                    match key.code {
                        KeyCode::Enter => {
                            // Play the focused episode in selection mode.
                            if let Some(episodes) = self.series_selection_episodes(lib_idx) {
                                let ep_idx = self.libs[lib_idx].series_selection.unwrap_or(0);
                                if let Some(ep) = episodes.get(ep_idx) {
                                    let ep = ep.clone();
                                    self.libs[lib_idx].series_selection = None;
                                    self.play_item(ep);
                                }
                            }
                            return false;
                        }
                        KeyCode::Esc | KeyCode::Backspace => {
                            self.libs[lib_idx].series_selection = None;
                            return false;
                        }
                        KeyCode::Up | KeyCode::Down => {
                            let delta: i64 = if key.code == KeyCode::Up { -1 } else { 1 };
                            if let Some(episodes) = self.series_selection_episodes(lib_idx) {
                                let count = episodes.len();
                                if count > 0 {
                                    let cur = self.libs[lib_idx].series_selection.unwrap_or(0);
                                    let new_idx =
                                        (cur as i64 + delta).clamp(0, count as i64 - 1) as usize;
                                    self.libs[lib_idx].series_selection = Some(new_idx);
                                }
                            }
                            return false;
                        }
                        KeyCode::Char('[') => {
                            self.switch_series_selection_season(lib_idx, -1);
                            return false;
                        }
                        KeyCode::Char(']') => {
                            self.switch_series_selection_season(lib_idx, 1);
                            return false;
                        }
                        _ => {}
                    }
                }
                // Activate series-selection mode on Enter when the cursor is
                // on a Series item (instead of drilling down via `select`).
                if !is_power_nav
                    && key.code == KeyCode::Enter
                    && self.libs[lib_idx].series_selection.is_none()
                {
                    if let Some(item) = self.power_selected_series_item(lib_idx) {
                        self.enter_series_selection(lib_idx, &item);
                        return false;
                    }
                }

                // Let Power View's shared Tab/BackTab cycling path run after this block.
                if !is_power_nav && !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    if let Some(quit) = self.handle_lib_key(lib_idx, key) {
                        return quit;
                    }
                }
            }
        }

        // Queue focus: PageUp/PageDown use the actual queue panel height.
        if matches!(self.panel_focus, PanelFocus::Queue) {
            let page = self.layout.main.queue_area.height.saturating_sub(1).max(1) as usize;
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
            KeyCode::Left | KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                self.library_tab_prev();
            }
            KeyCode::Right | KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                self.library_tab_next();
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
        if self.library_tab < self.tab_scroll {
            self.tab_scroll = self.library_tab;
            return;
        }
        let tab_w = self
            .terminal_width
            .saturating_sub(super::TABBAR_LEFT_RESERVE + super::TABBAR_RIGHT_RESERVE);
        loop {
            let (_, end) = self.visible_tab_range(tab_w);
            if self.library_tab < end {
                break;
            }
            self.tab_scroll += 1;
        }
    }

    /// Tab-bar title widths: Home + one per library (no Queue tab -- see
    /// `tab_count`).
    fn tab_title_widths(&self) -> Vec<u16> {
        let pad: u16 = 2;
        let mut w = vec!["Home".chars().count() as u16 + pad];
        for l in &self.libs {
            w.push(l.library.name.chars().count() as u16 + pad);
        }
        w
    }

    /// Map a column click to a left-panel tab index (0 = Home, 1+ = library),
    /// scroll-aware: returns `usize::MAX - 1` for a click on the `«` arrow
    /// and `usize::MAX` for a click on the `»` arrow (see `handle_mouse`).
    fn power_tab_idx_at(&self, col: u16) -> Option<usize> {
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

    pub(super) fn open_context_menu(&mut self) {
        let mut entries: Vec<super::ContextMenuEntry> = vec![];

        let cw_focused = matches!(self.panel_focus, PanelFocus::Library) && self.library_tab == 0;
        let power_lib_idx = self.context_menu_power_lib_idx();
        let in_podcast = power_lib_idx.is_some_and(|idx| self.is_podcast_library(idx))
            || self.is_in_podcast_library();
        let podcast_bulk_ids = power_lib_idx.and_then(|lib_idx| {
            if in_podcast && self.is_feed_home_video_group_view(lib_idx) {
                Some((
                    self.podcast_mark_all_ids(lib_idx),
                    self.podcast_mark_all_unplayed_ids(lib_idx),
                ))
            } else {
                None
            }
        });
        let artist_header_context = power_lib_idx
            .and_then(|lib_idx| self.selected_artist_header_album_items(lib_idx))
            .map(|(selection, _)| selection);

        let current_item = if artist_header_context.is_some() {
            None
        } else if cw_focused {
            self.home
                .continue_items
                .get(self.home.continue_cursor)
                .cloned()
        } else if power_lib_idx.is_some() {
            self.current_lib_item()
        } else if self.search.is_open() {
            self.current_home_item()
        } else if matches!(self.panel_focus, PanelFocus::Queue) {
            let queue = self.displayed_queue();
            queue.items.get(queue.queue_cursor).cloned()
        } else {
            None
        };

        if let Some(selection) = artist_header_context {
            Self::push_context_action(
                &mut entries,
                "Play All",
                ContextAction::PlayArtistHeader(selection.clone()),
            );
            Self::push_context_action(
                &mut entries,
                "Shuffle",
                ContextAction::ShuffleArtistHeader(selection.clone()),
            );
            Self::push_context_action(
                &mut entries,
                "Add to Queue",
                ContextAction::EnqueueArtistHeader(selection),
            );
        } else if let Some(ref item) = current_item {
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
                    || !matches!(self.panel_focus, PanelFocus::Queue)
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
                    || (!self.search.is_open() && self.library_tab == 0 && self.home.section == 0)
                {
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Continue Watching",
                        ContextAction::RemoveFromContinueWatching,
                    );
                }
                if !cw_focused
                    && !self.search.is_open()
                    && matches!(self.panel_focus, PanelFocus::Queue)
                {
                    let pos = self.displayed_queue().queue_cursor;
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Queue",
                        ContextAction::RemoveFromQueue(pos),
                    );
                }
                if self.search.is_open() || matches!(self.panel_focus, PanelFocus::Queue) {
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
        match self.panel_focus {
            PanelFocus::Library => {
                let area = self.layout.main.left_area;
                if area.width > 0 {
                    let y = self.layout.main.cursor_screen_y.unwrap_or(area.y);
                    let x = area.x + 2;
                    // Avoid inline image overlap (detail/episode poster).
                    if let Some(img) = self.layout.main.inline_image_rect {
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
            PanelFocus::Queue => {
                let area = self.layout.main.queue_area;
                if area.width > 0 {
                    let y = self.layout.main.queue_cursor_screen_y.unwrap_or(area.y);
                    return (area.x + 2, y);
                }
            }
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
        // New keys only (#361) -- readers still fall back to the old
        // `panel_focus`/`library_tab`/`queue_column_width` keys in
        // `load_prefs`'s callers; that fallback can be deleted a release
        // later. `tab_idx` is gone outright, not migrated: it was
        // Standard-view-only state.
        let v = serde_json::json!({
            "ui_volume": self.ui_volume,
            "mute_on": self.mute_on,
            "pre_mute_volume": self.pre_mute_volume,
            "panel_focus": self.panel_focus.pref_value(),
            "library_tab": self.library_tab,
            "queue_column_width": self.queue_column_width,
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
        {
            if self.has_direct_remote_queue() {
                if self
                    .layout
                    .main
                    .queue_scope_local_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Local);
                    return true;
                }
                if self
                    .layout
                    .main
                    .queue_scope_remote_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return true;
                }
            }
            // Click in queue area: focus queue and move cursor.
            let qa = self.layout.main.queue_area;
            if qa.contains((col, row).into()) {
                if !matches!(self.panel_focus, PanelFocus::Queue) {
                    self.last_card_height = 0;
                }
                self.set_panel_focus(PanelFocus::Queue);
                let content_y = (row - qa.y) as usize;
                if let Some(&Some(item_idx)) = self.layout.main.queue_row_map.get(content_y) {
                    self.displayed_queue_mut().queue_cursor = item_idx;
                }
                return true;
            }
            // Click in the left panel: focus it and set its cursor.
            let la = self.layout.main.left_area;
            if la.contains((col, row).into()) {
                if !matches!(self.panel_focus, PanelFocus::Library) {
                    self.last_card_height = 0;
                }
                self.set_panel_focus(PanelFocus::Library);
                if self.library_tab == 0 {
                    // Home tab: rectangle hit-test the two-column card grid.
                    let pos = (col, row).into();
                    if let Some((_, flat_idx)) = self
                        .layout
                        .main
                        .home
                        .hitmap
                        .iter()
                        .find(|(rect, _)| rect.contains(pos))
                    {
                        self.home.home_cursor = *flat_idx;
                    }
                } else {
                    let lib_idx = self.library_tab - 1;
                    if self.is_music_group_view(lib_idx)
                        || self.is_feed_home_video_group_view(lib_idx)
                        || self.should_show_letter_pills(lib_idx)
                    {
                        for (rect, target) in self.layout.main.selector_tabs.clone() {
                            if rect.contains((col, row).into()) {
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else if self.is_feed_home_video_group_view(lib_idx) {
                                    self.select_feed_folder_group(lib_idx, target);
                                } else {
                                    self.select_letter_pill(lib_idx, target);
                                }
                                return true;
                            }
                        }
                    }
                    let click_y = (row - la.y) as usize;
                    // Read the row map before taking a mutable borrow on libs (borrow checker).
                    let use_row_map = !self.layout.main.left_row_map.is_empty();
                    let row_map_item = if use_row_map {
                        self.layout.main.left_row_map.get(click_y).copied()
                    } else {
                        None
                    };
                    let row_target = self
                        .layout
                        .main
                        .left_row_targets
                        .get(click_y)
                        .cloned()
                        .flatten();
                    if self.is_music_group_view(lib_idx) {
                        match row_target {
                            Some(LibraryRowTarget::ArtistHeader(selection)) => {
                                self.libs[lib_idx].album_track_focus = None;
                                self.libs[lib_idx].artist_header_focus = Some(selection);
                                self.save_default_library_position(lib_idx);
                                return true;
                            }
                            Some(LibraryRowTarget::Album(item_idx)) => {
                                let lib = &mut self.libs[lib_idx];
                                if let Some(lvl) = lib.nav_stack.last_mut() {
                                    if item_idx < lvl.items.len() {
                                        if lvl.cursor != item_idx {
                                            lib.album_track_focus = None;
                                        }
                                        lib.artist_header_focus = None;
                                        lvl.cursor = item_idx;
                                        self.save_default_library_position(lib_idx);
                                        return true;
                                    }
                                }
                            }
                            None => {}
                        }
                    }
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
                        self.save_default_library_position(lib_idx);
                    } else if let Some(lvl) = lib.nav_stack.last_mut() {
                        if use_row_map {
                            // Letter-grouped mode: row map gives item index (None = header row).
                            if let Some(Some(item_idx)) = row_map_item {
                                if item_idx < lvl.items.len() {
                                    if lvl.cursor != item_idx {
                                        lib.album_track_focus = None;
                                    }
                                    lib.artist_header_focus = None;
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
                                if lvl.cursor != clicked {
                                    lib.album_track_focus = None;
                                }
                                lib.artist_header_focus = None;
                                lvl.cursor = clicked;
                            }
                        }
                        self.save_default_library_position(lib_idx);
                    }
                }
                return true;
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

    pub(super) fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};
        let col = mouse.column;
        let row = mouse.row;
        // Always track mouse position so hover rendering is up to date.
        self.mouse_col = col;
        self.mouse_row = row;

        // Swallow the single click that merely refocused the window (e.g.
        // alt-tab back in by clicking): if a FocusGained landed within the
        // last 150ms, this Down(Left)/Down(Right) is that click, not a UI
        // action. `.take()` makes this strictly one-shot -- it clears
        // `refocus_at` whether or not the click was in the window, so a
        // second click right after a suppressed one dispatches normally.
        if matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
        ) {
            if let Some(t) = self.refocus_at.take() {
                if t.elapsed() < Duration::from_millis(150) {
                    log::debug!(target: "input", "suppressed refocus click at ({col}, {row})");
                    return;
                }
            }
        }

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
            // Tab clicks change the left-panel selection.
            if let Some(idx) = self.power_tab_idx_at(col) {
                if idx == usize::MAX - 1 {
                    self.tab_scroll = self.tab_scroll.saturating_sub(1);
                } else if idx == usize::MAX {
                    let max_scroll = self.tab_count().saturating_sub(1);
                    self.tab_scroll = (self.tab_scroll + 1).min(max_scroll);
                } else {
                    self.set_library_tab(idx);
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
                // Scroll in whichever panel the mouse is over.
                let queue_area = self.layout.main.queue_area;
                let left_area = self.layout.main.left_area;
                if queue_area.contains((col, row).into()) {
                    let n = self.displayed_queue().items.len();
                    if n > 0 {
                        let delta = delta * 3;
                        let queue = self.displayed_queue_mut();
                        queue.queue_cursor =
                            (queue.queue_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    }
                } else if left_area.contains((col, row).into()) {
                    if self.library_tab == 0 {
                        self.power_cw_move_cursor(delta);
                    } else {
                        self.move_lib_cursor(delta);
                    }
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

                let is_double = now.duration_since(self.last_click_time)
                    < Duration::from_millis(400)
                    && self.last_click_pos == (col, row);
                self.last_click_time = now;
                self.last_click_pos = (col, row);

                {
                    for (rect, target) in self.layout.main.selector_tabs.clone() {
                        if rect.contains((col, row).into()) {
                            if self.library_tab == 0 {
                                self.power_home_select_section(target);
                            } else {
                                let lib_idx = self.library_tab - 1;
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else if self.is_feed_home_video_group_view(lib_idx) {
                                    self.select_feed_folder_group(lib_idx, target);
                                } else if self.should_show_letter_pills(lib_idx) {
                                    self.select_letter_pill(lib_idx, target);
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
                    if matches!(self.panel_focus, PanelFocus::Queue) {
                        let queue = self.displayed_queue();
                        let t = queue.queue_cursor;
                        // Spatial hit-test stays local (issue #134); the
                        // activation itself is the same `Command` the queue
                        // tab's `Enter` key dispatches, so double-click and
                        // `Enter` can't drift again the way they did before
                        // a70ad7a.
                        if t < queue.items.len()
                            && self.layout.main.queue_area.contains((col, row).into())
                        {
                            self.dispatch(super::action::Command::QueuePlayCursor);
                        }
                    } else if self.library_tab == 0 {
                        self.power_home_play();
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
                if self.layout.playback.stop_area.contains((col, row).into()) {
                    let stop_avail = self.connected_session_id.is_some()
                        || self.player.status.lock().unwrap().active;
                    if stop_avail {
                        self.dispatch(super::action::Command::Stop);
                    }
                    return;
                }
                if self.layout.playback.next_area.contains((col, row).into()) {
                    if self.transport_prev_next_available().1 {
                        self.dispatch(super::action::Command::NextTrack);
                    }
                    return;
                }
                // Header breadcrumb clicks.
                if self.library_tab > 0 {
                    let crumbs = self.layout.main.breadcrumbs.clone();
                    let lib_idx = self.library_tab - 1;
                    for (x_start, x_end, crumb_row, target_depth) in crumbs {
                        if row == crumb_row && col >= x_start && col < x_end {
                            self.libs[lib_idx].nav_stack.truncate(target_depth);
                            self.libs[lib_idx].search = None;
                            self.save_default_library_position(lib_idx);
                            return;
                        }
                    }
                }

                // Capture the row that was already selected *before* this
                // click moves the cursor. Clicking a folder row normally
                // emulates Enter (drills in), but if the click landed on
                // the row that was already selected (e.g. re-clicking the
                // current row), drilling in again produces a jarring,
                // unrequested navigation. Treat that case as a no-op
                // instead.
                let prev_id = self.current_lib_item().map(|i| i.id);
                let hit = self.click_set_cursor(col, row);
                if hit && self.library_tab > 0 {
                    let lib_idx = self.library_tab - 1;
                    if self.activate_recursive_album(lib_idx) {
                        // active-search jump; unchanged
                    } else if self.is_viewing_album_folders(lib_idx) {
                        // Track-selection mode only opens via Enter; mouse
                        // click never opens it. If it's already open, a click
                        // still plays the focused track (mirrors Enter's
                        // "already focused" branch inside
                        // `activate_album_folder_row`), but cannot open it.
                        if self.libs[lib_idx].album_track_focus.is_some() {
                            self.activate_album_folder_row(lib_idx);
                        }
                    } else {
                        let cur_item = self.current_lib_item();
                        let already_selected = cur_item
                            .as_ref()
                            .is_some_and(|i| prev_id.as_deref() == Some(i.id.as_str()));
                        // TV `Series` rows are always `is_folder` (they have
                        // seasons underneath), but Enter on a Series row
                        // doesn't drill into a generic folder browse -- it
                        // opens the inline series-selection detail (season
                        // pills + episode list; see
                        // `enter_series_selection`). A mouse click has no
                        // equivalent of that inline mode, so calling
                        // `select()` here would instead push a raw
                        // folder-browse nav level, a screen the click never
                        // asked for. Mouse clicks on Series rows -- and on
                        // any row that was already selected before this
                        // click -- should only ever move the
                        // cursor/highlight; leave opening the detail view to
                        // Enter/double-click.
                        let is_series = cur_item.as_ref().is_some_and(|i| i.item_type == "Series");
                        if !already_selected
                            && !is_series
                            && cur_item.map(|i| i.is_folder).unwrap_or(false)
                        {
                            self.select();
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.click_set_cursor(col, row) {
                    self.open_context_menu_at(col, row);
                }
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
#[path = "input_playback_header_mouse_tests.rs"]
mod playback_header_mouse_tests;

#[cfg(test)]
#[path = "input_power_movie_detail_tests.rs"]
mod power_movie_detail_tests;

#[cfg(test)]
#[path = "input_power_music_track_focus_tests.rs"]
mod power_music_track_focus_tests;

#[cfg(test)]
#[path = "input_power_library_scope_routing_tests.rs"]
mod power_library_scope_routing_tests;
