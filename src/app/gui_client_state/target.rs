//! The playback-target chooser's App-side state (add-tauri-gui-client row
//! 6.4): the open toggle and the stable-key target resolution a chooser's
//! Enter and a chooser row's press share.

use mbv_ui_model::panel_targets::{ChooserTargetKey, SessionTargetKey};

use super::overlay::{GuiOverlay, TargetChooserState};
use crate::app::App;
use crate::app::state::panel_targets::resolve_session_target;

impl App {
    /// Toggle the playback-target chooser (add-tauri-gui-client row 6.4):
    /// the canvas header's target press and the GUI-only `r` chord both
    /// open it, the same gesture closes it — the toggle the TUI's F3
    /// Sessions sidebar applies (`request_sidebar_toggle`). The chooser
    /// opens even with no remote targets to list, as the sidebar does. The
    /// cursor starts on the Owner's attached output, as the popover's
    /// starts on the chosen track.
    pub(in crate::app) fn toggle_gui_target_chooser(&mut self) {
        self.gui_overlay = if let Some(GuiOverlay::TargetChooser(_)) = &self.gui_overlay {
            None
        } else {
            let cursor = self
                .playback_target_choices()
                .iter()
                .position(|choice| choice.is_active)
                .unwrap_or(0);
            Some(GuiOverlay::TargetChooser(TargetChooserState { cursor }))
        };
    }

    /// Select a chooser row's target. The local row returns the playback to this
    /// machine, and does nothing while it already plays here, so a press on the
    /// local output raises no toast. A remote row resolves as `select_target_for_key`
    /// does.
    pub(in crate::app) fn select_chooser_target(&mut self, key: &ChooserTargetKey) -> bool {
        match key {
            ChooserTargetKey::Local => {
                if self.remote_playback_attached() {
                    self.detach_to_local();
                }
                true
            }
            ChooserTargetKey::Target(key) => self.select_target_for_key(key),
        }
    }

    /// Select the playback target a stable `SessionTargetKey` names,
    /// resolved against the current target snapshot: the dispatch a
    /// chooser's Enter and a chooser row's press share. A target the
    /// snapshot no longer holds selects nothing. Returns whether one did.
    pub(in crate::app) fn select_target_for_key(&mut self, key: &SessionTargetKey) -> bool {
        match resolve_session_target(&self.panel_targets, key) {
            Some(target) => {
                self.select_panel_target(target);
                true
            }
            None => false,
        }
    }
}
