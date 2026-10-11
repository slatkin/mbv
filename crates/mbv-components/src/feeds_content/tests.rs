use super::*;
use crate::library_panel::content::{ArtworkShape, ListSlot};
use crate::library_panel::owner::LibraryContentOwner;
use mbv_queue::FeedKind;

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

/// One Selector row carries watched-filter pills followed by one pill per
/// subscription (2026-10-10 user rule: the ungrouped scope has no pill of
/// its own — its highlight falls to its watched-filter pill, so the row
/// never paints two All choices).
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
        ["Latest", "All", "Played", "Unplayed", "A", "B"]
    );
    // A fresh owner opens on Latest (user rule, 2026-10-11).
    assert_eq!(selector.active, Some(0));
    drop(content);

    // The ungrouped scope highlights its watched-filter pill.
    owner.on_slot_event(crate::library_panel::owner::LibrarySlotEvent::SelectorPicked(1));
    let content = owner.content();
    assert_eq!(content.selector.as_ref().expect("pills").active, Some(1));
    // The panel's no-secondary-row contract is owned by the shared
    // `narrow_skeleton_keeps_fixed_rows_and_panel_slots` test.
    drop(content);

    owner.cycle_group(1);
    assert_eq!(owner.content().selector.unwrap().active, Some(4));
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
    // The ungrouped scope's highlight follows the active watched filter.
    assert_eq!(content.selector.unwrap().active, Some(2));
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
