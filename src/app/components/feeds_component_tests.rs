//! Feeds embedded-owner tests (tasks 7.1/7.3). These exercise `FeedsContent`
//! directly for the group/Watched-filter selection, the canonical row
//! projection, and the typed message translation; panel geometry and pointer
//! resolution are exercised through the mounted `LibraryPanel` that hosts the
//! owner (the destination component is deleted).

use super::feeds_content::{FeedsContent, FeedsOwnerPush};
use super::library_panel::{LibraryContentOwner, LibraryKey, LibraryPanel};
use super::media_list::MediaListRow;
use super::msg::{Msg, ShellRequest};
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute};

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

fn subscription(name: &str) -> FeedSubscription {
    FeedSubscription {
        name: name.into(),
        url: format!("https://example.test/{name}"),
        kind: FeedKind::Audio,
    }
}

fn owner_with(
    subscriptions: Vec<FeedSubscription>,
    entries: Vec<Vec<FeedEntry>>,
    all_entries: Vec<FeedEntry>,
) -> FeedsContent {
    let mut owner = FeedsContent::new();
    owner.set_content(FeedsOwnerPush {
        subscriptions,
        entries,
        all_entries,
        loading: false,
    });
    owner
}

fn component() -> FeedsContent {
    let subscriptions = vec![subscription("Test Feed")];
    let entries = vec![entry("First", false), entry("Second", true)];
    owner_with(subscriptions, vec![entries.clone()], entries)
}

fn grouped_component() -> FeedsContent {
    let subscriptions = vec![subscription("A"), subscription("B")];
    let entries = vec![
        vec![entry("A-unplayed", false), entry("A-played", true)],
        vec![entry("B-unplayed", false), entry("B-played", true)],
    ];
    let all_entries = entries.iter().flatten().cloned().collect::<Vec<_>>();
    owner_with(subscriptions, entries, all_entries)
}

fn panel_with(owner: FeedsContent, focused: bool) -> LibraryPanel {
    let mut panel = LibraryPanel::new();
    panel.insert_owner(LibraryKey::Feeds, Box::new(owner));
    panel.set_active(Some(LibraryKey::Feeds));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(focused));
    panel
}

fn paint(panel: &mut LibraryPanel, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(panel, frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

fn feeds(panel: &LibraryPanel) -> &FeedsContent {
    panel
        .owner(&LibraryKey::Feeds)
        .and_then(|owner| owner.as_any().downcast_ref::<FeedsContent>())
        .expect("Feeds owner installed")
}

fn feeds_mut(panel: &mut LibraryPanel) -> &mut FeedsContent {
    panel
        .owner_mut(&LibraryKey::Feeds)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<FeedsContent>())
        .expect("Feeds owner installed")
}

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn down(owner: &mut FeedsContent, code: Key) -> Option<Msg> {
    owner.on_key(&key(code))
}

#[test]
fn unfocused_panel_does_not_forward_keys_to_the_feeds_owner() {
    let mut panel = panel_with(component(), false);
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
            panel.on(&Event::<super::user_event::UserEvent>::Keyboard(key(code))),
            None
        );
    }
    assert_eq!(feeds(&panel).cursor(), 0);
    assert_eq!(feeds(&panel).scroll(), 0);
    assert_eq!(feeds(&panel).selected_group(), 0);
    assert_eq!(feeds(&panel).watched_filter(), WatchedFilter::All);
}

#[test]
fn down_moves_the_component_cursor_without_app_state() {
    let mut owner = component();
    let msg = down(&mut owner, Key::Down);

    assert_eq!(owner.cursor(), 1);
    assert_eq!(msg, None);
}

#[test]
fn watched_filter_rebuilds_the_component_visible_list() {
    let mut owner = component();
    down(&mut owner, Key::Char('w'));

    assert_eq!(owner.watched_filter(), WatchedFilter::Watched);
    assert_eq!(owner.visible_titles(), ["Second"]);
}

#[test]
fn visible_entries_all_group() {
    assert_eq!(component().visible_titles().len(), 2);
}

#[test]
fn visible_entries_subscription_group() {
    let mut owner = grouped_component();
    down(&mut owner, Key::Char(']'));
    assert_eq!(owner.visible_titles(), ["A-unplayed", "A-played"]);
}

#[test]
fn group_count_includes_all() {
    assert_eq!(component().group_count(), 2);
    assert_eq!(grouped_component().group_count(), 3);
}

#[test]
fn clamp_state_works() {
    let mut owner = component();
    owner.set_content(FeedsOwnerPush {
        subscriptions: Vec::new(),
        entries: Vec::new(),
        all_entries: Vec::new(),
        loading: false,
    });
    assert_eq!(owner.cursor(), 0);
    assert_eq!(owner.scroll(), 0);
}

#[test]
fn watched_filter_cycle_order() {
    let mut owner = component();
    for expected in [
        WatchedFilter::Watched,
        WatchedFilter::Unwatched,
        WatchedFilter::All,
    ] {
        down(&mut owner, Key::Char('w'));
        assert_eq!(owner.watched_filter(), expected);
    }
}

#[rstest]
#[case::watched_filter_shows_only_played(1, ["Second"])]
#[case::unwatched_filter_shows_only_unplayed(2, ["First"])]
fn watched_filter(#[case] key_presses: usize, #[case] expected: [&str; 1]) {
    let mut owner = component();
    for _ in 0..key_presses {
        down(&mut owner, Key::Char('w'));
    }
    assert_eq!(owner.visible_titles(), expected);
}

#[test]
fn watched_filter_empty_result() {
    let mut owner = owner_with(
        vec![subscription("Test Feed")],
        vec![vec![entry("First", false)]],
        vec![entry("First", false)],
    );
    down(&mut owner, Key::Char('w'));
    assert!(owner.visible_titles().is_empty());
}

#[test]
fn filter_cycle_resets_cursor_and_scroll() {
    let mut owner = component();
    down(&mut owner, Key::Down);
    down(&mut owner, Key::Char('w'));
    assert_eq!(owner.cursor(), 0);
    assert_eq!(owner.scroll(), 0);
}

#[test]
fn filter_applies_to_subscription_group() {
    let mut owner = grouped_component();
    down(&mut owner, Key::Char(']'));
    down(&mut owner, Key::Char('w'));
    assert_eq!(owner.visible_titles(), ["A-played"]);
}

#[test]
fn group_change_reflects_active_filter() {
    let mut owner = grouped_component();
    down(&mut owner, Key::Char('w'));
    down(&mut owner, Key::Char(']'));
    assert_eq!(owner.visible_titles(), ["A-played"]);
}

#[test]
fn unfocused_component_handles_mouse_input() {
    // The second group holds more entries than the painted narrow list shows,
    // so the claimed wheel can step the viewport (design D1).
    let subscriptions = vec![subscription("A"), subscription("B")];
    let entries = vec![
        (0..16)
            .map(|i| entry(&format!("A-{i}"), false))
            .collect::<Vec<_>>(),
        vec![entry("B-unplayed", false), entry("B-played", true)],
    ];
    let all_entries = entries.iter().flatten().cloned().collect::<Vec<_>>();
    let mut panel = panel_with(owner_with(subscriptions, entries, all_entries), false);
    let _ = paint(&mut panel, 60, 20);
    let selector = panel
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, id)| *id == WatchedFilter::COUNT + 1)
        .map(|(rect, _)| *rect)
        .expect("the second feed-group pill is painted");
    panel.on(&Event::Mouse(MouseEvent {
        column: selector.x,
        row: selector.y,
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    }));
    // A group change re-projects the list, so repaint before the wheel that
    // resolves the fresh retained frame (the legacy two-draw test shape).
    let _ = paint(&mut panel, 60, 20);
    let list = panel
        .test_narrow_geometry()
        .expect("narrow skeleton")
        .list_area;
    panel.on(&Event::Mouse(MouseEvent {
        column: list.x,
        row: list.y,
        kind: MouseEventKind::ScrollDown,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(feeds(&panel).selected_group(), 1);
    // The wheel steps the viewport one row (design D1): the window moves
    // while the selection — a display row below the group's `Heading`, not
    // on the window's edge — rides nowhere.
    assert_eq!(feeds(&panel).scroll(), 1);
    assert_eq!(feeds(&panel).cursor(), 0);
}

#[test]
fn mouse_owns_feed_selector_and_row_geometry() {
    let mut panel = panel_with(grouped_component(), true);
    let _ = paint(&mut panel, 60, 20);
    let selector = panel
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, id)| *id == WatchedFilter::COUNT + 1)
        .map(|(rect, _)| *rect)
        .expect("the second feed-group pill is painted");
    panel.on(&Event::Mouse(MouseEvent {
        column: selector.x,
        row: selector.y,
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(feeds(&panel).selected_group(), 1);
    assert_eq!(feeds(&panel).cursor(), 0);
}

#[test]
fn subscription_change_resets_component_selection() {
    let mut owner = grouped_component();
    down(&mut owner, Key::Down);
    assert_eq!(owner.cursor(), 1);
    owner.set_content(FeedsOwnerPush {
        subscriptions: vec![subscription("Replacement")],
        entries: vec![Vec::new()],
        all_entries: Vec::new(),
        loading: false,
    });
    assert_eq!(owner.selected_group(), 0);
    assert_eq!(owner.cursor(), 0);
    assert_eq!(owner.scroll(), 0);
}

#[test]
fn playback_requests_use_the_selected_entry_guid() {
    let subscriptions = vec![subscription("Test Feed")];
    let entries = vec![
        entry("Hidden", false),
        entry("Second", true),
        entry("Third", true),
    ];
    let mut owner = owner_with(subscriptions, vec![entries.clone()], entries);
    down(&mut owner, Key::Char('w'));
    down(&mut owner, Key::Down);

    assert_eq!(
        down(&mut owner, Key::Enter),
        Some(Msg::Shell(ShellRequest::FeedsPlay(vec![entry(
            "Third", true
        )])))
    );
    assert_eq!(
        down(&mut owner, Key::Char('e')),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue(vec![entry(
            "Third", true
        )])))
    );
}

#[test]
fn feed_actions_preserve_the_selected_entry_when_guids_collide() {
    let subscriptions = vec![subscription("A"), subscription("B")];
    let mut first = entry("First", false);
    first.guid = "shared-guid".into();
    first.feed_id = Some("https://example.test/a".into());
    let mut second = entry("Second", false);
    second.guid = "shared-guid".into();
    second.feed_id = Some("https://example.test/b".into());
    let entries = vec![vec![first.clone()], vec![second.clone()]];
    let all_entries = entries.iter().flatten().cloned().collect::<Vec<_>>();
    let mut owner = owner_with(subscriptions, entries, all_entries);
    for _ in 0..2 {
        down(&mut owner, Key::Char(']'));
    }

    assert_eq!(
        down(&mut owner, Key::Enter),
        Some(Msg::Shell(ShellRequest::FeedsPlay(vec![second.clone()])))
    );
    assert_eq!(
        down(&mut owner, Key::Char('e')),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue(vec![second])))
    );
}

#[test]
fn empty_feed_actions_request_shell_feedback() {
    let mut owner = FeedsContent::new();
    owner.set_content(FeedsOwnerPush {
        subscriptions: Vec::new(),
        entries: Vec::new(),
        all_entries: Vec::new(),
        loading: false,
    });

    assert_eq!(
        down(&mut owner, Key::Enter),
        Some(Msg::Shell(ShellRequest::FeedsPlay(Vec::new())))
    );
    assert_eq!(
        down(&mut owner, Key::Char('e')),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue(Vec::new())))
    );
}

#[test]
fn changing_group_invalidates_previous_row_geometry() {
    let mut panel = panel_with(grouped_component(), true);
    let _ = paint(&mut panel, 60, 20);
    let previous_row = panel
        .test_narrow_geometry()
        .expect("narrow skeleton")
        .selected
        .expect("the initial selected row is painted");

    down(feeds_mut(&mut panel), Key::Char(']'));
    feeds_mut(&mut panel).set_content(FeedsOwnerPush {
        subscriptions: Vec::new(),
        entries: Vec::new(),
        all_entries: Vec::new(),
        loading: false,
    });
    assert!(feeds(&panel)
        .resolve_row_id(Position::new(previous_row.x, previous_row.y))
        .is_none());
    down(feeds_mut(&mut panel), Key::Down);
    assert_eq!(feeds(&panel).cursor(), 0);
}

#[test]
fn unchanged_snapshot_does_not_overwrite_component_cursor() {
    let mut owner = component();
    down(&mut owner, Key::Down);
    let entries = vec![entry("First", false), entry("Second", true)];
    owner.set_content(FeedsOwnerPush {
        subscriptions: vec![subscription("Test Feed")],
        entries: vec![entries.clone()],
        all_entries: entries,
        loading: false,
    });

    assert_eq!(owner.cursor(), 1);
}

#[test]
fn feeds_render_without_app_state() {
    let mut panel = panel_with(component(), true);
    let terminal = paint(&mut panel, 60, 20);

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

fn dated_owner(entries: Vec<FeedEntry>) -> FeedsContent {
    let subscriptions = vec![subscription("Test Feed")];
    let grouped = vec![entries.clone()];
    owner_with(subscriptions, grouped, entries)
}

#[test]
fn structural_rows_are_non_selectable_and_cursor_movement_skips_them() {
    // Three entries in three distinct age groups -> the projected flow is
    // Heading/Item/Spacer/Heading/Item/Spacer/Heading/Item (8 display rows,
    // 3 selectable). Cursor movement addresses only the entries.
    let mut owner = dated_owner(vec![
        dated_entry("New One", false, 0),
        dated_entry("Recent One", false, 5),
        dated_entry("Old One", true, 40),
    ]);
    assert_eq!(owner.visible_titles(), ["New One", "Recent One", "Old One"]);
    assert_eq!(owner.cursor(), 0);
    assert_eq!(owner.canonical_selectable_len(), 3);
    assert_eq!(
        owner
            .canonical_rows()
            .iter()
            .filter(|row| matches!(row, MediaListRow::Heading { .. } | MediaListRow::Spacer))
            .count(),
        5
    );
    for expected in [1, 2, 2] {
        down(&mut owner, Key::Down);
        assert_eq!(owner.cursor(), expected);
    }
    for expected in [1, 0, 0] {
        down(&mut owner, Key::Up);
        assert_eq!(owner.cursor(), expected);
    }

    assert_eq!(owner.cursor(), 0);
    let mut panel = panel_with(owner, true);
    let _ = paint(&mut panel, crate::app::TWO_COLUMN_THRESHOLD, 30);
    assert!(panel
        .test_wide_geometry()
        .expect("wide skeleton")
        .selected
        .is_some());
}

#[test]
fn breakpoint_flip_carries_one_viewport_anchor() {
    let entries = (0..20)
        .map(|index| dated_entry(&format!("Entry {index:02}"), index == 15, 0))
        .collect();
    let mut owner = dated_owner(entries);
    for _ in 0..15 {
        down(&mut owner, Key::Down);
    }
    assert_eq!(owner.cursor(), 15);

    let wide = crate::app::TWO_COLUMN_THRESHOLD;
    let mut panel = panel_with(owner, true);
    let _ = paint(&mut panel, wide, 10);
    assert!(
        feeds(&panel)
            .painted_scroll()
            .is_some_and(|offset| offset > 0),
        "the wide display clamp shows the bottom selection",
    );

    // Breakpoint flip Wide -> Narrow: one ViewportAnchor carries the
    // selection and keeps it on screen.
    let narrow = wide - 1;
    let _ = paint(&mut panel, narrow, 10);
    assert_eq!(feeds(&panel).cursor(), 15);
    assert_eq!(
        feeds(&panel).canonical_selected_target(),
        Some(&"Entry 15".to_string())
    );
}

/// Task 4.1/4.5: a click on a list row resolves through the panel's slot
/// resolution into the owner's typed `FeedsRowClick`; a right-click is
/// ignored (task 4.6: no keyboard context-menu equivalent on this surface).
#[test]
fn feeds_mouse_click_resolves_row_and_right_click_opens_context_menu() {
    let mut panel = panel_with(component(), true);
    let _ = paint(&mut panel, 60, 20);
    let list = panel
        .test_narrow_geometry()
        .expect("narrow skeleton")
        .list_area;
    let click = |panel: &mut LibraryPanel, column: u16, row: u16, kind: MouseEventKind| {
        panel.on(&Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }))
    };
    let mut resolved = None;
    for row in (list.y..list.y + list.height).rev() {
        if click(
            &mut panel,
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
        feeds(&panel).cursor() > 0 || row < list.y + 4,
        "click must select the resolved row on both controls"
    );
    assert!(matches!(
        click(&mut panel, list.x, row, MouseEventKind::Down(MouseButton::Right)),
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Feeds(entries),
            Some(_),
        ))) if entries.len() == 1
    ));
}
