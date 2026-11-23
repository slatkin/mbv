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

    /// Write one Client-owned preference key: read the prefs file, change the
    /// key, write it back (design D4). Each Client persists its own keys this
    /// way; the core never writes them from `save_prefs`.
    pub fn save_client_pref(key: &str, value: serde_json::Value) {
        let path = crate::config::prefs_path();
        let mut v = read_prefs_object(&path);
        v[key] = value;
        write_prefs_object(&path, &v);
    }

    pub(in crate::app) fn save_prefs(&self) {
        let path = crate::config::prefs_path();
        // Launch state is written only by the exit snapshot path; these
        // writes are for unrelated preferences.
        let mut v = read_prefs_object(&path);
        v["ui_volume"] = serde_json::json!(self.ui_volume);
        v["mute_on"] = serde_json::json!(self.mute_on);
        v["pre_mute_volume"] = serde_json::json!(self.pre_mute_volume);
        v["queue_column_width"] = serde_json::json!(self.queue_column_width);
        v["visual_slot_hidden"] = serde_json::json!(self.visual_slot_hidden);
        v["gui_player_auto_hide"] = serde_json::json!(self.gui_player_auto_hide);
        write_prefs_object(&path, &v);
    }
}

/// The prefs file as a JSON object, or an empty object when it is missing,
/// unreadable, or not an object.
fn read_prefs_object(path: &std::path::Path) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Serialize and write the prefs object. A failure is silent, as before.
fn write_prefs_object(path: &std::path::Path, value: &serde_json::Value) {
    if let Ok(s) = serde_json::to_string(value) {
        let _ = std::fs::write(path, s);
    }
}

#[cfg(test)]
mod prefs_tests {
    use super::App;

    /// Each prefs test writes its own prefs file, so the tests never race on
    /// one shared file.
    fn stub() -> (App, crate::config::TestStateDirGuard) {
        let guard = crate::config::TestStateDirGuard::new();
        (crate::app::tests::make_app_stub(), guard)
    }

    #[test]
    fn visual_slot_hidden_prefs_round_trip() {
        let (mut app, _guard) = stub();

        app.visual_slot_hidden = true;
        app.save_prefs();
        assert_eq!(
            App::load_prefs()["visual_slot_hidden"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn gui_player_auto_hide_prefs_round_trip() {
        let (mut app, _guard) = stub();

        app.gui_player_auto_hide = true;
        app.save_prefs();
        assert_eq!(
            App::load_prefs()["gui_player_auto_hide"].as_bool(),
            Some(true)
        );
    }

    /// `list_pane_width` is a Client key (design D4): the core's `save_prefs`
    /// leaves it alone, and `save_client_pref` writes it.
    #[test]
    fn list_pane_width_prefs_round_trip_width_and_null() {
        let (app, _guard) = stub();

        App::save_client_pref("list_pane_width", serde_json::json!(42));
        assert_eq!(App::load_prefs()["list_pane_width"].as_u64(), Some(42));

        App::save_client_pref("list_pane_width", serde_json::Value::Null);
        assert!(App::load_prefs()["list_pane_width"].is_null());

        app.save_prefs();
        assert!(App::load_prefs()["list_pane_width"].is_null());
    }

    /// Design D4: `save_client_pref` reads the file, changes its one key, and
    /// writes it back, leaving every other key in place.
    #[test]
    fn save_client_pref_changes_one_key_and_keeps_the_others() {
        let (mut app, _guard) = stub();
        app.visual_slot_hidden = true;
        app.save_prefs();

        App::save_client_pref("a_client_key", serde_json::json!(7));

        let prefs = App::load_prefs();
        assert_eq!(prefs["a_client_key"].as_u64(), Some(7));
        assert_eq!(prefs["visual_slot_hidden"].as_bool(), Some(true));
    }
}
