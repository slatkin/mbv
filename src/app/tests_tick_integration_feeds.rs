use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::feeds_content::FeedsContent;
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::{ComponentId, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::state::types::feed_tab::WatchedFilter;
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

fn panel(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
}

/// The registered Feeds owner inside the mounted `LibraryPanel` (task 7.3):
/// the panel is the Feeds surface's one event boundary, and the feed state is
/// read through the owner the panel hosts — never a destination component.
fn feeds_owner(harness: &TickHarness) -> &FeedsContent {
    panel(harness)
        .owner(&LibraryKey::Feeds)
        .and_then(|owner| owner.as_any().downcast_ref::<FeedsContent>())
        .expect("Feeds owner installed")
}

fn list_rect(harness: &TickHarness) -> Rect {
    panel(harness)
        .test_wide_geometry()
        .map(|geometry| geometry.list_area)
        .or_else(|| {
            panel(harness)
                .test_narrow_geometry()
                .map(|geometry| geometry.list_area)
        })
        .expect("the panel painted a list slot")
}

fn selected_rect(harness: &TickHarness) -> Option<Rect> {
    panel(harness)
        .test_wide_geometry()
        .and_then(|geometry| geometry.selected)
        .or_else(|| {
            panel(harness)
                .test_narrow_geometry()
                .and_then(|geometry| geometry.selected)
        })
}

#[test]
fn feeds_tick_navigation_paints_selected_row_at_wide_and_narrow() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(feeds_owner(&harness).cursor(), 0);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(feeds_owner(&harness).cursor(), 1);

    let narrow = crate::app::TWO_COLUMN_THRESHOLD - 1;
    draw(&mut harness, narrow);
    assert_eq!(feeds_owner(&harness).cursor(), 1);
}

#[test]
fn feeds_tick_leaf_enter_opens_overlay_then_activates_selected_entry() {
    let width = crate::app::TWO_COLUMN_THRESHOLD - 1;
    let mut harness = harness(width);
    draw(&mut harness, width);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let opened = harness.step();
    assert!(opened.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)
    )));
    draw(&mut harness, width);
    assert!(panel(&harness).test_overlay_geometry().is_some());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let activated = harness.step();
    assert!(activated.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::FeedsPlay(entries)) if entries.len() == 1 && entries[0].guid == "one"
    )));
    assert!(panel(&harness).test_hero_overlay_open());
}

#[test]
fn feeds_tick_click_resolves_painted_entry_and_blank_is_noop() {
    for width in [
        crate::app::TWO_COLUMN_THRESHOLD,
        crate::app::TWO_COLUMN_THRESHOLD - 1,
    ] {
        let mut harness = harness(width);
        draw(&mut harness, width);
        let selected = selected_rect(&harness).expect("row");
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: selected.x + 1,
            row: selected.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(!outcome.raw_messages.is_empty());
        assert_eq!(feeds_owner(&harness).cursor(), 0);

        let before_cursor = feeds_owner(&harness).cursor();
        let before_paint = selected_rect(&harness);
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
        assert_eq!(feeds_owner(&harness).cursor(), before_cursor);
        assert_eq!(selected_rect(&harness), before_paint);
    }
}

#[test]
fn feeds_tick_wheel_is_claimed_only_over_active_control() {
    for width in [
        crate::app::TWO_COLUMN_THRESHOLD,
        crate::app::TWO_COLUMN_THRESHOLD - 1,
    ] {
        let mut off_harness = harness(width);
        draw(&mut off_harness, width);
        let before_cursor = feeds_owner(&off_harness).cursor();
        let before_paint = selected_rect(&off_harness);
        off_harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = off_harness.step();
        // The off-control wheel is still claimed by nothing: the marker is
        // the observer's mouse signal (design D6, task 6.1 — the shell's
        // silent prefix-disarm signal), not a claim.
        assert_eq!(
            outcome.raw_messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::Mouse)]
        );
        assert_eq!(feeds_owner(&off_harness).cursor(), before_cursor);
        assert_eq!(selected_rect(&off_harness), before_paint);

        let mut harness = harness(width);
        draw(&mut harness, width);
        let before_cursor = feeds_owner(&harness).cursor();
        let before_paint = selected_rect(&harness);
        let list = list_rect(&harness);
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: list.x + 1,
            row: list.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(!outcome.raw_messages.is_empty());
        draw(&mut harness, width);
        // A claimed wheel over the control moves through the shared owner's
        // Wheel→Move path now that an identical sync no longer invalidates the
        // painted frame (the 6.1 `last_projected_rows` skip).
        assert_eq!(feeds_owner(&harness).cursor(), before_cursor + 1);
        // The claimed move re-selects the next row, so the painted selected-row
        // rect necessarily moves with it.
        assert_ne!(selected_rect(&harness), before_paint);
    }
}

#[test]
fn feeds_tick_round_trip_preserves_selected_target() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD - 1);
    assert_eq!(feeds_owner(&harness).cursor(), 1);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(feeds_owner(&harness).cursor(), 1);
}

#[test]
fn feeds_tick_has_one_painted_list_surface() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert!(selected_rect(&harness).is_some());
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD - 1);
    assert!(selected_rect(&harness).is_some());
}

/// Focus and mouse eligibility follow the registered Feeds owner through the
/// real sync pass: the Feeds tab routes framework focus and the mouse
/// subscription to the mounted `LibraryPanel`, and the migrated component id
/// no longer exists.
#[test]
fn feeds_tick_focus_and_mouse_eligibility_follow_the_panel() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    assert_eq!(
        harness.model().application.focus().cloned(),
        Some(ComponentId::Library),
        "the registered Feeds tab routes focus to the Library panel"
    );
    assert!(
        harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::Library),
        "the painted Feeds panel is mouse-eligible"
    );
}

/// A Watched pill click resolves through the panel's Selector slot into the
/// owner's filter — no destination-side offset mirror.
#[test]
fn feeds_tick_watched_pill_click_changes_the_filter_through_the_panel() {
    let mut harness = harness(240);
    draw(&mut harness, 240);
    assert_eq!(feeds_owner(&harness).watched_filter(), WatchedFilter::All);
    let watched = panel(&harness)
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, id)| *id == 1 + WatchedFilter::Watched.position())
        .map(|(rect, _)| *rect)
        .expect("the Watched pill is painted");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: watched.x + 1,
        row: watched.y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        feeds_owner(&harness).watched_filter(),
        WatchedFilter::Watched,
        "the painted Watched pill must set the owner's filter"
    );
}

/// A single row click resolves the painted row through the panel and emits
/// the owner's `FeedsRowClick` request; the row's stable target is selected.
#[test]
fn feeds_tick_latest_selection_uses_loaded_snapshot_without_fetch_and_refreshes_in_place() {
    let mut harness = harness(240);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(10),
            current: 20,
        };
    harness.model_mut().app.feed_tab.entries[0][0].pub_date_secs = Some(15);
    harness.model_mut().app.feed_tab.rebuild_all_entries();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 240);
    assert!(panel(&harness).test_selector_markers()[0]);
    let latest = panel(&harness)
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, id)| *id == 0)
        .map(|(rect, _)| *rect)
        .expect("Latest pill is painted");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: latest.x + 1,
        row: latest.y,
        modifiers: KeyModifiers::NONE,
    }));
    let selected = harness.step();
    assert!(selected.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::FeedsLatestSelected)
    )));
    assert!(!selected.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::RefreshFeeds)
    )));
    assert!(feeds_owner(&harness).latest_selected());
    let mut music_resize = false;
    let mut tv_resize = false;
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::FeedsLatestSelected),
        &mut music_resize,
        &mut tv_resize,
    );
    draw(&mut harness, 240);
    assert!(
        !panel(&harness).test_selector_markers()[0],
        "selecting Latest acknowledges the launch-window marker"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('r'),
        modifiers: KeyModifiers::NONE,
    }));
    let refresh = harness.step();
    assert!(refresh.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::RefreshFeeds)
    )));

    harness.model_mut().app.feed_tab.entries = vec![vec![
        entry("one", "One"),
        entry("two", "Two"),
        entry("three", "Three"),
    ]];
    harness.model_mut().app.feed_tab.rebuild_all_entries();
    harness.model_mut().sync_mounted_surfaces();
    assert!(feeds_owner(&harness).latest_selected());
    assert!(feeds_owner(&harness).visible_titles().contains(&"Three"));
}

#[test]
fn feeds_tick_row_click_through_the_panel_emits_feeds_row_click() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    let list = list_rect(&harness);
    // The first painted selectable row: scan up from the list bottom so a
    // structural Heading/Spacer at the top is skipped.
    let mut resolved = None;
    for row in (list.y..list.bottom()).rev() {
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: list.x + 1,
            row,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        if outcome
            .messages
            .iter()
            .any(|msg| matches!(msg, Msg::Shell(ShellRequest::FeedsRowClick)))
        {
            resolved = Some(row);
            break;
        }
    }
    assert!(
        resolved.is_some(),
        "a painted selectable row must resolve FeedsRowClick through the panel"
    );
}

/// The inactive Feeds owner keeps its cursor across a tab change: the owner
/// map's retention rule (design D2) is driven through the real sync pass.
#[test]
fn feeds_owner_retained_across_a_tab_change() {
    let mut harness = harness(crate::app::TWO_COLUMN_THRESHOLD);
    draw(&mut harness, crate::app::TWO_COLUMN_THRESHOLD);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(feeds_owner(&harness).cursor(), 1);
    assert!(panel(&harness).has_owner(&LibraryKey::Feeds));

    // Leave Feeds: the retained owner must survive the round trip.
    harness.model_mut().app.tab = TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    assert!(panel(&harness).has_owner(&LibraryKey::Feeds));

    harness.model_mut().app.tab = TabSelection::Feeds;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        feeds_owner(&harness).cursor(),
        1,
        "the retained Feeds owner keeps its cursor across a tab change"
    );
}
