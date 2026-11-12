//! Reset UI State in the GUI (`add-gui-settings-screen` design D10, row 5.6):
//! the App-owned steps of the reset — Home, the panel toggles, the overlays,
//! the saved launch location, and the frame's reset counter. The `CoreThread`
//! runs the steps in the design's order; its own parts (the Library store, the
//! Queue Surface state) sit beside these.
//!
//! The reset never touches media, settings, key bindings, Service setup, the
//! queue's contents, playback, or the TUI Client's saved state: only the GUI's
//! presentation state (the spec's *Reset UI State resets the GUI's
//! presentation*).

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;

impl App {
    /// Design D10's App-owned steps: abandon the launch intent and select Home
    /// (step 1), clear every destination's kept collection and pick in the GUI
    /// Library store (step 2), return the collapse and fold toggles to off
    /// (step 3's App half), and close the overlays and the Settings screen
    /// (step 4). The `CoreThread`'s wrapper runs these with its own parts — the
    /// saved launch restore, the Queue Surface state, the launch-location file,
    /// and the reset counter.
    pub fn reset_gui_ui_state(&mut self) {
        // Step 1: Home, through the settled-tab path. `apply_tab_position`
        // marks the TUI's launch intent done on the way, so a late catalog
        // arrival cannot re-settle a destination the reset discarded.
        self.set_library_tab(0);
        // Step 2: the GUI Library's selections. The fetched content stays —
        // the reset is presentation only, and no refetch follows it, as the
        // TUI's reset keeps its fetched content too.
        self.gui_library_groups.clear();
        self.gui_library_picks.clear();
        self.gui_home_selection = None;
        self.gui_tv_shows.clear();
        self.gui_emby_libs.clear();
        self.gui_abs_libs.clear();
        self.gui_podcast_libs.clear();
        // Step 3's App half: the canvas panel's two toggles. The Queue
        // Surface state's cursor, marks, and viewport belong to the core.
        self.panel_collapsed = false;
        self.queue_folded = false;
        // Step 4: the GUI's blocking overlay (a confirmation or the
        // daemon-lost dialog) and the Settings screen. A pending shell
        // request the GUI has not drained goes with them.
        self.gui_overlay = None;
        self.pending_overlay = None;
        self.close_gui_settings();
    }

    /// Design D10's step 5: delete the GUI's saved launch location, so the
    /// next start's restore finds nothing and opens its first-choice defaults.
    /// A failure is reported as an error toast and never stops the reset.
    pub fn clear_gui_launch_state(&mut self) {
        match mbv_config::clear_gui_launch_state() {
            Ok(()) => self.flash("UI state reset".to_string(), ToastSeverity::Success),
            Err(error) => self.flash_error(format!(
                "saved UI state could not be fully cleared: {error}"
            )),
        }
    }

    /// Design D10's step 6: the frame's reset counter, which the window
    /// content watches to reset its own interaction stores.
    pub fn bump_gui_ui_reset_epoch(&mut self) {
        self.gui_ui_reset_epoch = self.gui_ui_reset_epoch.wrapping_add(1);
    }
}
