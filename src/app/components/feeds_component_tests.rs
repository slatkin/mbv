use super::feeds::FeedsComponent;
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn entry(title: &str, played: bool) -> FeedEntry {
    FeedEntry {
        guid: title.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.test/{title}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played,
    }
}

fn component() -> FeedsComponent {
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entry("First", false), entry("Second", true)];
    let grouped_entries = vec![entries];
    let mut component = FeedsComponent::new();
    component.set_content(
        &subscriptions,
        &grouped_entries,
        &grouped_entries[0],
        false,
        true,
    );
    component
}

fn grouped_component() -> FeedsComponent {
    let subscriptions = [
        FeedSubscription {
            name: "A".into(),
            url: "https://example.test/a".into(),
            kind: FeedKind::Audio,
        },
        FeedSubscription {
            name: "B".into(),
            url: "https://example.test/b".into(),
            kind: FeedKind::Audio,
        },
    ];
    let entries = vec![
        vec![entry("A-unplayed", false), entry("A-played", true)],
        vec![entry("B-unplayed", false), entry("B-played", true)],
    ];
    let all_entries = entries.iter().flatten().cloned().collect::<Vec<_>>();
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &entries, &all_entries, false, true);
    component
}

#[test]
fn unfocused_component_ignores_keyboard_input() {
    let mut component = component();
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entry("First", false), entry("Second", true)];
    component.set_content(
        &subscriptions,
        std::slice::from_ref(&entries),
        &entries,
        false,
        false,
    );
    let keys = [
        Key::Char('r'),
        Key::Char('w'),
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::PageUp,
        Key::PageDown,
        Key::Home,
        Key::End,
        Key::Char('['),
        Key::Char(']'),
        Key::Enter,
        Key::Char('e'),
    ];

    for code in keys {
        assert_eq!(
            component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            })),
            None
        );
    }
    assert_eq!(component.cursor(), 0);
    assert_eq!(component.scroll(), 0);
    assert_eq!(component.selected_group(), 0);
    assert_eq!(component.watched_filter(), WatchedFilter::All);
}

#[test]
fn down_moves_the_component_cursor_without_app_state() {
    let mut component = component();
    let msg = component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(component.cursor(), 1);
    assert_eq!(msg, None);
}

#[test]
fn watched_filter_rebuilds_the_component_visible_list() {
    let mut component = component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(component.watched_filter(), WatchedFilter::Watched);
    assert_eq!(component.visible_titles(), ["Second"]);
}

#[test]
fn visible_entries_all_group() {
    assert_eq!(component().visible_titles().len(), 2);
}

#[test]
fn visible_entries_subscription_group() {
    let mut component = grouped_component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.visible_titles(), ["A-unplayed", "A-played"]);
}

#[test]
fn group_count_includes_all() {
    assert_eq!(component().group_count(), 2);
    assert_eq!(grouped_component().group_count(), 3);
}

#[test]
fn clamp_state_works() {
    let mut component = component();
    component.set_content(&[], &[], &[], false, true);
    assert_eq!(component.cursor(), 0);
    assert_eq!(component.scroll(), 0);
}

#[test]
fn watched_filter_cycle_order() {
    let mut component = component();
    for expected in [
        WatchedFilter::Watched,
        WatchedFilter::Unwatched,
        WatchedFilter::All,
    ] {
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('w'),
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.watched_filter(), expected);
    }
}

#[test]
fn watched_filter_shows_only_played() {
    let mut component = component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.visible_titles(), ["Second"]);
}

#[test]
fn unwatched_filter_shows_only_unplayed() {
    let mut component = component();
    for _ in 0..2 {
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('w'),
            modifiers: KeyModifiers::NONE,
        }));
    }
    assert_eq!(component.visible_titles(), ["First"]);
}

#[test]
fn watched_filter_empty_result() {
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![vec![entry("First", false)]];
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &entries, &entries[0], false, true);
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.visible_titles().is_empty());
}

#[test]
fn filter_cycle_resets_cursor_and_scroll() {
    let mut component = component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.cursor(), 0);
    assert_eq!(component.scroll(), 0);
}

#[test]
fn filter_applies_to_subscription_group() {
    let mut component = grouped_component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.visible_titles(), ["A-played"]);
}

#[test]
fn group_change_reflects_active_filter() {
    let mut component = grouped_component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.visible_titles(), ["A-played"]);
}

#[test]
fn unfocused_component_handles_mouse_input() {
    let mut component = grouped_component();
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entry("First", false), entry("Second", true)];
    component.set_content(
        &subscriptions,
        std::slice::from_ref(&entries),
        &entries,
        false,
        false,
    );
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();
    let selector = component.layout().selector_tabs[1].0;
    component.on(&Event::<UserEvent>::Mouse(MouseEvent {
        column: selector.x,
        row: selector.y,
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::<UserEvent>::Mouse(MouseEvent {
        column: component.layout().left_area.x,
        row: component.layout().left_area.y,
        kind: MouseEventKind::ScrollDown,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.selected_group(), 1);
    assert_eq!(component.cursor(), 1);
}

#[test]
fn mouse_owns_feed_selector_and_row_geometry() {
    let mut component = grouped_component();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();
    let selector = component.layout().selector_tabs[1].0;
    component.on(&Event::<UserEvent>::Mouse(MouseEvent {
        column: selector.x,
        row: selector.y,
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.selected_group(), 1);
    assert_eq!(component.cursor(), 0);
}

#[test]
fn subscription_change_resets_component_selection() {
    let mut component = grouped_component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.cursor(), 1);
    let subscriptions = [FeedSubscription {
        name: "Replacement".into(),
        url: "https://example.test/replacement".into(),
        kind: FeedKind::Audio,
    }];
    component.set_content(&subscriptions, &[Vec::new()], &[], false, true);
    assert_eq!(component.selected_group(), 0);
    assert_eq!(component.cursor(), 0);
    assert_eq!(component.scroll(), 0);
}

#[test]
fn playback_requests_use_the_selected_entry_guid() {
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![
        entry("Hidden", false),
        entry("Second", true),
        entry("Third", true),
    ];
    let mut component = FeedsComponent::new();
    let grouped_entries = vec![entries.clone()];
    component.set_content(&subscriptions, &grouped_entries, &entries, false, true);
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsPlay(Some(entry(
            "Third", true
        )))))
    );
    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('e'),
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue(Some(entry(
            "Third", true
        )))))
    );
}

#[test]
fn feed_actions_preserve_the_selected_entry_when_guids_collide() {
    let subscriptions = [
        FeedSubscription {
            name: "A".into(),
            url: "https://example.test/a".into(),
            kind: FeedKind::Audio,
        },
        FeedSubscription {
            name: "B".into(),
            url: "https://example.test/b".into(),
            kind: FeedKind::Audio,
        },
    ];
    let mut first = entry("First", false);
    first.guid = "shared-guid".into();
    first.feed_id = Some("https://example.test/a".into());
    let mut second = entry("Second", false);
    second.guid = "shared-guid".into();
    second.feed_id = Some("https://example.test/b".into());
    let entries = vec![vec![first.clone()], vec![second.clone()]];
    let all_entries = entries.iter().flatten().cloned().collect::<Vec<_>>();
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &entries, &all_entries, false, true);
    for _ in 0..2 {
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }));
    }

    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsPlay(Some(second.clone()))))
    );
    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('e'),
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue(Some(second))))
    );
}

#[test]
fn empty_feed_actions_request_shell_feedback() {
    let mut component = FeedsComponent::new();
    component.set_content(&[], &[], &[], false, true);

    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsPlay(None)))
    );
    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('e'),
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue(None)))
    );
}

#[test]
fn changing_group_invalidates_previous_row_geometry() {
    let mut component = grouped_component();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();
    assert!(!component.layout().left_item_rows.is_empty());

    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.layout().left_item_rows.is_empty());
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.cursor(), 1);
}

#[test]
fn wide_feeds_keep_the_list_out_of_the_inline_hero_flow() {
    let mut wide = component();
    let mut wide_terminal =
        Terminal::new(TestBackend::new(crate::app::TWO_COLUMN_THRESHOLD, 20)).unwrap();
    wide_terminal
        .draw(|frame| wide.view(frame, Rect::new(0, 0, crate::app::TWO_COLUMN_THRESHOLD, 20)))
        .unwrap();
    assert_eq!(wide.layout().inline_hero_area, Rect::default());
    let wide_item_rows = wide
        .layout()
        .left_item_rows
        .iter()
        .filter(|row| !row.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(wide_item_rows, vec![vec![0], vec![1]]);

    let mut narrow = component();
    let width = crate::app::TWO_COLUMN_THRESHOLD - 1;
    let mut narrow_terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
    narrow_terminal
        .draw(|frame| narrow.view(frame, Rect::new(0, 0, width, 20)))
        .unwrap();
    assert!(narrow.layout().inline_hero_area.height > 0);
}

#[test]
fn unchanged_snapshot_does_not_overwrite_component_cursor() {
    let mut component = component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entry("First", false), entry("Second", true)];
    let grouped_entries = vec![entries.clone()];
    component.set_content(&subscriptions, &grouped_entries, &entries, false, true);

    assert_eq!(component.cursor(), 1);
}

#[test]
fn feeds_render_without_app_state() {
    let mut component = component();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(output.contains("Test Feed"));
    assert!(output.contains("First"));
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn dated_entry(title: &str, played: bool, days_ago: u64) -> FeedEntry {
    FeedEntry {
        pub_date_secs: Some(now_secs() - days_ago * 86_400),
        ..entry(title, played)
    }
}

fn dated_component(entries: Vec<FeedEntry>) -> FeedsComponent {
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let grouped = vec![entries.clone()];
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &grouped, &entries, false, true);
    component
}

#[test]
fn structural_rows_are_non_selectable_and_cursor_movement_skips_them() {
    // Three entries in three distinct age groups -> the projected flow is
    // Heading/Item/Spacer/Heading/Item/Spacer/Heading/Item (8 display rows,
    // 3 selectable). Cursor movement addresses only the entries.
    let mut component = dated_component(vec![
        dated_entry("New One", false, 0),
        dated_entry("Recent One", false, 5),
        dated_entry("Old One", true, 40),
    ]);
    assert_eq!(
        component.visible_titles(),
        ["New One", "Recent One", "Old One"]
    );
    assert_eq!(component.cursor(), 0);
    for expected in [1, 2, 2] {
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.cursor(), expected);
    }
    for expected in [1, 0, 0] {
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Up,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.cursor(), expected);
    }

    let mut terminal =
        Terminal::new(TestBackend::new(crate::app::TWO_COLUMN_THRESHOLD, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, crate::app::TWO_COLUMN_THRESHOLD, 30)))
        .unwrap();
    let item_rows = &component.layout().left_item_rows;
    assert_eq!(
        item_rows.iter().filter(|row| !row.is_empty()).count(),
        3,
        "three selectable entries: {item_rows:?}"
    );
    assert!(
        item_rows.len() > 3,
        "structural rows occupy display rows without a selectable index: {item_rows:?}"
    );
}

#[test]
fn breakpoint_flip_carries_one_viewport_anchor() {
    let entries = (0..20)
        .map(|index| dated_entry(&format!("Entry {index:02}"), index == 15, 0))
        .collect();
    let mut component = dated_component(entries);
    for _ in 0..15 {
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    }
    assert_eq!(component.cursor(), 15);

    let wide = crate::app::TWO_COLUMN_THRESHOLD;
    Terminal::new(TestBackend::new(wide, 10))
        .unwrap()
        .draw(|frame| component.view(frame, Rect::new(0, 0, wide, 10)))
        .unwrap();
    assert!(
        component.scroll() > 0,
        "wide viewport scrolled to the selection"
    );

    // Breakpoint flip Wide -> Narrow: the cursors stay in lockstep and the
    // single ViewportAnchor keeps the selection on screen.
    let narrow = wide - 1;
    Terminal::new(TestBackend::new(narrow, 10))
        .unwrap()
        .draw(|frame| component.view(frame, Rect::new(0, 0, narrow, 10)))
        .unwrap();
    assert_eq!(component.cursor(), 15);
    assert!(
        component.layout().left_row_map.contains(&Some(15)),
        "selected entry stays visible after the flip: {:?}",
        component.layout().left_row_map
    );
}

/// Task 4.1/4.5: a click on a list row resolves through the active canonical
/// control's `resolve_point`, selects the row on both controls, and emits the
/// `FeedsRowClick` focus request. A right-click is ignored (task 4.6: no
/// keyboard context-menu equivalent on this surface).
#[test]
fn feeds_mouse_click_resolves_row_and_right_click_is_ignored() {
    let mut component = component();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();
    let list = component.layout().left_area;
    // Scan for a painted selectable row below the hero-covered top rows; the
    // gesture recognizer is reset between probes.
    let click = |component: &mut FeedsComponent, column: u16, row: u16, kind: MouseEventKind| {
        component.on(&Event::<UserEvent>::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }))
    };
    let mut resolved = None;
    for row in (list.y..list.y + list.height).rev() {
        component.reset_mouse_gestures_for_test();
        if click(
            &mut component,
            list.x,
            row,
            MouseEventKind::Down(MouseButton::Left),
        )
        .is_some()
        {
            resolved = Some(row);
            break;
        }
    }
    let row = resolved.expect("a painted selectable row must resolve FeedsRowClick");
    assert!(
        component.cursor() > 0 || row < list.y + 4,
        "click must select the resolved row on both controls"
    );
    component.reset_mouse_gestures_for_test();
    assert_eq!(
        click(
            &mut component,
            list.x,
            row,
            MouseEventKind::Down(MouseButton::Right)
        ),
        None,
        "task 4.6: right-click must be ignored on this surface"
    );
}

/// Task 4.1: a double-click on a list row plays the resolved entry through
/// the existing `FeedsPlay` request.
#[test]
fn feeds_mouse_double_click_plays_the_resolved_entry() {
    let mut component = component();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();
    let list = component.layout().left_area;
    // Two quick Downs at the same painted row = DoubleClick on the second;
    // scan for a row whose double-click resolves a played entry.
    for row in (list.y..list.y + list.height).rev() {
        component.reset_mouse_gestures_for_test();
        let mut played = None;
        for _ in 0..2 {
            let msg = component.on(&Event::<UserEvent>::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: list.x,
                row,
                modifiers: KeyModifiers::NONE,
            }));
            if let Some(Msg::Shell(ShellRequest::FeedsPlay(Some(entry)))) = msg {
                played = Some(entry);
            }
        }
        if let Some(entry) = played {
            assert_eq!(entry.guid, "Second");
            return;
        }
    }
    panic!("double-click on a painted row must emit FeedsPlay(Some(entry))");
}
