//! The GUI Client's core state (design D4 of `decouple-client-core`): the
//! shared `App` the GUI owns, wrapped by composition.
//!
//! `GuiCore` is defined in the root package but outside `crate::app`, so
//! `2-split-tui-gui-clients` does not need to move it once the GUI modules sit
//! beside it (its D4c). The GUI's `CoreThread` holds one and keeps its
//! `ClientTickHost` implementation; no `App` field becomes `pub` for it (this
//! change's D4), so the owned `App` stays behind this field, visible only
//! inside the root package. The GUI state (`GuiClientState`) sits beside it,
//! so a GUI `impl GuiCore` block reads the core as `self.app.<...>` and its
//! Client state as `self.gui.<...>`.

use crate::app::App;
use crate::app::gui_client_state::GuiClientState;

/// The GUI Client's core state: the `App` it owns and its own Client state.
/// Every GUI `impl` block becomes `impl GuiCore` and reads its core state as
/// `self.app.<...>` (design D4).
pub struct GuiCore {
    /// The owned core. Its `impl GuiCore` blocks live under `crate::app`, so
    /// the field is visible there; every `App` field stays as private as it
    /// was.
    pub(crate) app: App,
    /// The GUI Client's own state (design D4): every field only GUI code
    /// reads or writes. It is its own `ClientLibraryStores` implementor, so
    /// the core's library seam reads it as this Client's stores.
    pub(crate) gui: GuiClientState,
}

impl GuiCore {
    /// The GUI Client's core over an attached `App`.
    #[must_use]
    pub fn new(app: App) -> Self {
        let player_auto_hide =
            crate::app::state::construct::gui_player_auto_hide_from_prefs(&App::load_prefs());
        Self {
            app,
            gui: GuiClientState::new(player_auto_hide),
        }
    }

    /// The shared `App` the GUI Client owns: the tick host's drains, the
    /// router, and the projections read and mutate it.
    #[must_use]
    pub fn app(&self) -> &App {
        &self.app
    }

    /// The shared `App` the GUI Client owns, for mutation.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// The GUI Client's own state, for the Client's test seeds (design D4):
    /// the seeds write the stores a landed fetch would, so a Client's own test
    /// drives the production `GuiCore` and never a second copy of its state.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_state(&mut self) -> &mut GuiClientState {
        &mut self.gui
    }

    /// The GUI Client's own state, for the core seams that keep stores. Only
    /// the root package's own GUI test helpers swap a state in and out of it;
    /// every other caller reaches the state through this type's methods.
    pub(crate) fn gui_state_mut(&mut self) -> &mut GuiClientState {
        &mut self.gui
    }

    /// Take one pending library event and apply it through the GUI Client's
    /// own stores (`decouple-client-core` design D4a (c), row 8.1): the
    /// event's App-owned half runs in the core, and the store work it
    /// collects applies to this Client's `GuiClientState` before the next
    /// event. The GUI's tick host calls this once per event.
    pub fn drain_lib_event(&mut self) -> bool {
        crate::app::drain_lib_event_through(&mut self.app, &mut self.gui)
    }

    /// Adopt the GUI Client's own state a dependent Client's test seeds beside
    /// its `App` (`decouple-client-core` row 8.1): the `App`-seam seeds write
    /// a state that is not this core's, so the core takes that one before the
    /// step reads it, and leaves its own in the same place for the next seed.
    /// A build without the `test` feature keeps no sidecar, so there it
    /// leaves this state alone: the call compiles in every build because the
    /// tick host's step makes it in every build, and the swap itself is the
    /// gated half.
    pub fn adopt_seeded_gui_state_for_test(&mut self) {
        #[cfg(any(test, feature = "test"))]
        {
            let live = std::mem::take(&mut self.gui);
            let seeded = crate::app::gui_state_for_test(|slot| std::mem::replace(slot, live));
            self.gui = seeded;
        }
    }

    /// The shared `App` back out of the wrapper, for a caller that wraps the
    /// `App` it owns for one call and takes it back afterwards.
    #[must_use]
    pub fn into_app(self) -> App {
        self.app
    }

    /// Take the `App` back into the caller's slot, keeping this Client's own
    /// state beside it (the root package's test helpers, task 8.1): the state
    /// a call left installs in the slot's sidecar.
    #[cfg(test)]
    pub(crate) fn into_app_keeping_gui(self, app: &mut App) {
        let Self { app: owned, gui } = self;
        *app = owned;
        crate::app::tests::stub_gui_state(|slot| *slot = gui);
    }
}

impl std::fmt::Debug for GuiCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiCore")
            .field("app", &self.app)
            .field("gui", &self.gui)
            .finish()
    }
}
