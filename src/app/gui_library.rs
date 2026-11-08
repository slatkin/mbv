//! The GUI Library panel's Client-core entries (add-gui-library-panel design
//! D2): the public `App` steps the GUI's `library` command applies, beside
//! `gui_client_state.rs`. The window content sends semantic intent, and each
//! entry calls the existing `App` action for it. The TUI Client never calls
//! them.

use mbv_ui_model::settings::PanelFocus;

use crate::app::App;

impl App {
    /// Set Panel focus for the GUI Library panel (add-gui-library-panel design
    /// D4): the take-focus command's effect, sent by a press inside the panel
    /// and by a Tab across the panel boundary. The core thread sets its
    /// `FocusStack` with the same value, so the frame's focus and the App's
    /// key gates cannot disagree.
    pub fn set_gui_panel_focus(&mut self, focus: PanelFocus) {
        self.set_panel_focus(focus);
    }
}
