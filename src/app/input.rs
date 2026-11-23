mod browse_dispatch;
mod confirm_keys;
pub(in crate::app) mod gui;
pub(in crate::app) mod key_policy;
pub(in crate::app) use key_policy::{
    OverlayFocus, RemoteControlTarget, RouterOverlayState, RouterPlaybackState,
};
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
use mbv_emby_model::EmbyItem;
impl App {
    pub(in crate::app) fn context_menu_play_state(item: &EmbyItem) -> bool {
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
            + usize::from(self.has_feeds_subscriptions())
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
        // Launch state is written only by the exit snapshot path; these
        // writes are for unrelated preferences.
        let mut v = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        v["ui_volume"] = serde_json::json!(self.ui_volume);
        v["mute_on"] = serde_json::json!(self.mute_on);
        v["pre_mute_volume"] = serde_json::json!(self.pre_mute_volume);
        v["queue_column_width"] = serde_json::json!(self.queue_column_width);
        v["visual_slot_hidden"] = serde_json::json!(self.visual_slot_hidden);
        v["gui_player_auto_hide"] = serde_json::json!(self.gui_player_auto_hide);
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
    fn visual_slot_hidden_prefs_round_trip() {
        let mut app = crate::app::tests::make_app_stub();

        app.visual_slot_hidden = true;
        app.save_prefs();
        assert_eq!(
            App::load_prefs()["visual_slot_hidden"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn gui_player_auto_hide_prefs_round_trip() {
        let mut app = crate::app::tests::make_app_stub();

        app.gui_player_auto_hide = true;
        app.save_prefs();
        assert_eq!(
            App::load_prefs()["gui_player_auto_hide"].as_bool(),
            Some(true)
        );
    }

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
