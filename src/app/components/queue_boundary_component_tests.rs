use super::msg::{Msg, QueueRequest};
use super::queue_boundary::QueueBoundaryComponent;
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

fn boundary() -> QueueBoundaryComponent {
    let mut component = QueueBoundaryComponent::new();
    component.sync(Rect::new(10, 0, 1, 8), 10, 200, 50, true, true);
    component
}

#[test]
fn exact_column_arms_and_resolves_from_frame_origin() {
    let mut component = boundary();
    assert_eq!(
        component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 10)),
        None
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 60)),
        Some(Msg::Queue(QueueRequest::ResizeColumnLive(51)))
    );
    let mut outside = boundary();
    assert_eq!(
        outside.on(&mouse(MouseEventKind::Down(MouseButton::Left), 9)),
        None
    );
    assert_eq!(
        outside.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 20)),
        None
    );
}

#[test]
fn click_only_and_disabled_sync_cancel_without_messages() {
    let mut component = boundary();
    assert_eq!(
        component.on(&mouse(MouseEventKind::Down(MouseButton::Left), 10)),
        None
    );
    assert_eq!(
        component.on(&mouse(MouseEventKind::Up(MouseButton::Left), 10)),
        None
    );
    component.sync(Rect::default(), 0, 100, 50, true, false);
    assert_eq!(
        component.on(&mouse(MouseEventKind::Drag(MouseButton::Left), 80)),
        None
    );
}
