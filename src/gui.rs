//! The GUI Client's core state (design D4 of `decouple-client-core`): the
//! shared `App` the GUI owns, wrapped by composition.
//!
//! `GuiCore` is defined in the root package but outside `crate::app`, so
//! `2-split-tui-gui-clients` does not need to move it once the GUI modules sit
//! beside it (its D4c). The GUI's `CoreThread` holds one and keeps its
//! `ClientTickHost` implementation; no `App` field becomes `pub` for it (this
//! change's D4), so the owned `App` stays behind this field, visible only
//! inside the root package. Row 8.1 moves the GUI state (`GuiClientState`)
//! beside it.

use crate::app::App;

/// The GUI Client's core state: the `App` it owns. Every GUI `impl` block
/// becomes `impl GuiCore` and reads its core state as `self.app.<...>`
/// (design D4).
pub struct GuiCore {
    /// The owned core. Its `impl GuiCore` blocks live under `crate::app`, so
    /// the field is visible there; every `App` field stays as private as it
    /// was.
    pub(crate) app: App,
}

impl GuiCore {
    /// The GUI Client's core over an attached `App`.
    #[must_use]
    pub fn new(app: App) -> Self {
        Self { app }
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

    /// The shared `App` back out of the wrapper, for a caller that wraps the
    /// `App` it owns for one call and takes it back afterwards.
    #[must_use]
    pub fn into_app(self) -> App {
        self.app
    }
}

impl std::fmt::Debug for GuiCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiCore").field("app", &self.app).finish()
    }
}
