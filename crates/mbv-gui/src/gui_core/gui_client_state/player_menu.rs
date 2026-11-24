//! The player panel's context menu (`gui-player-overlay-menu`): the App-side
//! open, entry select, and the shared auto-hide toggle. The menu replaces the
//! player panel's auto-hide checkbox, which the overlay rows painted: the
//! panel's right-click opens the menu, and its single entry toggles the
//! persisted preference the same way the checkbox's press did
//! (`gui-art-player-overlay` design D5).

use super::overlay::{GuiOverlay, GuiPlayerMenu};
use crate::gui_core::GuiCore;

impl GuiCore {
    /// Open the player panel's context menu at the right-click's pointer
    /// anchor (`PlaybackRequest::OpenPlayerMenu`'s App-side effect). A menu
    /// opens over no other overlay: the core dismisses an open overlay on
    /// the same right press before the gesture resolves, so a request that
    /// arrives over one is a stale echo and opens nothing.
    pub(crate) fn open_gui_player_menu(&mut self, x: f32, y: f32) {
        if self.gui.overlay.is_some() {
            return;
        }
        self.gui.overlay = Some(GuiOverlay::PlayerMenu(GuiPlayerMenu { cursor: 0, x, y }));
    }

    /// Activate the player menu's auto-hide entry (the entry press's
    /// App-side effect): the same execution the menu's Enter applies
    /// (`execute_gui_overlay_entry`), gated on the open overlay being the
    /// player menu — a stale entry press while another overlay or none
    /// holds the slot toggles nothing.
    pub(crate) fn select_gui_player_menu_entry(&mut self) {
        if matches!(self.gui.overlay, Some(GuiOverlay::PlayerMenu(_))) {
            self.execute_gui_overlay_entry(0);
        }
    }

    /// Flip the persisted auto-hide preference and write the prefs file: the
    /// one effect the player menu's entry, its Enter key, and the former
    /// checkbox's request all apply.
    pub(crate) fn toggle_gui_player_auto_hide(&mut self) {
        self.gui.player_auto_hide = !self.gui.player_auto_hide;
        // The key is the GUI's own (design D4), so it writes through the
        // Client-key writer rather than the core's whole-key `save_prefs`.
        self.gui.save_player_auto_hide();
    }
}

#[cfg(test)]
mod tests {
    use crate::gui_core::GuiCore;
    use mbv_client_core::App;
    use mbv_client_core::make_app_stub;

    /// The key is the GUI Client's (design D4): the toggle writes
    /// `gui_player_auto_hide` through the Client-key writer, and a fresh
    /// state reads it back at start.
    #[test]
    fn player_auto_hide_prefs_round_trip() {
        let _guard = mbv_client_core::config::TestStateDirGuard::new();
        let mut core = GuiCore::new(make_app_stub());
        core.gui.player_auto_hide = true;

        core.gui.save_player_auto_hide();

        assert_eq!(
            App::load_prefs()["gui_player_auto_hide"].as_bool(),
            Some(true)
        );
        assert!(GuiCore::new(make_app_stub()).gui_player_auto_hide());
    }
}
