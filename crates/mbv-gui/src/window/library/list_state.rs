//! The state every filtered Library destination shares (`add-gui-library-panel`
//! row 7C.1, the port of `lib/library/pill-list.ts`, issue #1009): what the
//! window content owns and the core never reads — the `/` filter's text. The
//! scroll offset and the measured viewport live in the list's scroll store
//! ([`super::scroll`], design D9), which the caller names. The core owns the
//! selection and the selected row (the collections frame's `all`, `entries`,
//! and the list's `selected`), so nothing here mirrors it: a press sends the
//! collection command, and the frame carries the choice back. The Hero overlay
//! stays in [`super::state`]'s shared store.
//!
//! The destinations keep no keyboard cursor: their keys act on the row the
//! pointer marks (`gui-pointer-target-interaction` D2, D6), so this state holds
//! no cursor either. Each destination's module keeps its own doc, its own
//! command shapes, its own names, and its own extras — Emby's letter rail —
//! over this one state, so a filter change lands the same way on every one of
//! them.

use dioxus_native::prelude::*;

use super::state::LibraryState;

/// A filtered destination's interaction state (design D12): what the window
/// content owns and the core never reads.
#[derive(Debug, Clone, PartialEq)]
pub struct ListUiState {
    /// The `/` filter's text: the list paints only the rows whose title
    /// contains it, without case (design D7's filter rule).
    pub filter: String,
}

impl ListUiState {
    /// The state a freshly mounted list starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self {
            filter: String::new(),
        }
    }
}

impl LibraryState {
    /// Accept a destination list's filter with Enter: the field closes and
    /// the list keeps the text (design D3, step 2).
    pub fn accept_destination_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
    }
}
