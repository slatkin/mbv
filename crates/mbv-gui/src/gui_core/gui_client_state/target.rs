//! The playback-target chooser's App-side state (add-tauri-gui-client row
//! 6.4): the open toggle and the stable-key target resolution a chooser's
//! Enter and a chooser row's press share. The resolution itself is core
//! target behaviour and lives in `state::playback_target`.

use mbv_ui_model::panel_targets::{ChooserTargetKey, SessionTargetKey};

use super::overlay::{GuiOverlay, TargetChooserState};
use crate::gui_core::GuiCore;
use mbv_client_core::state::panel_targets::resolve_session_target;

impl GuiCore {
    /// Select a row's playback target by its stable key, as the playback-target
    /// chooser's Enter and a chooser row's press both do. The local row returns
    /// the playback to this machine and does nothing while it already plays
    /// here, so a press on the local output raises no toast. A remote row
    /// resolves as `select_target_for_key` does.
    pub(crate) fn select_chooser_target(&mut self, key: &ChooserTargetKey) -> bool {
        match key {
            ChooserTargetKey::Local => {
                if self.app.remote_playback_attached() {
                    self.app.detach_to_local();
                }
                true
            }
            ChooserTargetKey::Target(key) => self.select_target_for_key(key),
        }
    }

    /// Toggle the playback-target chooser (add-tauri-gui-client row 6.4):
    /// the canvas header's target press and the GUI-only `r` chord both
    /// open it, the same gesture closes it — the toggle the TUI's F3
    /// Sessions sidebar applies (`request_sidebar_toggle`). The chooser
    /// opens even with no remote targets to list, as the sidebar does. The
    /// cursor starts on the Owner's attached output, as the popover's
    /// starts on the chosen track. Opening refreshes the chooser's list — the
    /// session list and the cast receivers — exactly as the TUI's F3 open
    /// does; closing refreshes nothing.
    pub(crate) fn toggle_gui_target_chooser(&mut self) {
        if let Some(GuiOverlay::TargetChooser(_)) = &self.gui.overlay {
            self.gui.overlay = None;
            return;
        }
        let cursor = self
            .app
            .playback_target_choices()
            .iter()
            .position(|choice| choice.is_active)
            .unwrap_or(0);
        self.gui.overlay = Some(GuiOverlay::TargetChooser(TargetChooserState { cursor }));
        self.refresh_playback_targets();
    }

    /// Refresh the playback-target list a chooser and the Settings Sessions
    /// section paint: the session load and the cast browse, through the
    /// startup entry that retries until an Emby client exists. A refresh while
    /// either half runs starts no second one (each half holds its in-flight
    /// guard), and an open chooser stays open.
    pub(crate) fn refresh_playback_targets(&mut self) {
        self.app.request_playback_target_refresh();
    }

    /// Whether a playback-target refresh is in flight, for the GUI's busy
    /// paint. The refresh half the list needs is the cast browse even with no
    /// Emby client, so a cast browse counts on its own.
    #[must_use]
    pub fn gui_playback_target_refresh_busy(&self) -> bool {
        self.app.playback_target_discovery_busy()
    }

    /// Select the playback target a stable `SessionTargetKey` names,
    /// resolved against the current target snapshot: the dispatch a
    /// chooser's Enter and a chooser row's press share. A target the
    /// snapshot no longer holds selects nothing. Returns whether one did.
    pub(crate) fn select_target_for_key(&mut self, key: &SessionTargetKey) -> bool {
        match resolve_session_target(self.app.panel_targets(), key) {
            Some(target) => {
                self.app.select_panel_target(target);
                true
            }
            None => false,
        }
    }
}
