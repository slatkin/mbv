use super::media_list::{MediaListRow, MediaSemanticState};
use super::msg::{Msg, QueueColumnResize, QueueIntent, QueueRequest, ShellRequest};
use super::queue::{queue_media_rows, QueueComponent, QueueCursorUpdate};
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
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);

    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Down))),
        Some(Msg::Queue(QueueRequest::Cursor { slot_id, .. })) if slot_id == second
    ));

    let mut reordered = slots;
    reordered.swap(0, 1);
    component.set_content(
        reordered,
        QueueCursorUpdate::Preserve,
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Enter))),
        Some(Msg::Queue(QueueRequest::Play { slot_id, .. })) if slot_id == second
    ));
}

/// Regression test for the bug where `set_content` only ever consulted its
/// cursor argument inside the slot-identity fallback, so a `Set` push was
/// silently discarded whenever the previously selected slot still existed.
/// That's the common case for follow-the-playhead: no slot is removed (a
/// music album keeps every slot alive since `consume_audio` defaults to
/// false), so identity reconciliation alone would keep the cursor pinned to
/// the item the user had selected instead of moving it to the newly playing
/// item.
#[test]
fn queue_set_content_follow_the_playhead_moves_cursor_when_slots_persist() {
    let slots = queue();
    let mut component = QueueComponent::new();
    component.set_content(
        slots.clone(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    assert_eq!(component.test_cursor(), 0);

    // Same slot list, no removal: an identity-based `Preserve` would find
    // slot 0 still present at index 0 and leave the cursor there.
    component.set_content(
        slots,
        QueueCursorUpdate::Set(1),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    assert_eq!(
        component.test_cursor(),
        1,
        "a Set push must move the cursor even when the previously selected slot persists"
    );
}

#[test]
fn queue_delete_removes_multi_selection_request_and_clears_selection() {
    let slots = queue();
    let first = slots[0].slot_id;
    let second = slots[1].slot_id;
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    component.test_toggle_selection(first);
    component.test_toggle_selection(second);

    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Delete))),
        Some(Msg::Queue(QueueRequest::RemoveSelection {
            scope: QueueScope::Local,
            slot_ids,
        })) if slot_ids == vec![first, second]
    ));
    assert!(component.test_multi_selection().is_empty());
}

#[test]
fn queue_delete_without_multi_selection_removes_cursor_slot() {
    let slots = queue();
    let first = slots[0].slot_id;
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);

    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Delete))),
        Some(Msg::Queue(QueueRequest::Remove { scope: QueueScope::Local, slot_id }))
            if slot_id == first
    ));
}

#[test]
fn queue_component_emits_typed_keyboard_intents() {
    let mut component = QueueComponent::new();
    component.set_content(
        queue(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
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
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    // 9 rows: the recessed box fits the blank inset + title + two list rows
    // + bottom padding alongside the footer band, so both seeded rows paint.
    let mut terminal = Terminal::new(TestBackend::new(40, 9)).unwrap();

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
        QueueCursorUpdate::Set(1),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let rect = component
        .selected_row_rect()
        .expect("selected queue row is retained after paint");
    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        matches!(message, Some(Msg::Shell(super::msg::ShellRequest::RowContextMenu(
        crate::app::types_context_menu::ContextMenuTargets::Queue(ids), Some(_)
    ))) if ids == vec![second])
    );
}

#[test]
fn queue_dot_opens_the_context_menu_for_the_selected_row() {
    let slots = queue();
    let first = slots[0].slot_id;
    let mut component = QueueComponent::new();
    component.set_content(
        slots,
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Char('.')))),
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Queue(ids), None
        ))) if ids == vec![first]
    ));
}

#[test]
fn queue_right_click_on_blank_space_opens_no_menu() {
    let mut component = QueueComponent::new();
    component.set_content(
        queue(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    // Row 6 is below the two rendered slots — blank queue space.
    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 1,
        row: 6,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        message.is_none(),
        "a right-click on blank queue space must not open a menu"
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
        QueueCursorUpdate::Set(29),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
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
        QueueCursorUpdate::Set(29),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    // The page step (design D6) is the painted row-flow height, resolved
    // from the framed content rectangle the view retains (design D2): from
    // the bottom-seeded cursor one PageUp pages the window to the top and
    // drags the selection to the nearest row it shows.
    let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component.on(&Event::Keyboard(key(Key::PageUp)));
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert_eq!(
        component.test_scroll(),
        0,
        "the page step pages the window to the top"
    );
    assert_eq!(
        component.test_cursor(),
        17,
        "the drag pulls the selection to the nearest row the paged window shows"
    );
    // A further page-up at the content end is a boundary no-op.
    component.on(&Event::Keyboard(key(Key::PageUp)));
    assert_eq!(component.test_scroll(), 0);
    assert_eq!(component.test_cursor(), 17);
}

#[test]
fn queue_component_instances_isolate_viewport_state() {
    let slots = long_queue();
    let mut bottom = QueueComponent::new();
    bottom.set_content(
        slots.clone(),
        QueueCursorUpdate::Set(29),
        QueueScope::Local,
        PlaybackState::default(),
    );
    bottom.set_focused(true);
    let mut untouched = QueueComponent::new();
    untouched.set_content(
        slots,
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    untouched.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| bottom.view(frame, frame.area()))
        .unwrap();
    terminal
        .draw(|frame| untouched.view(frame, frame.area()))
        .unwrap();
    assert!(
        bottom.test_painted_offset().unwrap_or(0) > 0,
        "the bottom instance's display clamp follows its bottom cursor"
    );
    assert_eq!(untouched.test_painted_offset(), Some(0));
    assert_eq!(untouched.test_cursor(), 0);
}

#[test]
fn queue_projection_clamps_active_progress_to_presentation_bounds() {
    let mut item = crate::app::tests::make_item("bounded", "Audio");
    item.runtime_ticks = 100;
    let slots = PlaybackQueue::from_queue_items(vec![QueueItem::Emby(Box::new(item))], None)
        .slots()
        .to_vec();

    for position_ticks in [0, 250] {
        let rows = queue_media_rows(
            &slots,
            PlaybackState {
                active: true,
                active_idx: Some(0),
                position_ticks,
                runtime_ticks: 100,
                paused: false,
            },
            None,
        );
        let Some(MediaListRow::Item { semantic_state, .. }) = rows.first() else {
            panic!("queue projection must produce an item row")
        };
        let MediaSemanticState::NowPlaying { progress } = semantic_state else {
            panic!("active queue row must use now-playing semantic state")
        };
        assert_eq!(
            progress.as_ref().map(|value| value.percent()),
            if position_ticks == 0 { None } else { Some(100) }
        );
    }
}

#[test]
fn queue_refresh_retains_selected_target_and_scrolls_to_it() {
    let slots = long_queue();
    let selected = slots[20].slot_id;
    let mut component = QueueComponent::new();
    component.set_content(
        slots.clone(),
        QueueCursorUpdate::Set(20),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component.set_content(
        slots,
        QueueCursorUpdate::Preserve,
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert_eq!(component.test_cursor(), 20);
    assert_eq!(component.test_selected_target(), Some(selected));
    assert!(
        component.selected_row_rect().is_some(),
        "the refreshed paint keeps the retained selection on screen"
    );
}

#[test]
fn queue_cursor_push_never_hand_clamps_the_window() {
    // D9 retirement: `set_cursor`'s `scroll.min(cursor)` hand clamp is gone.
    // The cursor path (`select_index` -> `follow_cursor`) is the only window
    // movement a cursor push needs: a Set below the window lowers it through
    // the cursor path, and a Set inside the stored window leaves it where it
    // is (an explicit scope reset remains the only other writer).
    let mut component = QueueComponent::new();
    component.set_content(
        long_queue(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.test_seed_scroll(12);
    component.set_cursor(QueueCursorUpdate::Set(4));
    assert_eq!(component.test_cursor(), 4);
    assert_eq!(
        component.test_scroll(),
        4,
        "the cursor path lowers the window"
    );

    component.test_seed_scroll(4);
    component.set_cursor(QueueCursorUpdate::Set(6));
    assert_eq!(component.test_cursor(), 6);
    assert_eq!(
        component.test_scroll(),
        4,
        "a Set inside the window leaves the stored window untouched"
    );

    // The discriminating case: a window seeded BELOW the selection and a
    // push that does not move it. The cursor path (follow_cursor) only
    // lowers the window when the selection is above it, so the stored
    // window stays authoritative at 9 — a reinstated `scroll.min(cursor)`
    // hand clamp would drag it up to the cursor at 3.
    component.set_cursor(QueueCursorUpdate::Set(3));
    component.test_seed_scroll(9);
    component.set_cursor(QueueCursorUpdate::Preserve);
    assert_eq!(component.test_cursor(), 3);
    assert_eq!(
        component.test_scroll(),
        9,
        "a push that keeps the selection never moves the window"
    );
}

#[test]
fn queue_movement_uses_single_row_stride_and_follows_focus() {
    let mut component = QueueComponent::new();
    component.set_content(
        long_queue(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Down))),
        Some(Msg::Queue(QueueRequest::Cursor { .. }))
    ));
    assert_eq!(component.test_cursor(), 1);
    // PageDown is the page step (design D6): the window pages while a
    // mid-window selection rides nowhere, so no cursor echo reports.
    let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    // The selection sits on the window's top edge, so the page step drags it
    // to the nearest row the paged window shows and the cursor echo reports;
    // 30 rows over an 18-row page clamp the window at the content end.
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::PageDown))),
        Some(Msg::Queue(QueueRequest::Cursor { .. }))
    ));
    assert_eq!(component.test_cursor(), 12);
    assert_eq!(component.test_scroll(), 12);
    // A further page at the content end is a boundary no-op: claimed by the
    // component's nav-key contract, reporting no selection move.
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::PageDown))),
        Some(Msg::TerminalEvent(_))
    ));
    assert_eq!(component.test_cursor(), 12);
    assert_eq!(component.test_scroll(), 12);
}

#[test]
fn now_playing_queue_row_drops_elapsed_and_keeps_progress() {
    let mut item = crate::app::tests::make_item("playing", "Audio");
    item.runtime_ticks = 120 * mbv_core::api::TICKS_PER_SECOND;
    let slot = PlaybackQueue::from_queue_items(vec![QueueItem::Emby(Box::new(item))], None)
        .slots()
        .to_vec();
    let mut component = QueueComponent::new();
    component.set_content(
        slot,
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState {
            active: true,
            active_idx: Some(0),
            position_ticks: 30 * mbv_core::api::TICKS_PER_SECOND,
            runtime_ticks: 120 * mbv_core::api::TICKS_PER_SECOND,
            paused: false,
        },
    );
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(!output.contains("0:30 / 2:00"));
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
        QueueCursorUpdate::Set(29),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    // The framed sub-areas derive from the placement the view receives (task
    // 3.1); 14 rows leave eight body rows, enough for an overflow window.
    // The stored window is seeded explicitly (an explicit seed is the one
    // legitimate writer besides input): the paint is read-only (design D2).
    let mut terminal = Terminal::new(TestBackend::new(40, 14)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component.test_seed_scroll(20);
    assert_eq!(
        component.test_scroll(),
        20,
        "test setup: the stored window sits in overflow"
    );

    // Keyboard scope change preassigns `self.scope` before the request, so
    // the shell's `set_content` scope-diff reset would not fire; the
    // component must reset its own scroll at key time. This assertion is
    // necessarily taken before the next draw: the stale (pre-refresh) cursor
    // is still 29 rows below the top, so rendering now would legitimately
    // re-clamp scroll to reveal it, independent of whether the reset ran.
    component.on(&Event::Keyboard(key(Key::Char(']'))));
    assert_eq!(
        component.test_scroll(),
        0,
        "keyboard scope change must reset the component's own scroll"
    );

    // External scope change (e.g. session switch) flows through
    // `set_content` with a differing scope; the component resets there too.
    // The stored window is seeded explicitly; the paint is read-only
    // (design D2), so the setup's overflow never comes from a paint.
    component.set_content(
        long_queue(),
        QueueCursorUpdate::Set(29),
        QueueScope::Remote,
        PlaybackState::default(),
    );
    component.set_focused(true);
    // Same framed-subarea note (task 3.1): the in-view cursor below needs
    // more than the two body rows a 40x8 placement leaves, so keep the body
    // at eight rows via a 40x14 placement.
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component.test_seed_scroll(20);
    assert_eq!(
        component.test_scroll(),
        20,
        "test setup: the stored window sits in overflow again"
    );
    component.set_content(
        long_queue(),
        // A nonzero-but-in-view cursor, not 0: with the stale (pre-reset)
        // scroll from the Remote content above, render's own reveal-cursor
        // clamp would independently drag scroll down to this same cursor
        // value regardless of whether the reset ran, which would make a
        // cursor-0 assertion pass even on a broken reset. This value only
        // renders as scroll 0 if the reset actually fired.
        QueueCursorUpdate::Set(3),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    // Assert after a draw, not immediately after `set_content`: only a draw
    // proves the reset survives the render pass rather than just the field
    // write.
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert_eq!(
        component.test_scroll(),
        0,
        "set_content scope change must reset the component's own scroll"
    );
}

fn scope_title(local_selected: bool) -> crate::app::render::components::queue::QueueTitleModel {
    crate::app::render::components::queue::QueueTitleModel {
        local_icon: String::new(),
        local_label: String::new(),
        remote_icon: "M".into(),
        local_selected,
        show_split: true,
        is_mbv_session: true,
    }
}

fn footer_component(
    scope: Option<crate::app::render::components::queue::QueueTitleModel>,
) -> QueueComponent {
    let mut component = QueueComponent::new();
    component.set_content(
        queue(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_status_pills(Vec::new(), None, scope);
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component
}

fn click(column: u16, row: u16) -> Event<super::user_event::UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

/// The QueueColumn footer paints the Local/Remote scope pills (queue
/// concern, while on an mbv-based session) at the far right, each region
/// over its painted pill.
#[test]
fn queue_footer_paints_scope_pills_at_the_far_right() {
    let component = footer_component(Some(scope_title(true)));
    let (local, remote) = component.test_scope_pill_areas();
    let local = local.expect("local scope region");
    let remote = remote.expect("remote scope region");
    assert_eq!(local.right(), remote.x, "local precedes remote");
    assert_eq!(
        remote.right(),
        38,
        "scope pills end at the footer's right edge"
    );
    assert_eq!(local.y, 10, "scope pills sit on the footer row");
}

/// Clicking a footer scope pill emits the matching `QueueScopeClick`.
#[test]
fn click_on_a_footer_scope_pill_emits_the_scope_click() {
    let mut component = footer_component(Some(scope_title(false)));
    let (local, remote) = component.test_scope_pill_areas();
    let local = local.expect("local region");
    let remote = remote.expect("remote region");
    assert_eq!(
        component.on(&click(local.x + 1, local.y)),
        Some(Msg::Shell(ShellRequest::QueueScopeClick {
            scope: QueueScope::Local
        }))
    );
    assert_eq!(
        component.on(&click(remote.x + 1, remote.y)),
        Some(Msg::Shell(ShellRequest::QueueScopeClick {
            scope: QueueScope::Remote
        }))
    );
}

/// Without an mbv-based session the footer shows no scope pills and a
/// footer click resolves to no scope message.
#[test]
fn queue_footer_hides_scope_pills_when_disconnected() {
    let mut component = footer_component(None);
    assert_eq!(component.test_scope_pill_areas(), (None, None));
    assert_eq!(component.on(&click(37, 10)), None);
}

/// The Queue's one-row viewport chords (task 6.2, design D7): `Ctrl+y`/
/// `Ctrl+e` step the window one display row; the selection rides only at
/// the window's edge and a window-only step claims without a cursor echo.
#[test]
fn queue_viewport_ctrl_chords_step_the_window() {
    let mut component = QueueComponent::new();
    component.set_content(
        long_queue(),
        QueueCursorUpdate::Set(0),
        QueueScope::Local,
        PlaybackState::default(),
    );
    component.set_focused(true);
    for _ in 0..5 {
        component.on(&Event::Keyboard(key(Key::Down)));
    }
    let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let before_cursor = component.test_cursor();

    assert!(matches!(
        component.on(&Event::Keyboard(chord(
            Key::Char('y'),
            KeyModifiers::CONTROL
        ))),
        Some(Msg::TerminalEvent(_))
    ));
    assert_eq!(component.test_scroll(), 1);
    assert_eq!(component.test_cursor(), before_cursor);

    assert!(matches!(
        component.on(&Event::Keyboard(chord(
            Key::Char('e'),
            KeyModifiers::CONTROL
        ))),
        Some(Msg::TerminalEvent(_))
    ));
    assert_eq!(component.test_scroll(), 0);
    assert_eq!(component.test_cursor(), before_cursor);
}
