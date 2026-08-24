use super::feeds::FeedsComponent;
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
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
fn down_moves_the_component_cursor_without_app_state() {
    let mut component = component();
    let msg = component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(component.cursor(), 1);
    assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
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
        Some(Msg::Shell(ShellRequest::FeedsPlay("Third".into())))
    );
    assert_eq!(
        component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
            code: Key::Char('e'),
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::FeedsEnqueue("Third".into())))
    );
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
