use super::{App, PanelFocus, TabSelection};
use mbv_core::api::EmbyItem;
// The following are unused by input.rs's own code (the code that used them
// moved to input_mouse.rs / input_context_menu.rs in #365 step 2 lane B, and
// the input_*_keys.rs siblings in #367 lane L2), but input's `#[cfg(test)]`
// submodules (declared below) rely on `use super::*;` to reach them.
#[cfg(test)]
use super::ContextAction;

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
        // Residual A: derive the tab-strip width from the shared arrangement
        // primitive instead of reading the painted `layout.tabs_area`.
        let area = ratatui::layout::Rect::new(0, 0, self.terminal_width, self.terminal_height);
        let chrome = self.compute_chrome_geometry(area);
        let tab_w = if chrome.right_visible {
            crate::app::render::arrangements::chrome::tab_strip_text_width(
                chrome.tab_bar_area.width,
            )
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

    pub(super) fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        let existing_home_section = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|prefs| prefs.get("home_section").cloned());
        let mut v = serde_json::json!({
            "ui_volume": self.ui_volume,
            "mute_on": self.mute_on,
            "pre_mute_volume": self.pre_mute_volume,
            "panel_focus": self.panel_focus.pref_value(),
            "library_tab": self
                .tab
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
            "queue_column_width": self.queue_column_width,
        });
        if let Some(home_section) = existing_home_section {
            v["home_section"] = home_section;
        }
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }
}

#[cfg(test)]
#[path = "input_music_track_scope_tests.rs"]
mod music_track_scope_tests;
#[cfg(test)]
#[path = "input_music_track_test_support.rs"]
mod music_track_test_support;
