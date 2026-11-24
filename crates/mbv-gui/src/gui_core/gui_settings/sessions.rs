//! The Sessions section's command (`add-gui-settings-screen` design D5, row
//! 5.5): target selection through the chooser's own path.

use mbv_ui_model::panel_targets::ChooserTargetKey;

use crate::gui_core::GuiCore;

impl GuiCore {
    /// Row 5.5's `selectTarget` arm (design D5): select the playback target
    /// through the chooser's own path, leaving the screen open (the spec's
    /// *Sessions lists and selects the playback targets*).
    pub(super) fn apply_gui_settings_select_target(&mut self, target: &ChooserTargetKey) {
        match target {
            ChooserTargetKey::Local => {
                if self.app.remote_playback_attached() {
                    self.app.detach_to_local();
                }
            }
            ChooserTargetKey::Target(key) => {
                if let Some(target) = mbv_client_core::state::panel_targets::resolve_session_target(
                    self.app.panel_targets(),
                    key,
                ) {
                    self.app.select_panel_target(target);
                }
            }
        }
    }
}
