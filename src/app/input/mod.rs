mod browse_dispatch;
mod confirm_keys;
mod key_policy;
mod lib_keys;
mod playlist_keys;
mod queue_keys;
pub(in crate::app) mod resolver;
pub(in crate::app) mod router;
mod search_sidebar_keys;

#[cfg(test)]
mod music_track_scope_tests;
#[cfg(test)]
mod music_track_test_support;

use super::{App, PanelFocus, TabSelection};
use mbv_core::api::EmbyItem;
// The following are unused by input.rs's own code (the code that used them
// moved to input_mouse.rs / input_context_menu.rs in #365 step 2 lane B, and
// the input_*_keys.rs siblings in #367 lane L2), but input's `#[cfg(test)]`
// submodules (declared below) rely on `use super::*;` to reach them.
#[cfg(test)]
use super::ContextAction;

impl App {
    pub(in crate::app) fn context_menu_play_state(&self, item: &EmbyItem) -> bool {
        if item.is_folder {
            item.unplayed_item_count == 0
        } else {
            item.played
        }
    }

    pub(in crate::app) fn context_menu_lib_idx(&self) -> Option<usize> {
        if matches!(self.effective_panel_focus(), PanelFocus::Library) {
            match self.tab {
                TabSelection::EmbyLibrary(lib_idx) => Some(lib_idx),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Home + one tab per library (no Queue tab -- the queue is the
    /// always-visible left column, not a tab).
    pub(in crate::app) fn tab_count(&self) -> usize {
        1 + self.libs.len()
            + self.audiobookshelf_libraries.len()
            + if self.has_feeds_subscriptions() { 1 } else { 0 }
    }

    pub(in crate::app) fn visible_tab_range(&self, avail_w: u16) -> (usize, usize) {
        // The shared tab-window computation (task 2.1): the same one the tab
        // bar painter resolves the painted window and hit regions from, so
        // the keyboard scroll anchor and the painted bar cannot drift.
        crate::app::render::components::chrome_tabs::visible_tab_range(
            &self.tab_title_widths(),
            self.tab_scroll,
            avail_w,
        )
    }

    pub(in crate::app) fn ensure_tab_visible(&mut self) {
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
                .saturating_sub(crate::app::TABBAR_LEFT_RESERVE)
        };
        loop {
            let (_, end) = self.visible_tab_range(tab_w);
            if pos < end {
                break;
            }
            self.tab_scroll += 1;
        }
    }

    /// Tab-bar title widths: Continue + one per library + Feeds when present.
    pub(in crate::app) fn tab_title_widths(&self) -> Vec<u16> {
        let pad: u16 = 2;
        let mut w = vec![
            crate::app::ui_util::continue_tab_title(self.use_nerd_fonts)
                .chars()
                .count() as u16
                + pad,
        ];
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

    pub(in crate::app) fn load_prefs() -> serde_json::Value {
        let path = crate::config::prefs_path();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_default()
    }

    pub(in crate::app) fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        // Keep legacy launch keys readable for the one-time migration, but do
        // not update them during the session. Launch state is written only by
        // the exit snapshot path; these writes are for unrelated preferences.
        let mut v = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        v["ui_volume"] = serde_json::json!(self.ui_volume);
        v["mute_on"] = serde_json::json!(self.mute_on);
        v["pre_mute_volume"] = serde_json::json!(self.pre_mute_volume);
        v["queue_column_width"] = serde_json::json!(self.queue_column_width);
        v["list_pane_width"] = serde_json::json!(self.list_pane_width);
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = std::fs::write(path, s);
        }
    }
}

#[cfg(test)]
mod prefs_tests {
    use super::App;

    #[test]
    fn list_pane_width_prefs_round_trip_width_and_null() {
        let mut app = crate::app::tests::make_app_stub();

        app.list_pane_width = Some(42);
        app.save_prefs();
        assert_eq!(App::load_prefs()["list_pane_width"].as_u64(), Some(42));

        app.list_pane_width = None;
        app.save_prefs();
        assert!(App::load_prefs()["list_pane_width"].is_null());
    }
}
