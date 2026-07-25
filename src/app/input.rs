use super::action::{power_album_track_command_for_key, Command};
use super::input_resolver::KeyChord;
use super::settings::settings_total_rows;
use super::PanelFocus;
use super::{
    App, LibSearch, PendingQueueAction, QueueScope, SavePlaylistDialog, SavePlaylistStage,
    POWER_LEFT_WIDTH_DEFAULT, POWER_LEFT_WIDTH_STEP,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::MediaItem;
use mbv_core::player::PlayerCommand;
use std::time::{Duration, Instant};
// The following are unused by input.rs's own code (the code that used them
// moved to input_mouse.rs / input_context_menu.rs in #365 step 2 lane B),
// but input's `#[cfg(test)]` submodules (declared below) rely on
// `use super::*;` to reach them.
#[cfg(test)]
use super::layout::LibraryRowTarget;
#[cfg(test)]
use super::ContextAction;
#[cfg(test)]
use ratatui::layout::Rect;

impl App {
    /// Whether a context menu is currently open. Shared by every
    /// CONTEXT_STACK layer above `context_menu` that must yield to it
    /// (`power_sidebar_toggle_h`, `home_search`, `power_lib_search`, `lib_search`,
    /// `clear_queue_prompt_c`, `queue_column_width`) — see
    /// docs/adr/0002-centralized-input-handling.md phase 6 (#135).
    fn context_menu_open(&self) -> bool {
        self.context_menu.is_some()
    }

    pub(super) fn context_menu_play_state(&self, item: &MediaItem) -> bool {
        if item.is_folder {
            item.unplayed_item_count == 0
        } else {
            item.played
        }
    }

    pub(super) fn context_menu_power_lib_idx(&self) -> Option<usize> {
        if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
            Some(self.library_tab - 1)
        } else {
            None
        }
    }

    pub(super) fn podcast_mark_all_ids(&self, lib_idx: usize) -> Vec<String> {
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

    pub(super) fn podcast_mark_all_unplayed_ids(&self, lib_idx: usize) -> Vec<String> {
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
    pub(super) fn tab_title_widths(&self) -> Vec<u16> {
        let pad: u16 = 2;
        let mut w = vec!["Home".chars().count() as u16 + pad];
        for l in &self.libs {
            w.push(l.library.name.chars().count() as u16 + pad);
        }
        w
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
