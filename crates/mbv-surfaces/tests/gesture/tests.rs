//! Public-API tests for the pointer gesture policy (add-gui-front-end row
//! 4.8): the double-click window, the uniform wheel step, the drag anchor,
//! and the Ctrl/Shift selection choice, moved from the TUI adapter's
//! `mouse/gesture.rs` tests and recognized here over the neutral vocabulary
//! with the caller's clock injected.

use std::time::{Duration, Instant};

use mbv_surfaces::gesture::{ClickSelection, PointerGesture, PointerGestureState, WHEEL_STEP};
use mbv_ui_msg::{KeyModifiers, PointerKind, Position};

/// One translated pointer event: the neutral kind, the modifiers held, and
/// the position in the Client's own coordinate space.
type Event = (PointerKind, KeyModifiers, Position);

fn press(x: f32, y: f32, modifiers: KeyModifiers) -> Event {
    (PointerKind::Press, modifiers, Position { x, y })
}

fn bare(x: f32, y: f32) -> Event {
    press(x, y, KeyModifiers::NONE)
}

fn wheel(kind: PointerKind, x: f32, y: f32) -> Event {
    (kind, KeyModifiers::NONE, Position { x, y })
}

/// Feed one translated event to the recognizer, spreading the event's parts
/// onto the policy's arguments.
fn recognize(
    state: &mut PointerGestureState<Position>,
    event: Event,
    now: Instant,
) -> Option<PointerGesture<Position>> {
    let (kind, modifiers, at) = event;
    state.recognize(kind, modifiers, at, now)
}

#[test]
fn two_clicks_inside_window_and_position_are_a_double_click() {
    let mut state = PointerGestureState::new();
    let t0 = Instant::now();
    assert_eq!(
        recognize(&mut state, bare(3.0, 4.0), t0),
        Some(PointerGesture::Click {
            at: Position { x: 3.0, y: 4.0 },
            selection: ClickSelection::Plain,
        })
    );
    assert_eq!(
        recognize(&mut state, bare(3.0, 4.0), t0 + Duration::from_millis(100)),
        Some(PointerGesture::DoubleClick(Position { x: 3.0, y: 4.0 }))
    );
}

#[test]
fn back_to_back_wheel_events_at_the_same_instant_all_recognize() {
    // mouse-input "Rapid wheel events are all recognized": no throttle,
    // every event in a burst emits a `Scroll` with the uniform step.
    let mut state = PointerGestureState::new();
    let t0 = Instant::now();
    let down = wheel(PointerKind::ScrollDown, 1.0, 1.0);
    assert_eq!(
        recognize(&mut state, down, t0),
        Some(PointerGesture::Scroll {
            at: Position { x: 1.0, y: 1.0 },
            delta: WHEEL_STEP,
        })
    );
    assert_eq!(
        recognize(&mut state, down, t0 + Duration::from_millis(5)),
        Some(PointerGesture::Scroll {
            at: Position { x: 1.0, y: 1.0 },
            delta: WHEEL_STEP,
        })
    );
    assert_eq!(
        recognize(
            &mut state,
            wheel(PointerKind::ScrollUp, 1.0, 1.0),
            t0 + Duration::from_millis(10)
        ),
        Some(PointerGesture::Scroll {
            at: Position { x: 1.0, y: 1.0 },
            delta: -WHEEL_STEP,
        })
    );
}

#[test]
fn press_drag_and_release_reports_the_full_gesture() {
    let mut state = PointerGestureState::new();
    let t0 = Instant::now();
    assert_eq!(
        recognize(&mut state, bare(1.0, 2.0), t0),
        Some(PointerGesture::Click {
            at: Position { x: 1.0, y: 2.0 },
            selection: ClickSelection::Plain,
        })
    );
    assert_eq!(
        recognize(&mut state, wheel(PointerKind::Drag, 4.0, 5.0), t0),
        Some(PointerGesture::Drag {
            from: Position { x: 1.0, y: 2.0 },
            to: Position { x: 4.0, y: 5.0 },
        })
    );
    assert_eq!(
        recognize(&mut state, wheel(PointerKind::Release, 4.0, 5.0), t0),
        Some(PointerGesture::DragEnd)
    );
}

/// The modifier choice is part of the gesture policy: the recognizer derives
/// the selection from the event's modifiers, so both Clients toggle and
/// range-select the same way (add-gui-front-end row 4.8).
#[rstest::rstest]
#[case::ctrl_toggles(KeyModifiers::CONTROL, ClickSelection::Toggle)]
#[case::shift_extends(KeyModifiers::SHIFT, ClickSelection::Range)]
#[case::bare_click_selects(KeyModifiers::NONE, ClickSelection::Plain)]
fn the_held_modifiers_choose_the_click_selection(
    #[case] modifiers: KeyModifiers,
    #[case] expected: ClickSelection,
) {
    let mut state = PointerGestureState::new();
    assert_eq!(
        recognize(&mut state, press(2.0, 3.0, modifiers), Instant::now()),
        Some(PointerGesture::Click {
            at: Position { x: 2.0, y: 3.0 },
            selection: expected,
        })
    );
}
