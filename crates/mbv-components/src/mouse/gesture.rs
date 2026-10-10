//! [`MouseGestureState`] — per-mounted-parent gesture recognition (ADR 0024,
//! design.md D3).
//!
//! Each mounted destination parent owns one `MouseGestureState`. It consumes
//! raw `tuirealm` [`MouseEvent`]s and emits recognized [`MouseGesture`]s. It is
//! keyed by nothing but that parent's own recent events — it is **not** a
//! shared clock, and reintroduces neither the global completed-frame hit map
//! nor the position-keyed cross-surface clock that D16 forbade (design.md D3,
//! "Reconciling with D16").
//!
//! Recognition covers `Click`/`DoubleClick`/`RightClick`, wheel `Scroll`, and
//! left-button drag gestures. Hover-move spam is dropped by the consuming
//! component's first `on()` arm (`MouseEventKind::Moved => return None`),
//! never by this module (design.md D7).
//!
//! ## Chosen intervals
//!
//! * **Double-click window: 400 ms**, exact-position match (legacy standard).
//! * **Wheel step: 3 rows, no throttle.** Every vertical wheel event the parent
//!   forwards is recognized and emitted as `Scroll { delta: ±WHEEL_STEP }`;
//!   none is dropped or coalesced. The uniform step is the policy (design.md
//!   D1): consumers receive the movement already scaled, and no per-component
//!   wheel step may exist. A terminal would need to coalesce at most one
//!   `ScrollUp`/`ScrollDown` per physical notch for the step to behave
//!   uniformly across terminals; the run loop delivers one event per poll
//!   tick, so successive notches arrive as successive gestures.

use std::time::{Duration, Instant};

use ratatui::layout::Position;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

/// Double-click recognition window. See module docs.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);
/// Uniform wheel scroll step, in rows, per wheel event. See module docs.
const WHEEL_STEP: i64 = 3;

/// A recognized pointer gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickModifier {
    None,
    Ctrl,
    Shift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseGesture {
    Click {
        at: Position,
        modifier: ClickModifier,
    },
    DoubleClick(Position),
    RightClick(Position),
    /// Vertical wheel step: `delta` is `-WHEEL_STEP` up, `WHEEL_STEP` down.
    Scroll {
        at: Position,
        delta: i64,
    },
    Drag {
        from: Position,
        to: Position,
    },
    DragEnd,
}

/// Per-parent gesture recognition state. One per mounted parent.
#[derive(Debug, Default)]
pub struct MouseGestureState {
    last_click: Option<(Instant, Position)>,
    drag_anchor: Option<Position>,
}

impl MouseGestureState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one raw mouse event; return the gesture it completes, if any.
    ///
    /// `Moved` and horizontal wheel events are accepted and produce `None`
    /// (they must not panic — design.md D7). A left-button drag reports motion
    /// from its press anchor and ends on release.
    pub fn recognize(&mut self, event: MouseEvent) -> Option<MouseGesture> {
        self.recognize_at(event, Instant::now())
    }

    fn recognize_at(&mut self, event: MouseEvent, now: Instant) -> Option<MouseGesture> {
        let at = Position {
            x: event.column,
            y: event.row,
        };
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let modifier = if event
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                {
                    ClickModifier::Ctrl
                } else if event
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::SHIFT)
                {
                    ClickModifier::Shift
                } else {
                    ClickModifier::None
                };
                let is_double = modifier == ClickModifier::None
                    && self.last_click.is_some_and(|(t, p)| {
                        now.duration_since(t) < DOUBLE_CLICK_WINDOW && p == at
                    });
                self.last_click = (modifier == ClickModifier::None).then_some((now, at));
                self.drag_anchor = (modifier == ClickModifier::None).then_some(at);
                Some(if is_double {
                    MouseGesture::DoubleClick(at)
                } else {
                    MouseGesture::Click { at, modifier }
                })
            }
            MouseEventKind::Down(MouseButton::Right) => Some(MouseGesture::RightClick(at)),
            MouseEventKind::Drag(MouseButton::Left) => self
                .drag_anchor
                .map(|from| MouseGesture::Drag { from, to: at }),
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag_anchor.take().map(|_| MouseGesture::DragEnd)
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta = if matches!(event.kind, MouseEventKind::ScrollUp) {
                    -WHEEL_STEP
                } else {
                    WHEEL_STEP
                };
                Some(MouseGesture::Scroll { at, delta })
            }
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            modifiers: tuirealm::event::KeyModifiers::NONE,
            column: x,
            row: y,
        }
    }

    #[test]
    fn two_clicks_inside_window_and_position_are_a_double_click() {
        let mut s = MouseGestureState::new();
        let t0 = Instant::now();
        assert_eq!(
            s.recognize_at(ev(MouseEventKind::Down(MouseButton::Left), 3, 4), t0),
            Some(MouseGesture::Click {
                at: Position { x: 3, y: 4 },
                modifier: ClickModifier::None
            })
        );
        assert_eq!(
            s.recognize_at(
                ev(MouseEventKind::Down(MouseButton::Left), 3, 4),
                t0 + Duration::from_millis(100)
            ),
            Some(MouseGesture::DoubleClick(Position { x: 3, y: 4 }))
        );
    }

    #[test]
    fn back_to_back_wheel_events_at_the_same_instant_all_recognize() {
        // mouse-input "Rapid wheel events are all recognized": no throttle,
        // every event in a burst emits a `Scroll` with the uniform step.
        let mut s = MouseGestureState::new();
        let t0 = Instant::now();
        assert_eq!(
            s.recognize_at(ev(MouseEventKind::ScrollDown, 1, 1), t0),
            Some(MouseGesture::Scroll {
                at: Position { x: 1, y: 1 },
                delta: 3
            })
        );
        assert_eq!(
            s.recognize_at(
                ev(MouseEventKind::ScrollDown, 1, 1),
                t0 + Duration::from_millis(5)
            ),
            Some(MouseGesture::Scroll {
                at: Position { x: 1, y: 1 },
                delta: 3
            })
        );
        assert_eq!(
            s.recognize_at(
                ev(MouseEventKind::ScrollUp, 1, 1),
                t0 + Duration::from_millis(10)
            ),
            Some(MouseGesture::Scroll {
                at: Position { x: 1, y: 1 },
                delta: -3
            })
        );
    }

    #[test]
    fn press_drag_and_release_reports_the_full_gesture() {
        let mut s = MouseGestureState::new();
        assert_eq!(
            s.recognize(ev(MouseEventKind::Down(MouseButton::Left), 1, 2)),
            Some(MouseGesture::Click {
                at: Position { x: 1, y: 2 },
                modifier: ClickModifier::None
            })
        );
        assert_eq!(
            s.recognize(ev(MouseEventKind::Drag(MouseButton::Left), 4, 5)),
            Some(MouseGesture::Drag {
                from: Position { x: 1, y: 2 },
                to: Position { x: 4, y: 5 }
            })
        );
        assert_eq!(
            s.recognize(ev(MouseEventKind::Up(MouseButton::Left), 4, 5)),
            Some(MouseGesture::DragEnd)
        );
    }
}
