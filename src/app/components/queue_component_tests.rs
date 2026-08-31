use super::msg::{Msg, QueueColumnResize, QueueHitRegion, QueueIntent, QueueRequest, ShellRequest};
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

/// A queue long enough that rendering the bottom cursor produces a nonzero
/// viewport scroll (30 slots in an 8-row terminal).
fn long_queue() -> Vec<mbv_core::playback_queue::QueueSlot> {
    let items: Vec<QueueItem> = (0..30)
        .map(|i| {
            QueueItem::Emby(Box::new(crate::app::tests::make_item(
                &format!("item-{i}"),
                "Audio",
            )))
        })
        .collect();
    PlaybackQueue::from_queue_items(items, None)
        .slots()
        .to_vec()
}

#[test]
fn queue_component_upward_scrolling_reaches_top() {
    let mut component = QueueComponent::new();
    component.set_content(
        long_queue(),
        29,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    for _ in 0..29 {
        component.on(&Event::Keyboard(key(Key::Up)));
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
    }
    assert_eq!(component.test_cursor(), 0);
    assert_eq!(component.test_scroll(), 0);
}

#[test]
fn queue_component_page_up_from_bottom_reaches_top() {
    let mut component = QueueComponent::new();
    component.set_content(
        long_queue(),
        29,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    component.set_area(ratatui::layout::Rect::new(0, 0, 40, 8));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    for _ in 0..5 {
        component.on(&Event::Keyboard(key(Key::PageUp)));
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
    }
    assert_eq!(component.test_cursor(), 0);
    assert_eq!(component.test_scroll(), 0);
}

#[test]
fn queue_component_instances_isolate_viewport_state() {
    let slots = long_queue();
    let mut bottom = QueueComponent::new();
    bottom.set_content(
        slots.clone(),
        29,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut untouched = QueueComponent::new();
    untouched.set_content(
        slots,
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| bottom.view(frame, frame.area()))
        .unwrap();
    terminal
        .draw(|frame| untouched.view(frame, frame.area()))
        .unwrap();
    assert!(bottom.test_scroll() > 0);
    assert_eq!(untouched.test_scroll(), 0);
    assert_eq!(untouched.test_cursor(), 0);
}

#[test]
fn now_playing_queue_row_shows_elapsed_next_to_duration() {
    let mut item = crate::app::tests::make_item("playing", "Audio");
    item.runtime_ticks = 120 * mbv_core::api::TICKS_PER_SECOND;
    let slot = PlaybackQueue::from_queue_items(vec![QueueItem::Emby(Box::new(item))], None)
        .slots()
        .to_vec();
    let mut component = QueueComponent::new();
    component.set_content(
        slot,
        0,
        QueueScope::Local,
        true,
        PlaybackState {
            active: true,
            active_idx: 0,
            position_ticks: 30 * mbv_core::api::TICKS_PER_SECOND,
            runtime_ticks: 120 * mbv_core::api::TICKS_PER_SECOND,
            paused: false,
        },
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
    assert!(output.contains("0:30 / 2:00"));
}

#[test]
fn queue_scope_switch_resets_component_scroll() {
    // Scroll is component-owned (split-queue-cursor-ownership D3): switching
    // scope resets the component's own scroll to 0. Drive scroll nonzero by
    // rendering with a bottom cursor, then switch scope.
    let slots = long_queue();
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        29,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(
        component.test_scroll() > 0,
        "bottom cursor must produce nonzero scroll, got {}",
        component.test_scroll()
    );

    // Keyboard scope change preassigns `self.scope` before the request, so
    // the shell's `set_content` scope-diff reset would not fire; the
    // component must reset its own scroll at key time.
    component.on(&Event::Keyboard(key(Key::Char(']'))));
    assert_eq!(
        component.test_scroll(),
        0,
        "keyboard scope change must reset the component's own scroll"
    );

    // External scope change (e.g. session switch) flows through
    // `set_content` with a differing scope; the component resets there too.
    component.set_content(
        long_queue(),
        29,
        QueueScope::Remote,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(
        component.test_scroll() > 0,
        "remote content must again produce nonzero scroll, got {}",
        component.test_scroll()
    );
    component.set_content(
        long_queue(),
        0,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        QueueTitleModel::default(),
    );
    assert_eq!(
        component.test_scroll(),
        0,
        "set_content scope change must reset the component's own scroll"
    );
}

#[test]
fn queue_scope_mouse_pills_reset_component_scroll_from_nonzero() {
    // The mouse scope-pill branches (components/queue.rs handle_mouse) must
    // reset the component's own scroll from a nonzero viewport, just like the
    // '['/']' keys and set_content scope changes. Drive each pill branch
    // independently from nonzero scroll and assert the reset.
    let title = QueueTitleModel {
        local_icon: "L".into(),
        local_label: "Local".into(),
        remote_icon: "R".into(),
        remote_label: "Remote".into(),
        local_selected: true,
        show_split: true,
        is_mbv_session: true,
    };

    let slots = long_queue();
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        29,
        QueueScope::Local,
        true,
        PlaybackState::default(),
        title.clone(),
    );
    component.set_title_area(Some(ratatui::layout::Rect::new(0, 0, 40, 1)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(
        component.test_scroll() > 0,
        "bottom cursor must produce nonzero scroll, got {}",
        component.test_scroll()
    );
    let (local_pill, remote_pill) = component.test_scope_pill_areas();
    assert!(
        local_pill.width > 0 && remote_pill.width > 0,
        "scope pills must be painted for the split title"
    );

    // Click the Remote pill: scope preassigned to Remote, scroll reset to 0.
    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: remote_pill.x,
        row: remote_pill.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::QueueClick {
                region: QueueHitRegion::ScopeRemote,
                ..
            }))
        ),
        "remote pill click must emit a ScopeRemote QueueClick"
    );
    assert_eq!(
        component.test_scroll(),
        0,
        "remote pill click must reset the component's own scroll"
    );

    // Re-render at the bottom cursor to restore nonzero scroll, then click
    // the Local pill: scope preassigned to Local, scroll reset to 0.
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(
        component.test_scroll() > 0,
        "bottom cursor must again produce nonzero scroll, got {}",
        component.test_scroll()
    );
    let (local_pill, _) = component.test_scope_pill_areas();
    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: local_pill.x,
        row: local_pill.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::QueueClick {
                region: QueueHitRegion::ScopeLocal,
                ..
            }))
        ),
        "local pill click must emit a ScopeLocal QueueClick"
    );
    assert_eq!(
        component.test_scroll(),
        0,
        "local pill click must reset the component's own scroll"
    );
}
