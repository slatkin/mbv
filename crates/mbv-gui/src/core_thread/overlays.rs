//! The open overlay's press answers (add-tauri-gui-client row 6.5): whether a
//! press lands outside the overlay, and what a press on the overlay's own
//! buttons executes.
//!
//! The overlay's keyboard rules and its projections live in the Client core's
//! `App` (`src/app/gui_client_state/`); this module is the window's half: it
//! resolves the reported regions against the held overlay and hands the press
//! to the `App` method the button's key hint would run.

use mbv_surfaces::gesture::PointerGesture;
use mbv_ui_msg::{PointerEvent, PointerKind, Position};

use crate::bridge::layout::RegionTarget;
use crate::core_thread::CoreThread;

impl CoreThread {
    /// A press on a confirm button executes the button the index names; any
    /// other gesture on its region does nothing.
    pub(super) fn confirm_button_press(&mut self, gesture: PointerGesture<Position>, index: usize) {
        if matches!(gesture, PointerGesture::Click { .. }) {
            self.app.execute_gui_confirm_button(index);
        }
    }

    /// A press on a daemon-lost button answers the dialog with the recovery
    /// choice the index names; any other gesture on its region does nothing.
    /// Returns whether the Client's run should end, which only the quit
    /// choice asks for.
    pub(super) fn daemon_lost_button_press(
        &mut self,
        gesture: PointerGesture<Position>,
        index: usize,
    ) -> bool {
        matches!(gesture, PointerGesture::Click { .. })
            && self.app.execute_gui_daemon_lost_button(index)
    }

    /// Whether this press lands outside an open overlay's own targets — the
    /// dismissal condition the caller applies.
    pub(super) fn press_outside_open_overlay(&self, event: &PointerEvent) -> bool {
        matches!(event.kind, PointerKind::Press | PointerKind::RightPress)
            && self.app.gui_overlay_is_open()
            && !self
                .regions
                .resolve(event.position)
                .is_some_and(RegionTarget::owns_gui_overlay)
    }
}
