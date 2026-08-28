use super::msg::{Msg, QueueColumnResize, QueueIntent, QueueRequest, ShellRequest};
use super::queue::QueueComponent;
use crate::app::render::QueueTitleModel;
use crate::app::types_playback::{PlaybackState, QueueScope};
use mbv_core::playback_queue::{PlaybackQueue, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn chord(code: Key, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers }
}

fn queue() -> Vec<mbv_core::playback_queue::QueueSlot> {
    PlaybackQueue::from_queue_items(
        vec![
            QueueItem::Emby(Box::new(crate::app::tests::make_item("one", "Movie"))),
            QueueItem::Emby(Box::new(crate::app::tests::make_item("two", "Movie"))),
        ],
        None,
    )
    .slots()
    .to_vec()
}

#[test]
fn queue_activation_uses_slot_id_after_snapshot_reorder() {
    let slots = queue();
    let second = slots[1].slot_id;
    let mut component = QueueComponent::new();
    component.set_content(
        slots.clone(),
        0,
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );

    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Down))),
        Some(Msg::Queue(QueueRequest::Cursor { slot_id, .. })) if slot_id == second
    ));

    let mut reordered = slots;
    reordered.swap(0, 1);
    component.set_content(
        reordered,
        0,
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Enter))),
        Some(Msg::Queue(QueueRequest::Play { slot_id, .. })) if slot_id == second
    ));
}

#[test]
fn queue_component_emits_typed_keyboard_intents() {
    let mut component = QueueComponent::new();
    component.set_content(
        queue(),
        0,
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    assert!(matches!(
        component.on(&Event::Keyboard(chord(Key::Char(']'), KeyModifiers::NONE))),
        Some(Msg::Queue(QueueRequest::Scope(QueueScope::Remote)))
    ));
    assert!(matches!(
        component.on(&Event::Keyboard(chord(
            Key::Char('z'),
            KeyModifiers::CONTROL
        ))),
        Some(Msg::Queue(QueueRequest::Undo {
            scope: QueueScope::Remote
        }))
    ));
    assert!(matches!(
        component.on(&Event::Keyboard(chord(
            Key::Char('t'),
            KeyModifiers::CONTROL
        ))),
        Some(Msg::Shell(ShellRequest::QueueIntent(
            QueueIntent::StopRemoteTracking
        )))
    ));
    assert!(matches!(
        component.on(&Event::Keyboard(chord(Key::Left, KeyModifiers::SHIFT))),
        Some(Msg::Shell(ShellRequest::QueueIntent(
            QueueIntent::ResizeColumn(QueueColumnResize::Narrower)
        )))
    ));
    assert!(matches!(
        component.on(&Event::Keyboard(chord(Key::Char('c'), KeyModifiers::NONE))),
        Some(Msg::Shell(ShellRequest::QueueIntent(QueueIntent::Clear)))
    ));
    assert!(
        component
            .on(&Event::Keyboard(chord(Key::Char('x'), KeyModifiers::NONE)))
            .is_none(),
        "unhandled queue keys must return None (no legacy QueueKey to reconstruct)"
    );
}

#[test]
fn queue_component_renders_a_snapshot_without_app_state() {
    let mut component = QueueComponent::new();
    component.set_content(
        queue(),
        0,
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();

    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(output.contains("one"));
    assert!(output.contains("two"));
}

#[test]
fn queue_right_click_uses_the_rendered_slot_target() {
    let slots = queue();
    let second = slots[1].slot_id;
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        0,
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let (rect, _) = component.test_rows()[1];
    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        matches!(message, Some(Msg::Shell(super::msg::ShellRequest::QueueClick {
        region: super::msg::QueueHitRegion::ContextMenu(Some(slot_id)), ..
    })) if slot_id == second)
    );
}
