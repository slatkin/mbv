//! The GUI's saved launch location and its restore (add-gui-library-panel
//! design D9): the file is read once, before the first step; the destination
//! opens when the catalog lands, and each saved collection set applies through
//! the destination and target it names. A set whose collection catalog has not
//! landed keeps its saved keys; a later effective read drops vanished keys.
//! The orderly exit's save runs from the core's run loop here too.

use mbv::GuiLaunchOpen;
use mbv_config::TabIdentity;

use super::CoreThread;

/// How far the saved GUI launch location's restore has come (add-gui-library-
/// panel design D9). A start with no saved location restores
/// [`mbv_config::GuiLaunchState::first_launch`] through the same phases.
pub(super) enum GuiLaunchRestore {
    /// The saved location, or a fresh launch's first-choice default, waiting
    /// for the catalog's presented destination.
    Destination(mbv_config::GuiLaunchState),
    /// The destination is open, applying the saved entries as their
    /// destination identities resolve. `destination` is the identity of the
    /// destination the first phase opened: a mid-restore tab switch abandons
    /// any unresolved entries, so saved sets never land under a destination
    /// the user moved to.
    Collections {
        state: mbv_config::GuiLaunchState,
        destination: TabIdentity,
    },
    /// Restored.
    Done,
}

impl CoreThread {
    /// Advance the launch location's restore (design D9): open the saved
    /// destination — a start with no saved location opens the first presented
    /// one, through [`mbv_config::GuiLaunchState::first_launch`] — once the
    /// catalog lands, then apply every saved collection set through its own
    /// destination and target, so a destination the first phase did not open
    /// still restores. A mid-restore tab switch abandons unresolved entries.
    /// A saved destination the settled catalog no longer holds opens
    /// nothing: the restore finishes, so a library that appears later cannot
    /// move the tab under the user. Idempotent, so the catalog and library
    /// drains call it whenever their content can complete a phase.
    pub(crate) fn restore_launch_location(&mut self) {
        let (state, destination) = match &self.launch_restore {
            GuiLaunchRestore::Done => return,
            GuiLaunchRestore::Destination(state) => {
                let state = state.clone();
                let destination = match self.app.open_gui_launch_destination(&state) {
                    GuiLaunchOpen::Retry => return,
                    GuiLaunchOpen::Opened(destination) => destination,
                    GuiLaunchOpen::Unmatched => {
                        // The saved destination is gone: the restore finishes
                        // so a late destination never moves the tab.
                        self.launch_restore = GuiLaunchRestore::Done;
                        return;
                    }
                };
                (state, destination)
            }
            GuiLaunchRestore::Collections { state, destination } => {
                (state.clone(), destination.clone())
            }
        };
        // The first phase just opened the destination: record its identity
        // so a mid-restore tab switch abandons the saved sets (design D9).
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

    /// Save the GUI's launch location (design D9): the destination and the
    /// selected collections of each destination, written at orderly exit and
    /// never while the window is open. A failed write is logged and never
    /// blocks the exit.
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
