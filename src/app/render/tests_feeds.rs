//! Feeds panel-output tests (tasks 7.2/7.3). Feeds paints through the shared
//! Wide/Narrow Library panel skeleton over `FeedsContent::content()`, so these
//! assertions target the mounted `LibraryPanel`'s own output — one Selector
//! pill bar and the policy hero header — instead of
//! the deleted `render_feeds_content` chrome or the deleted mounted Feeds
//! destination component.

use super::test_helpers::*;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
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
use tuirealm::props::{AttrValue, Attribute};

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

fn feed_owner_with_entries(entries: Vec<FeedEntry>) -> FeedsContent {
    let subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entries];
    let all_entries = entries[0].clone();
    let mut owner = FeedsContent::new();
    owner.set_content(FeedsOwnerPush {
        subscriptions,
        entries,
        all_entries,
        loading: false,
    });
    owner
}

fn feed_owner() -> FeedsContent {
    feed_owner_with_entries(vec![
        feed_entry("entry-1", "Entry One", false),
        feed_entry("entry-2", "Played Entry Two", true),
    ])
}

fn panel_with(owner: FeedsContent, focused: bool) -> LibraryPanel {
    let mut panel = LibraryPanel::new();
    panel.insert_owner(LibraryKey::Feeds, Box::new(owner));
    panel.set_active(Some(LibraryKey::Feeds));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(focused));
    panel
}

fn terminal_for(panel: &mut LibraryPanel, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(panel, frame, Rect::new(0, 0, width, height)))
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

/// The panel paints one Selector row containing watched-filter and feed-group
/// pills, plus the policy hero header — no destination chrome. The shared
/// skeleton owns the no-secondary-row contract.
#[test]
fn feeds_paints_one_pill_bar_and_the_policy_header() {
    let mut panel = panel_with(feed_owner(), true);
    let terminal = terminal_for(&mut panel, 240, 30);
    let wide = panel
        .test_wide_geometry()
        .expect("the panel painted a Wide skeleton");
    let output = buffer_to_string(&terminal);

    let group_hits = panel.test_selector_hits().regions();
    assert!(
        !group_hits.is_empty(),
        "the feed-group Selector pills must paint"
    );
    assert!(
        group_hits.iter().all(|(rect, _)| rect.height == 1),
        "the Selector row is one pill bar"
    );
    // The policy hero header paints the selected entry's title in the hero
    // pane (the artwork policy arm itself is covered by `feeds_content`).
    assert!(
        area_contains(terminal.backend().buffer(), wide.hero_area, "Entry One"),
        "the policy hero header must paint the selected entry title"
    );
    assert!(
        output.contains("Latest") && output.contains("Played") && output.contains("Unplayed"),
        "missing Latest and Watched filter pills: {output:?}"
    );
}

/// Clicking the painted Watched pill changes the filter through the panel's
/// Selector slot-event resolution.
#[test]
fn feeds_latest_paints_provider_date_subscription_title_marker_and_wide_hero() {
    for (width, height, wide) in [(240, 30, true), (80, 30, false)] {
        let mut owner =
            feed_owner_with_entries(vec![feed_entry("latest", "Latest Episode", false)]);
        owner.set_latest_marker(true);
        let mut panel = panel_with(owner, true);
        let _ = terminal_for(&mut panel, width, height);
        let (latest, _) = panel
            .test_selector_hits()
            .regions()
            .iter()
            .find(|(_, id)| *id == 0)
            .expect("Latest pill is painted");
        let click = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: latest.x,
            row: latest.y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(panel.on(&click).is_some());
        let terminal = terminal_for(&mut panel, width, height);
        let output = buffer_to_string(&terminal);
        assert!(output.contains("14 Nov"), "Latest date gutter: {output:?}");
        assert!(
            output.contains("Test Feed") && output.contains("Latest Episode"),
            "{output:?}"
        );
        assert!(output.contains('•'), "Latest selector marker: {output:?}");
        if wide {
            let geometry = panel.test_wide_geometry().expect("Wide panel");
            assert!(area_contains(
                terminal.backend().buffer(),
                geometry.hero_area,
                "Latest Episode"
            ));
        }
    }
}

#[test]
fn watched_pill_click_changes_the_filter() {
    let mut panel = panel_with(feed_owner(), true);
    let _terminal = terminal_for(&mut panel, 240, 30);
    let selector = panel.test_selector_hits().regions();
    let (watched, _) = selector
        .iter()
        .find(|(_, id)| *id == 1 + WatchedFilter::Watched.position())
        .expect("the Watched pill is painted");
    let watched_x = watched.x;
    let filter = |panel: &LibraryPanel| {
        panel
            .owner(&LibraryKey::Feeds)
            .and_then(|owner| owner.as_any().downcast_ref::<FeedsContent>())
            .map(FeedsContent::watched_filter)
    };
    assert_eq!(filter(&panel), Some(WatchedFilter::All));

    let msg = panel.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: watched_x + 1,
        row: watched.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(msg, None);
    assert_eq!(filter(&panel), Some(WatchedFilter::Watched));
}

/// The `w` chord still changes the filter through the panel's keyboard
/// forwarding.
#[test]
fn w_key_changes_the_filter() {
    let mut panel = panel_with(feed_owner(), true);
    let msg = panel.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(msg, None);
    let owner = panel
        .owner_mut(&LibraryKey::Feeds)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<FeedsContent>())
        .expect("Feeds owner installed");
    assert_eq!(owner.watched_filter(), WatchedFilter::Watched);
}

/// Without subscriptions the panel paints neither the Selector bar nor a
/// secondary row, only the empty-slot placeholder. Secondary-row absence is
/// owned by the shared skeleton characterization.
#[test]
fn feeds_without_subscriptions_paints_no_pill_bar() {
    let mut panel = panel_with(FeedsContent::new(), false);
    let terminal = terminal_for(&mut panel, 120, 30);
    assert!(
        panel.test_selector_hits().regions().is_empty(),
        "no Selector bar without subscriptions"
    );
    let output = buffer_to_string(&terminal);
    assert!(
        output.contains("No feed subscriptions configured"),
        "{output:?}"
    );
}
