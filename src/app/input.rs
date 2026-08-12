use super::{App, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::EmbyItem;
// The following are unused by input.rs's own code (the code that used them
// moved to input_mouse.rs / input_context_menu.rs in #365 step 2 lane B, and
// the input_*_keys.rs siblings in #367 lane L2), but input's `#[cfg(test)]`
// submodules (declared below) rely on `use super::*;` to reach them.
#[cfg(test)]
use super::layout::LibraryRowTarget;
#[cfg(test)]
use super::ContextAction;
#[cfg(test)]
use mbv_core::player::PlayerCommand;
#[cfg(test)]
use ratatui::layout::Rect;
#[cfg(test)]
use std::time::{Duration, Instant};

impl App {
    /// Whether a context menu is currently open. Shared by every
    /// CONTEXT_STACK layer above it that must yield to it
    /// (`panel_mode_cycle_x`, `search_sidebar`, `lib_search`, `clear_queue_prompt_c`,
    /// `queue_column_width`) — see
    /// docs/adr/0002-centralized-input-handling.md phase 6 (#135).
    pub(super) fn context_menu_open(&self) -> bool {
        self.context_menu.is_some()
    }

    pub(super) fn context_menu_play_state(&self, item: &EmbyItem) -> bool {
        if item.is_folder {
            item.unplayed_item_count == 0
        } else {
            item.played
        }
    }

    pub(super) fn context_menu_lib_idx(&self) -> Option<usize> {
        if matches!(self.panel_focus, PanelFocus::Library) {
            self.tab.library_index()
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
            + self.audiobookshelf_libraries.len()
            + if self.has_feeds_subscriptions() { 1 } else { 0 }
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
        for entry in super::input_resolver::CONTEXT_STACK {
            if let Some(quit) = (entry.handler)(self, key) {
                return quit;
            }
        }
        false
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
        // Terminals disagree on how they send Ctrl+/: most send the ASCII
        // unit-separator, which crossterm surfaces as either `Char('/')` or
        // `Char('_')` with CONTROL depending on the terminal and whether the
        // kitty keyboard protocol is active. Match both encodings.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('/') | KeyCode::Char('_'))
        {
            if self.search_sidebar.is_some() {
                return Some(false);
            }
            self.open_search_sidebar();
            return Some(false);
        }
        None
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

    pub(super) fn handle_key_visualizer(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code == KeyCode::Char('v') && key.modifiers.is_empty() {
            if self.connected_session_id.is_some() {
                return Some(false);
            }
            self.toggle_visualizer();
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn handle_key_view_dispatch(&mut self, key: KeyEvent) -> Option<bool> {
        // `handle_queue_key` (despite its name -- a holdover from when this
        // was Standard's Queue-tab handler) is the view's single left-column
        // dispatch: it branches internally on `panel_focus`/`library_tab`
        // to route to Home (`handle_cw_key`), a library
        // (`handle_lib_key`), or genuine queue-cursor movement.
        Some(self.handle_queue_key(key))
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
        let pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        if pos < self.tab_scroll {
            self.tab_scroll = pos;
            return;
        }
        let tab_w = self
            .terminal_width
            .saturating_sub(super::TABBAR_LEFT_RESERVE + super::TABBAR_RIGHT_RESERVE);
        loop {
            let (_, end) = self.visible_tab_range(tab_w);
            if pos < end {
                break;
            }
            self.tab_scroll += 1;
        }
    }

    /// Tab-bar title widths: Home + one per library + Feeds when present.
    pub(super) fn tab_title_widths(&self) -> Vec<u16> {
        let pad: u16 = 2;
        let mut w = vec!["Home".chars().count() as u16 + pad];
        for l in &self.libs {
            w.push(l.library.name.chars().count() as u16 + pad);
        }
        for l in &self.audiobookshelf_libraries {
            w.push(l.name.chars().count() as u16 + pad);
        }
        if self.has_feeds_subscriptions() {
            w.push("Feeds".chars().count() as u16 + pad);
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
            "visualizer_enabled": self.visualizer_enabled,
            "panel_focus": self.panel_focus.pref_value(),
            "library_tab": self
                .tab
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
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
#[path = "input_movie_detail_tests.rs"]
mod movie_detail_tests;

#[cfg(test)]
#[path = "input_music_track_focus_tests.rs"]
mod music_track_focus_tests;
#[cfg(test)]
#[path = "input_music_track_navigation_tests.rs"]
mod music_track_navigation_tests;
#[cfg(test)]
#[path = "input_music_track_scope_tests.rs"]
mod music_track_scope_tests;
#[cfg(test)]
#[path = "input_music_track_test_support.rs"]
mod music_track_test_support;

#[cfg(test)]
#[path = "input_library_scope_routing_tests.rs"]
mod library_scope_routing_tests;
