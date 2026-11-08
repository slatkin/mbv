//! The GUI's saved launch location and its restore (add-gui-library-panel
//! design D9): the file is read once, before the first step; the destination
//! opens when the catalog lands, and the saved collection sets apply once the
//! destination's group list lands, because a saved group is dropped against
//! the groups the library actually holds. The orderly exit's save runs from
//! the core's run loop here too.

use mbv_config::{LaunchPanelFocus, TabIdentity};
use mbv_ui_model::settings::PanelFocus;

use super::CoreThread;

/// How far the saved GUI launch location's restore has come (add-gui-library-
/// panel design D9).
pub(super) enum GuiLaunchRestore {
    /// No saved location: nothing to restore.
    None,
    /// The saved location, waiting for the catalog's presented destination.
    Destination(mbv_config::GuiLaunchState),
    /// The destination is open, waiting for its group list. `destination` is
    /// the identity of the destination the first phase opened, so the saved
    /// collection sets apply only to it, never to a destination a mid-restore
    /// tab switch brought up.
    Collections {
        state: mbv_config::GuiLaunchState,
        destination: TabIdentity,
    },
    /// Restored.
    Done,
}

impl CoreThread {
    /// Advance the saved launch location's restore (design D9): open the saved
    /// destination once the catalog lands, then apply its saved collection
    /// sets once the destination's group list lands, and set the saved Panel
    /// focus on both authorities. The collection sets apply only to the
    /// destination the first phase opened; a mid-restore tab switch abandons
    /// them. The saved Panel focus applies only while the user has not taken
    /// focus in the pre-catalog window. Idempotent, so the catalog and library
    /// drains call it whenever their content can complete a phase.
    pub(crate) fn restore_launch_location(&mut self) {
        let (state, destination) = match &self.launch_restore {
            GuiLaunchRestore::None | GuiLaunchRestore::Done => return,
            GuiLaunchRestore::Destination(state) => {
                let Some(destination) = self.app.open_gui_launch_destination(state) else {
                    return;
                };
                // The open takes Library focus; the saved Panel focus applies
                // over it on both authorities (design D9), unless the user
                // already took focus while the restore was pending — then the
                // taken focus stands.
                if !self.launch_focus_taken {
                    let saved_focus = match state.panel_focus {
                        LaunchPanelFocus::Library => PanelFocus::Library,
                        LaunchPanelFocus::Queue => PanelFocus::Queue,
                    };
                    self.app.set_gui_panel_focus(saved_focus);
                    self.focus.set_panel_focus(saved_focus);
                }
                (state.clone(), destination)
            }
            GuiLaunchRestore::Collections { state, destination } => {
                (state.clone(), destination.clone())
            }
        };
        // The first phase just opened the destination: record its identity
        // so the second phase applies the saved sets only to it (design D9).
        if matches!(self.launch_restore, GuiLaunchRestore::Destination(_)) {
            self.launch_restore = GuiLaunchRestore::Collections {
                state: state.clone(),
                destination: destination.clone(),
            };
        }
        // The destination's group list may already be there (a cached launch),
        // so the second phase runs in the same call; otherwise a library drain
        // retries it when the list lands.
        if self
            .app
            .restore_gui_launch_collections(&state, &destination)
        {
            self.launch_restore = GuiLaunchRestore::Done;
        }
    }

    /// Record that the user moved Panel focus while the launch restore was
    /// pending (design D9): the first phase then skips the saved Panel-focus
    /// re-apply, so the pre-catalog take-focus survives the restore.
    pub(crate) fn note_launch_focus_taken(&mut self) {
        if !matches!(
            self.launch_restore,
            GuiLaunchRestore::None | GuiLaunchRestore::Done
        ) {
            self.launch_focus_taken = true;
        }
    }

    /// Save the GUI's launch location (design D9): the destination, the
    /// selected collections of each destination, and Panel focus, written at
    /// orderly exit and never while the window is open. A failed write is
    /// logged and never blocks the exit.
    pub(crate) fn save_launch_location(&mut self) {
        let state = self.app.gui_launch_state();
        if let Err(error) = mbv_config::save_gui_launch_state(&state) {
            tracing::warn!(
                name: "launch_state.save.failed",
                target: "launch_state",
                error = %error,
                "GUI launch state save failed"
            );
        }
    }
}
