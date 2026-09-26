use super::*;
use crate::app::components::library_panel::content::{ArtworkShape, ListSlot};
use crate::app::components::library_panel::owner::LibraryContentOwner;
use mbv_core::config::FeedKind;

fn subscription(name: &str) -> FeedSubscription {
    FeedSubscription {
        name: name.into(),
        url: format!("https://example.test/{name}"),
        kind: FeedKind::Audio,
    }
}

fn entry(guid: &str, kind: FeedKind, played: bool) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(kind),
        feed_id: None,
        position_ticks: 0,
        played,
    }
}

fn owner(subscriptions: &[FeedSubscription], entries: Vec<FeedEntry>) -> FeedsContent {
    let mut owner = FeedsContent::new();
    let grouped = vec![entries.clone()];
    owner.set_content(FeedsOwnerPush {
        subscriptions: subscriptions.to_vec(),
        entries: grouped,
        all_entries: entries,
        loading: false,
    });
    owner
}

/// One Selector row carries watched-filter pills followed by feed groups.
#[test]
fn content_has_one_selector_row_for_filter_and_groups() {
    let mut owner = owner(
        &[subscription("A"), subscription("B")],
        vec![entry("one", FeedKind::Audio, false)],
    );
    let content = owner.content();
    let selector = content.selector.as_ref().expect("feed-group pills");
    assert_eq!(
        selector.pills,
        ["Latest", "All", "Played", "Unplayed", "All", "A", "B"]
    );
    assert_eq!(selector.active, Some(4));
    // The panel's no-secondary-row contract is owned by the shared
    // `narrow_skeleton_keeps_fixed_rows_and_panel_slots` test.
    drop(content);

    owner.cycle_group(1);
    assert_eq!(owner.content().selector.unwrap().active, Some(5));
}

/// Without subscriptions the legacy chrome painted no pill bar at all:
/// the Selector row is absent; the shared panel owns the row structure.
#[test]
fn content_omits_selector_row_without_subscriptions() {
    let mut owner = FeedsContent::new();
    owner.set_content(FeedsOwnerPush {
        subscriptions: Vec::new(),
        entries: Vec::new(),
        all_entries: Vec::new(),
        loading: false,
    });
    let content = owner.content();
    assert!(content.selector.is_none());
    // Secondary-row absence is covered by the shared panel skeleton test;
    // this owner test only covers Feeds' empty-state slot projection.
    match content.list {
        ListSlot::Empty { loading, text } => {
            assert!(!loading);
            assert_eq!(text, " No feed subscriptions configured");
        }
        _ => panic!("expected the unconfigured placeholder"),
    }
}

/// The watched pill state remains owner-local, and an empty filtered list
/// still renders the selector plus the reload placeholder.
#[test]
fn content_selector_follows_the_active_watched_filter() {
    let mut owner = owner(
        &[subscription("A")],
        vec![entry("unplayed", FeedKind::Audio, false)],
    );
    owner.cycle_watched_filter();
    assert_eq!(owner.watched_filter, WatchedFilter::Watched);
    let content = owner.content();
    assert_eq!(content.selector.unwrap().active, Some(4));
    match content.list {
        ListSlot::Empty { loading, text } => {
            assert!(!loading);
            assert_eq!(text, " Press r to load feeds");
        }
        _ => panic!("expected the empty-filter placeholder"),
    }
}

/// A selected podcast feed entry is Square with the placeholder (design
/// D5: Feeds entries declare no artwork source).
#[test]
fn selected_podcast_entry_hero_is_square_with_the_placeholder() {
    let mut owner = owner(
        &[subscription("A")],
        vec![entry("episode", FeedKind::Audio, false)],
    );
    let hero = owner.content().hero.expect("selected entry hero");
    assert_eq!(hero.facts.artwork.shape, ArtworkShape::Square);
    assert!(hero.facts.artwork.source.is_none());
    assert_eq!(hero.facts.title, "episode");
}
