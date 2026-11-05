//! The TUI's gesture-translation edge over the Client core's pointer gesture
//! policy (`add-gui-front-end` row 4.8). The recognition rules — the
//! double-click window, the uniform wheel step, the drag anchor, and the
//! Ctrl/Shift selection choice — live once in `mbv_surfaces::gesture`; this
//! module only translates: `neutral_pointer` fills the neutral kind and
//! modifiers, the Client core recognizes the gesture over the TUI's painted
//! cells, and the adapter resolves it against its own geometry.
//!
//! Each mounted destination parent owns one [`MouseGestureState`]. It is
//! keyed by nothing but that parent's own recent events — it is **not** a
//! shared clock, and reintroduces neither the global completed-frame hit map
//! nor the position-keyed cross-surface clock that ADR 0024's design D16
//! forbade.

use std::time::Instant;

use ratatui::layout::Position;
use tuirealm::event::MouseEvent;

use mbv_surfaces::gesture::{PointerGesture, PointerGestureState};

use crate::root::neutral_pointer;

/// The TUI's gesture recognition state: the Client core's
/// [`PointerGestureState`] over the TUI's painted cells, fed by
/// [`MouseGestureState::recognize`].
///
/// The TUI's logical units are whole cells (one cell is one logical unit,
/// design D6), so the core recognizes over the same numbers the adapters'
/// geometry resolves against; no coordinate conversion happens on this path.
#[derive(Debug, Default)]
pub struct MouseGestureState {
    core: PointerGestureState<Position>,
}

impl MouseGestureState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one raw terminal mouse event; return the gesture it completes, if
    /// any. The translation edge drops the kinds the neutral vocabulary
    /// cannot represent (the middle button, a right-button drag or release,
    /// horizontal wheel) — the events the core's recognizer would decline
    /// without touching state.
    #[must_use]
    pub fn recognize(&mut self, event: MouseEvent) -> Option<PointerGesture<Position>> {
        let pointer = neutral_pointer(event)?;
        let at = Position {
            x: event.column,
            y: event.row,
        };
        self.core
            .recognize(pointer.kind, pointer.modifiers, at, Instant::now())
    }
}

#[cfg(any(test, feature = "test"))]
impl MouseGestureState {
    /// Test seam: forget the last click so the next event is not promoted to
    /// a double-click.
    pub fn reset_for_test(&mut self) {
        *self = Self::default();
    }
}
