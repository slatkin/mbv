use super::msg::{Msg, QueueRequest};
use super::queue::{QueueComponent, QueueCursorUpdate};
use crate::app::render::QueueTitleModel;
use crate::app::types_playback::{PlaybackState, QueueScope};
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

fn component() -> QueueComponent {
    let mut component = QueueComponent::new();
    component.set_content(
        queue_three(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    component.set_focused(true);
    component
}

fn drawn_component() -> (QueueComponent, Terminal<TestBackend>) {
    let mut component = component();
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    (component, terminal)
}

#[test]
fn queue_drag_moves_grabbed_row_onto_target() {
    let (mut component, _terminal) = drawn_component();
    let (row0, first) = component.test_rows()[0];
    let (row2, third) = component.test_rows()[2];
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0.x,
            row: row0.y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_some());
    assert!(matches!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2.x,
            row: row2.y,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Queue(QueueRequest::MoveTo { slot_id, onto, .. })) if slot_id == first && onto == third
    ));
    assert_eq!(component.test_selected_target(), Some(first));
}

#[test]
fn queue_drag_blank_space_keeps_grab_for_later_row() {
    let (mut component, _terminal) = drawn_component();
    let (row0, first) = component.test_rows()[0];
    let (row2, third) = component.test_rows()[2];
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0.x,
            row: row0.y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_some());
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2.x,
            row: row2.y + 1,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
    assert!(matches!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2.x,
            row: row2.y,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Queue(QueueRequest::MoveTo { slot_id, onto, .. })) if slot_id == first && onto == third
    ));
}

#[test]
fn queue_drag_without_press_emits_no_message() {
    let (mut component, _terminal) = drawn_component();
    let (row, _) = component.test_rows()[1];
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row.x,
            row: row.y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
}

#[test]
fn queue_drag_end_clears_grab() {
    let (mut component, _terminal) = drawn_component();
    let (row0, _) = component.test_rows()[0];
    let (row2, _) = component.test_rows()[2];
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row0.x,
            row: row0.y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_some());
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: row0.x,
            row: row0.y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
    assert!(component
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: row2.x,
            row: row2.y,
            modifiers: KeyModifiers::NONE,
        }))
        .is_none());
}
