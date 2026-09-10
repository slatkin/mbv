use super::msg::{Msg, ShellRequest};
use super::wide_hero_boundary::WideHeroBoundaryComponent;
use super::UserEvent;
use ratatui::layout::Rect;
use tuirealm::component::AppComponent;
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn mouse(kind: MouseEventKind, column: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row: 2,
    })
}

/// A 200-column content area whose default split puts the list pane at
/// columns `0..80` and the two-column gap at `80..82`. `pane_origin_x == 0`,
/// so the resolved width is the pointer column; the valid range is `40..=158`.
fn boundary() -> WideHeroBoundaryComponent {
    let mut component = WideHeroBoundaryComponent::new();
    component.sync(Rect::new(80, 0, 2, 8), 0, 200, 80, true);
    component
}

#[test]
fn exact_edge_arming_does_not_jump_and_outer_column_is_one_wider() {
    let mut component = boundary();
    // Press on the near gap column (exact edge): no message on arm.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80)),
        None
    );
    // Motion within the exact edge still resolves to the current width, so
    // nothing is emitted (grabbing without motion changes nothing).
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 80)),
        None
    );
    // The outer gap column resolves one column wider.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 81)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(81)))
    );
}

#[test]
fn one_column_resolution_tracks_both_directions() {
    let mut component = boundary();
    component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80));
    // Dragging left leaves the gap immediately; tracking continues outside it.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 79)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(79)))
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 78)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(78)))
    );
}

#[test]
fn drag_outside_the_gap_continues_once_the_width_changed() {
    let mut component = boundary();
    component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80));
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 81)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(81)))
    );
    // Far outside the two-column gap, tracking still resolves.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 120)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(120)))
    );
}

#[test]
fn clamps_to_the_shared_pane_bounds() {
    let mut component = boundary();
    component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80));
    // 200 - 40 (hero minimum) - 2 (gap) = 158 is the largest valid width.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 300)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(158)))
    );
    // The list pane's own minimum is 40.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 10)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(40)))
    );
    // Motion beyond the bound stays at the bound.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 0)),
        None
    );
}

#[test]
fn click_only_and_drag_end_emit_nothing() {
    let mut component = boundary();
    assert_eq!(
        component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 81)),
        None
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Up(MouseButton::Left), 81)),
        None
    );
    // A changed drag emits the live width, but its release is still a no-op.
    assert_eq!(
        component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80)),
        None
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 81)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(81)))
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Up(MouseButton::Left), 81)),
        None
    );
}

#[test]
fn press_outside_the_gap_does_not_arm_a_drag() {
    let mut component = boundary();
    assert_eq!(
        component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 10)),
        None
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 120)),
        None
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Up(MouseButton::Left), 120)),
        None
    );
}

#[test]
fn eligibility_cancellation_resets_gesture_state() {
    let mut component = boundary();
    component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80));
    // Eligibility loss (empty/loading state, overlay mount, breakpoint flip)
    // resets the gesture state before the next delivery.
    component.sync(Rect::default(), 0, 200, 80, false);
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 120)),
        None
    );
    // Re-enabling without a fresh press still has no armed drag anchor.
    component.sync(Rect::new(80, 0, 2, 8), 0, 200, 80, true);
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 120)),
        None
    );
    // A fresh press inside the restored gap arms again.
    component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 80));
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 81)),
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(81)))
    );
}
