//! Feeds panel-output tests (task 7.2). Feeds paints through the shared
//! Wide/Narrow Library panel skeleton over `FeedsContent::content()`, so these
//! assertions target the panel's own output — one Selector pill bar, one List
//! controls row, and the policy hero header — instead of the deleted
//! `render_feeds_content` chrome.

use super::test_helpers::*;
use crate::app::components::FeedsComponent;
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn feed_entry(guid: &str, title: &str, played: bool) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: Some(format!("https://example.test/{guid}")),
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(65 * TICKS_PER_SECOND as u64),
        pub_date_secs: Some(1_700_000_000),
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("https://example.test/feed".into()),
        position_ticks: if played { 42 } else { 0 },
        played,
    }
}

fn feed_component_with_entries(entries: Vec<FeedEntry>) -> FeedsComponent {
    let subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entries];
    let all_entries = entries[0].clone();
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &entries, &all_entries, false);
    component.set_focused(true);
    component
}

fn feed_component() -> FeedsComponent {
    feed_component_with_entries(vec![
        feed_entry("entry-1", "Entry One", false),
        feed_entry("entry-2", "Played Entry Two", true),
    ])
}

fn terminal_for(component: &mut FeedsComponent, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

/// Whether `needle` appears in any single buffer row inside `area`.
fn area_contains(buf: &ratatui::buffer::Buffer, area: Rect, needle: &str) -> bool {
    (area.top()..area.bottom()).any(|y| {
        let line: String = (area.left()..area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect();
        line.contains(needle)
    })
}

/// The panel paints the Selector row (feed groups), the List controls row
/// (Watched filter) and the policy hero header — one pill bar per row, no
/// destination chrome.
#[test]
fn feeds_paints_one_pill_bar_one_controls_row_and_the_policy_header() {
    let mut component = feed_component();
    let terminal = terminal_for(&mut component, 120, 30);
    let layout = component.layout().clone();
    let output = buffer_to_string(&terminal);

    let rows: std::collections::BTreeSet<u16> = layout
        .selector_tabs
        .iter()
        .map(|(rect, _)| rect.y)
        .collect();
    assert_eq!(
        rows.len(),
        2,
        "one Selector bar + one List controls row: {rows:?}"
    );
    assert!(layout
        .selector_tabs
        .iter()
        .all(|(rect, _)| rect.height == 1));
    let group_row = *rows.iter().next().expect("group row");
    let controls_row = *rows.iter().next_back().expect("controls row");
    assert!(group_row < controls_row);
    assert_eq!(
        layout
            .selector_tabs
            .iter()
            .filter(|(rect, _)| rect.y == controls_row)
            .count(),
        3,
        "the Watched List controls row is one bar of three pills"
    );

    // The policy hero header paints the selected entry's title in the hero
    // pane (the artwork policy arm itself is covered by `feeds_content`).
    assert!(
        area_contains(terminal.backend().buffer(), layout.hero_area, "Entry One"),
        "the policy hero header must paint the selected entry title"
    );
    assert!(output.contains("Test Feed"), "missing feed-group pill");
    assert!(
        output.contains("Played") && output.contains("Unplayed"),
        "missing Watched filter pills: {output:?}"
    );
}

/// Clicking the painted Watched pill changes the filter through the
/// component's own retained pill hit store.
#[test]
fn watched_pill_click_changes_the_filter() {
    let mut component = feed_component();
    let _ = terminal_for(&mut component, 120, 30);
    let layout = component.layout().clone();
    let filter_base = component.group_count();
    let watched = layout
        .selector_tabs
        .iter()
        .find(|(_, id)| *id == filter_base + WatchedFilter::Watched.position())
        .map(|(rect, _)| *rect)
        .expect("the Watched pill is painted");

    let msg = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: watched.x + 1,
        row: watched.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(msg, None);
    assert_eq!(component.watched_filter(), WatchedFilter::Watched);
}

/// The `w` chord still changes the filter on the panel-painted surface.
#[test]
fn w_key_changes_the_filter() {
    let mut component = feed_component();
    let msg = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(msg, None);
    assert_eq!(component.watched_filter(), WatchedFilter::Watched);
}

/// Narrow derives the inline hero from the same `HeroContent`; `hero_area`
/// and `inline_hero_area` publish the admitted block.
#[test]
fn narrow_feeds_render_the_inline_hero_from_the_same_content() {
    let mut component = feed_component();
    let width = crate::app::TWO_COLUMN_THRESHOLD - 1;
    let terminal = terminal_for(&mut component, width, 20);
    let layout = component.layout().clone();
    assert!(layout.inline_hero_area.height > 0);
    assert_eq!(layout.inline_hero_area, layout.hero_area);
    let output = buffer_to_string(&terminal);
    assert!(output.contains("Test Feed"));
    assert!(output.contains("Entry One"));
}

/// Without subscriptions the panel paints neither the Selector bar nor the
/// List controls row, only the empty-slot placeholder.
#[test]
fn feeds_without_subscriptions_paints_no_pill_bar() {
    let mut component = FeedsComponent::new();
    let terminal = terminal_for(&mut component, 60, 20);
    let layout = component.layout().clone();
    assert!(layout.selector_tabs.is_empty());
    let output = buffer_to_string(&terminal);
    assert!(
        output.contains("No feed subscriptions configured"),
        "{output:?}"
    );
}
