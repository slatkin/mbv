use super::msg::{Msg, QueueRequest};
use super::queue::{QueueComponent, QueueCursorUpdate};
use crate::app::state::types::playback::{PlaybackState, QueueScope};
use mbv_core::playback_queue::{PlaybackQueue, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn queue_three() -> Vec<mbv_core::playback_queue::QueueSlot> {
    PlaybackQueue::from_queue_items(
        (1..=3)
            .map(|index| {
                QueueItem::Emby(Box::new(crate::app::tests::make_item(
                    &format!("item-{index}"),
                    "Movie",
                )))
            })
            .collect(),
        None,
    )
    .slots()
    .to_vec()
}

fn component_with_slots(slots: Vec<mbv_core::playback_queue::QueueSlot>) -> QueueComponent {
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    component
}

fn drawn_component() -> (
    QueueComponent,
    Vec<mbv_core::playback_queue::QueueSlot>,
    Terminal<TestBackend>,
) {
    let slots = queue_three();
    let mut component = component_with_slots(slots.clone());
    // The panel derives its status overhead from the placement it is
    // handed (task 3.1); 14 rows leave enough framed body rows for the
    // three dragged rows to resolve.
    let mut terminal = Terminal::new(TestBackend::new(40, 14)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    (component, slots, terminal)
}

fn row_point(component: &QueueComponent, row_offset: u16) -> (u16, u16) {
    let row = component
        .selected_row_rect()
        .expect("selected queue row is retained after paint");
    (row.x, row.y + row_offset)
}

#[test]
fn queue_modified_click_does_not_arm_drag() {
    let (mut component, slots, _terminal) = drawn_component();
    let first = slots[0].slot_id;
    let third = slots[2].slot_id;
    let (row0_x, row0_y) = row_point(&component, 0);
    let (row2_x, row2_y) = row_point(&component, 2);

    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0_x,
            row: row0_y,
            modifiers: KeyModifiers::CONTROL,
        }))
        .is_some());
    assert_eq!(component.test_selected_target(), Some(first));
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2_x,
            row: row2_y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
    assert_eq!(component.test_selected_target(), Some(first));
    assert_ne!(first, third);
}

#[test]
fn queue_drag_moves_grabbed_row_onto_target() {
    let (mut component, slots, _terminal) = drawn_component();
    let first = slots[0].slot_id;
    let third = slots[2].slot_id;
    let (row0_x, row0_y) = row_point(&component, 0);
    let (row2_x, row2_y) = row_point(&component, 2);
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0_x,
            row: row0_y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_some());
    assert!(matches!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2_x,
            row: row2_y,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Queue(QueueRequest::MoveTo { slot_id, onto, .. })) if slot_id == first && onto == third
    ));
    assert_eq!(component.test_selected_target(), Some(first));
}

#[test]
fn queue_drag_blank_space_keeps_grab_for_later_row() {
    let (mut component, slots, _terminal) = drawn_component();
    let first = slots[0].slot_id;
    let third = slots[2].slot_id;
    let (row0_x, row0_y) = row_point(&component, 0);
    let (row2_x, row2_y) = row_point(&component, 2);
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0_x,
            row: row0_y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_some());
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2_x,
            row: row2_y + 1,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
    assert!(matches!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2_x,
            row: row2_y,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Queue(QueueRequest::MoveTo { slot_id, onto, .. })) if slot_id == first && onto == third
    ));
}

#[test]
fn queue_drag_without_press_emits_no_message() {
    let (mut component, _slots, _terminal) = drawn_component();
    let (column, row) = row_point(&component, 1);
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
}

#[test]
fn queue_drag_end_clears_grab() {
    let (mut component, _slots, _terminal) = drawn_component();
    let (row0_x, row0_y) = row_point(&component, 0);
    let (row2_x, row2_y) = row_point(&component, 2);
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0_x,
            row: row0_y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_some());
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: row0_x,
            row: row0_y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2_x,
            row: row2_y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
}
