//! The pointer gesture policy (`add-gui-front-end` row 4.8): the recognition
//! rules the TUI Client and the GUI Client share, held one time in the
//! Client core. A Client translates its raw pointer input into the neutral
//! [`PointerEvent`] at its own edge and feeds the recognizer; the adapter
//! keeps only geometry and translation.
//!
//! Each Client instantiates [`PointerGestureState`] over its own coordinate
//! space `P` — the TUI over its painted cells, the GUI over its egui points
//! — because the positions a gesture carries are the positions the Client's
//! own geometry resolves against. The recognizer never converts coordinates.
//!
//! ## Chosen intervals
//!
//! * **Double-click window: 400 ms**, same-place match (legacy standard).
//!   The same-place rule is the coordinate type's own (`SamePlace`): the
//!   TUI's painted cells compare exactly, and the GUI's `f32` logical
//!   points compare within [`DOUBLE_CLICK_SLOP`], because a toolkit pointer
//!   rarely repeats a position exactly.
//! * **Wheel step: 3 rows, no throttle.** Every wheel event the Client
//!   translates is recognized and emitted as `Scroll { delta: ±WHEEL_STEP }`;
//!   none is dropped or coalesced. The uniform step is the policy: consumers
//!   receive the movement already scaled, and no per-component wheel step may
//!   exist. Successive notches arrive as successive gestures.
//!
//! Hover moves are dropped here: they recognize as `None` and touch no
//! state. A drag reports motion from its press anchor and ends on release.

use std::time::{Duration, Instant};

use mbv_ui_msg::{KeyModifiers, PointerKind, Position};

/// Double-click recognition window. See module docs.
pub const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);
/// Uniform wheel scroll step, in rows, per wheel event. See module docs.
pub const WHEEL_STEP: i64 = 3;
/// The double-click slop radius, in logical units, for the `f32` logical
/// points the GUI Client recognizes over. The rule lives here so the GUI
/// does not choose its own tolerance; the TUI's painted cells compare
/// exactly and never see it.
pub const DOUBLE_CLICK_SLOP: f32 = 4.0;

/// Whether two positions name the same place for the double-click rule. Each
/// Client's coordinate type decides: the TUI's painted cells name the same
/// place only when they are equal, and the GUI's `f32` logical points name
/// the same place within [`DOUBLE_CLICK_SLOP`].
pub trait SamePlace {
    /// Whether `self` and `other` name the same place.
    #[must_use]
    fn same_place(&self, other: &Self) -> bool;
}

impl SamePlace for Position {
    fn same_place(&self, other: &Self) -> bool {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy <= DOUBLE_CLICK_SLOP * DOUBLE_CLICK_SLOP
    }
}

/// The selection a click requests. The choice is part of the gesture policy:
/// the recognizer derives it from the event's modifiers, so both Clients
/// toggle and range-select the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickSelection {
    /// A bare click: move the selection to the clicked row.
    Plain,
    /// A Ctrl-held click: toggle the clicked row's mark.
    Toggle,
    /// A Shift-held click: extend the selection to the clicked row.
    Range,
}

impl ClickSelection {
    /// The selection a click with `modifiers` held requests: Ctrl toggles,
    /// Shift extends, and a bare click selects. Ctrl wins when both are held.
    #[must_use]
    pub fn of(modifiers: KeyModifiers) -> Self {
        if modifiers.contains(KeyModifiers::CONTROL) {
            Self::Toggle
        } else if modifiers.contains(KeyModifiers::SHIFT) {
            Self::Range
        } else {
            Self::Plain
        }
    }
}

/// A recognized pointer gesture, in the Client's own coordinate space `P`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerGesture<P> {
    Click {
        at: P,
        selection: ClickSelection,
    },
    DoubleClick(P),
    RightClick(P),
    /// Vertical wheel step: `delta` is `-WHEEL_STEP` up, `WHEEL_STEP` down.
    Scroll {
        at: P,
        delta: i64,
    },
    Drag {
        from: P,
        to: P,
    },
    DragEnd,
}

/// Per-Client gesture recognition state. One per mounted parent. It is keyed
/// by nothing but that parent's own recent events — it is **not** a shared
/// clock, and reintroduces neither the global completed-frame hit map nor the
/// position-keyed cross-surface clock that ADR 0024's design D16 forbade.
#[derive(Debug)]
pub struct PointerGestureState<P> {
    last_click: Option<(Instant, P)>,
    drag_anchor: Option<P>,
}

impl<P> Default for PointerGestureState<P> {
    fn default() -> Self {
        Self {
            last_click: None,
            drag_anchor: None,
        }
    }
}

impl<P: Copy + SamePlace> PointerGestureState<P> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one translated pointer event at `at`, the Client's own coordinate
    /// for it; return the gesture it completes, if any. `now` is the caller's
    /// clock, so a test injects time instead of sleeping.
    ///
    /// `Move` and the kinds the neutral vocabulary cannot represent reach no
    /// caller that recognizes (the translation edge drops them first), and a
    /// `Move` that does arrive produces `None` without touching state. A
    /// left-button drag reports motion from its press anchor and ends on
    /// release. Only a bare click (`Plain`) arms the double-click pair and
    /// the drag anchor; a modified press recognizes as its own click.
    pub fn recognize(
        &mut self,
        kind: PointerKind,
        modifiers: KeyModifiers,
        at: P,
        now: Instant,
    ) -> Option<PointerGesture<P>> {
        let selection = ClickSelection::of(modifiers);
        match kind {
            PointerKind::Press => {
                let is_double = selection == ClickSelection::Plain
                    && self.last_click.is_some_and(|(t, p)| {
                        now.duration_since(t) < DOUBLE_CLICK_WINDOW && p.same_place(&at)
                    });
                self.last_click = (selection == ClickSelection::Plain).then_some((now, at));
                self.drag_anchor = (selection == ClickSelection::Plain).then_some(at);
                Some(if is_double {
                    PointerGesture::DoubleClick(at)
                } else {
                    PointerGesture::Click { at, selection }
                })
            }
            PointerKind::RightPress => Some(PointerGesture::RightClick(at)),
            PointerKind::Drag => self
                .drag_anchor
                .map(|from| PointerGesture::Drag { from, to: at }),
            PointerKind::Release => self.drag_anchor.take().map(|_| PointerGesture::DragEnd),
            PointerKind::ScrollUp => Some(PointerGesture::Scroll {
                at,
                delta: -WHEEL_STEP,
            }),
            PointerKind::ScrollDown => Some(PointerGesture::Scroll {
                at,
                delta: WHEEL_STEP,
            }),
            PointerKind::Move => None,
        }
    }
}
