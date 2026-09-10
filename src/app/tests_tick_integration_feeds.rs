use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::{ComponentId, FeedsComponent, Msg, TerminalObserverEvent};
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, TabSelection};
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;

fn entry(guid: &str, title: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(120 * mbv_core::api::TICKS_PER_SECOND as u64),
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("feed".into()),
        position_ticks: 0,
        played: false,
    }
}

fn harness(width: u16) -> TickHarness {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    app.terminal_width = width;
    app.terminal_height = 24;
    app.feed_tab.subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    app.feed_tab.entries = vec![vec![entry("one", "One"), entry("two", "Two")]];
    app.feed_tab.rebuild_all_entries();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn draw(harness: &mut TickHarness, width: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

fn feeds(harness: &TickHarness) -> &FeedsComponent {
    harness
        .model()
        .application
        .get_component(&ComponentId::Feeds)
        .expect("Feeds mounted")
        .as_any()
        .downcast_ref::<FeedsComponent>()
        .expect("Feeds component")
}

#[test]
fn feeds_tick_navigation_paints_selected_row_at_wide_and_narrow() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(feeds(&harness).cursor(), 0);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(feeds(&harness).cursor(), 1);

    let narrow = crate::app::TWO_COLUMN_THRESHOLD - 1;
    draw(&mut harness, narrow);
    assert_eq!(feeds(&harness).cursor(), 1);
}

#[test]
fn feeds_tick_click_resolves_painted_entry_and_blank_is_noop() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut harness = harness(width);
        draw(&mut harness, width);
        let selected = feeds(&harness).layout().selected_item_rect.expect("row");
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: selected.x + 1,
            row: selected.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(!outcome.raw_messages.is_empty());
        assert_eq!(feeds(&harness).cursor(), 0);

        let before_cursor = feeds(&harness).cursor();
        let before_paint = feeds(&harness).layout().selected_item_rect;
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert_eq!(
            outcome.raw_messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::MouseClick {
                column: 0,
                row: 0,
            })]
        );
        assert_eq!(feeds(&harness).cursor(), before_cursor);
        assert_eq!(feeds(&harness).layout().selected_item_rect, before_paint);
    }
}

#[test]
fn feeds_tick_wheel_is_claimed_only_over_active_control() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut off_harness = harness(width);
        draw(&mut off_harness, width);
        let before_cursor = feeds(&off_harness).cursor();
        let before_paint = feeds(&off_harness).layout().selected_item_rect;
        off_harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = off_harness.step();
        assert_eq!(
            outcome.raw_messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::NoOp)]
        );
        assert_eq!(feeds(&off_harness).cursor(), before_cursor);
        assert_eq!(feeds(&off_harness).layout().selected_item_rect, before_paint);

        let mut harness = harness(width);
        draw(&mut harness, width);
        let before_cursor = feeds(&harness).cursor();
        let before_paint = feeds(&harness).layout().selected_item_rect;
        let list = feeds(&harness).layout().left_area;
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: list.x + 1,
            row: list.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(!outcome.raw_messages.is_empty());
        draw(&mut harness, width);
        assert_eq!(feeds(&harness).cursor(), before_cursor);
        assert_eq!(feeds(&harness).layout().selected_item_rect, before_paint);
    }
}

#[test]
fn feeds_tick_round_trip_preserves_selected_target() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Down, modifiers: KeyModifiers::NONE }));
    let _ = harness.step();
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD - 1);
    assert_eq!(feeds(&harness).cursor(), 1);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(feeds(&harness).cursor(), 1);
}

#[test]
fn feeds_tick_has_one_painted_list_surface() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert!(feeds(&harness).layout().selected_item_rect.is_some());
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD - 1);
    assert!(feeds(&harness).layout().selected_item_rect.is_some());
}
