use super::{App, PanelFocus, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::EmbyItem;
// The following are unused by input.rs's own code (the code that used them
// moved to input_mouse.rs / input_context_menu.rs in #365 step 2 lane B, and
// the input_*_keys.rs siblings in #367 lane L2), but input's `#[cfg(test)]`
// submodules (declared below) rely on `use super::*;` to reach them.
#[cfg(test)]
use super::ContextAction;
#[cfg(test)]
use mbv_core::player::PlayerCommand;
#[cfg(test)]
use ratatui::layout::Rect;
#[cfg(test)]
use std::time::Instant;

impl App {
    pub(super) fn context_menu_play_state(&self, item: &EmbyItem) -> bool {
        if item.is_folder {
            item.unplayed_item_count == 0
        } else {
            item.played
        }
    }

    pub(super) fn context_menu_lib_idx(&self) -> Option<usize> {
        if matches!(self.effective_panel_focus(), PanelFocus::Library) {
            // Positive match: the browse dispatch front door has already
            // normalized the destination, so a library-focused context menu
            // can only belong to the explicitly selected Emby library.
            match self.tab {
                TabSelection::EmbyLibrary(lib_idx) => Some(lib_idx),
                _ => None,
            }
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
            self.spawn_cast_discovery();
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
            self.toggle_visualizer();
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn handle_key_view_dispatch(&mut self, key: KeyEvent) -> Option<bool> {
        // Shared globals (q, Tab/BackTab, 1-9, `.`) precede every panel and
        // destination. Historically each browse branch reached these by
        // falling through to the bottom of `handle_queue_key`; hoisting them
        // ahead preserves the same precedence because no earlier library or
        // queue routing arm claims these keys.
        if let Some(quit) = self.handle_global_view_key(key) {
            return Some(quit);
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            self.handle_key_alt(key);
            return Some(false);
        }
        match self.effective_panel_focus() {
            PanelFocus::Queue => Some(self.handle_queue_key(key)),
            PanelFocus::Library => self.handle_key_browse_dispatch(key),
        }
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
        // Use the width `render_tabs` actually budgeted last frame
        // (`layout.tabs_area`), not a fresh guess from `terminal_width` --
        // the tab bar sits in the right column alongside a left panel and
        // has its own padding/reserve math, so re-deriving it here drifted
        // out of sync with what actually renders.
        let tab_w = if self.layout.tabs_area.width > 0 {
            self.layout.tabs_area.width
        } else {
            self.terminal_width
                .saturating_sub(super::TABBAR_LEFT_RESERVE)
        };
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

    /// The currently selected Home pill's persistent identity, or an empty
    /// string when Continue Watching (section 0) is selected.
    pub(super) fn home_section_pref(&self) -> String {
        // Section 0 is Continue Watching and has no `latest` entry; an empty
        // string is its restore sentinel. `saturating_sub(1)` would underflow
        // to 0 and wrongly return `latest[0]`'s key, landing on the next pill.
        if self.home.section == 0 {
            return String::new();
        }
        self.home
            .latest
            .get(self.home.section - 1)
            .map(|(_, source, _, _)| source.pref_key())
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
            "library_tab": self
                .tab
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
            "queue_column_width": self.queue_column_width,
            "home_section": self.home_section_pref(),
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
#[cfg(test)]
#[path = "input_podcast_selection_modal_tests.rs"]
mod podcast_selection_modal_tests;
#[cfg(test)]
#[path = "input_series_music_selection_modal_tests.rs"]
mod series_music_selection_modal_tests;
