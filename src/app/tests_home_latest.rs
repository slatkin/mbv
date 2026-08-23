//! Part 1 of #543: Home's population is decoupled from Emby's connection
//! state. `fetch_home()` no longer errors without an Emby client, and both
//! Emby writers (sync `fetch_home()` and async `apply_emby_bootstrap()`)
//! merge their entries by `HomeLatestSource` instead of replacing Home's
//! data outright, so entries from other providers survive.

use crate::app::tests::*;
use crate::app::types_feed_tab::WatchedFilter;
use crate::app::types_playback::HomeLatestSource;
use mbv_core::audiobookshelf::{
    AudiobookshelfLibrary, AudiobookshelfShelf, AudiobookshelfShelfEntry,
};
use mbv_core::playback_queue::{AudiobookshelfQueueItem, FeedEntry, QueueItem};
use mbv_core::service_runtime::{EmbyBootstrap, EmbyLatestSection};

fn feed_item(title: &str) -> FeedEntry {
    FeedEntry {
        guid: format!("guid-{title}"),
        title: title.into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

fn abs_episode(id: &str) -> AudiobookshelfQueueItem {
    AudiobookshelfQueueItem {
        library_item_id: format!("show-{id}"),
        episode_id: format!("episode-{id}"),
        title: format!("Episode {id}"),
        show_title: Some("Podcast".into()),
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    }
}

fn abs_library(id: &str, media_type: &str) -> AudiobookshelfLibrary {
    AudiobookshelfLibrary {
        id: id.into(),
        name: id.into(),
        media_type: media_type.into(),
    }
}

#[test]
fn fetch_home_with_no_emby_preserves_feeds_and_abs_entries_without_error() {
    let mut app = make_app_stub();
    assert!(
        app.emby_client().is_none(),
        "stub must have no Emby Service"
    );

    // A Feeds subscription with entries and an Audiobookshelf library with a
    // cached shelf sit in Home before the refresh (what Parts 2/3 write).
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab.entries = vec![vec![feed_item("Feed one")]];
    app.feed_tab.rebuild_all_entries();
    app.audiobookshelf_libraries = vec![abs_library("abs-pod", "podcast")];
    let shelves = vec![AudiobookshelfShelf {
        label: "Newest episodes".into(),
        entries: vec![AudiobookshelfShelfEntry::Episode(abs_episode("1"))],
    }];
    app.audiobookshelf_shelf_cache.insert(
        "abs-pod".into(),
        crate::app::App::newest_episodes_items(shelves),
    );

    let result = app.fetch_home();
    assert!(result.is_ok(), "no-Emby refresh must not fail");

    // Feeds and Audiobookshelf pills are both rebuilt from their sources.
    assert_eq!(app.home.latest.len(), 2, "both non-Emby pills must exist");
    let feeds = app
        .home
        .latest
        .iter()
        .find(|(_, source, _, _)| matches!(source, HomeLatestSource::Feeds))
        .expect("Feeds pill must survive");
    assert_eq!(feeds.2.len(), 1);
    assert_eq!(feeds.2[0].display_name(), "Feed one");
    assert!(app
        .home
        .latest
        .iter()
        .any(|(_, source, _, _)| matches!(source, HomeLatestSource::Audiobookshelf(_))));

    // Second refresh: still no Emby error, both pills intact.
    assert!(app.fetch_home().is_ok());
    let feeds = app
        .home
        .latest
        .iter()
        .find(|(_, source, _, _)| matches!(source, HomeLatestSource::Feeds))
        .expect("Feeds pill must survive the second refresh");
    assert_eq!(feeds.2.len(), 1);
}

#[test]
fn apply_emby_bootstrap_merges_only_emby_entries() {
    let mut app = make_app_stub();

    // Pre-existing mixed Home: an Audiobookshelf and a Feeds entry alongside
    // an Emby entry (view id "movies") that the bootstrap will replace.
    // Canonical pill order (Emby, Audiobookshelf, Feeds) is applied by the
    // merge regardless of this arrival order.
    app.home.latest = vec![
        (
            "Podcasts".into(),
            HomeLatestSource::Audiobookshelf("abs-lib".into()),
            Vec::new(),
            0,
        ),
        (
            "Feeds".into(),
            HomeLatestSource::Feeds,
            vec![QueueItem::Feed(feed_item("Feed one"))],
            1,
        ),
        (
            "Movies".into(),
            HomeLatestSource::Emby("movies".into()),
            vec![QueueItem::Emby(Box::new(make_item("Old", "Movie")))],
            0,
        ),
    ];

    app.apply_emby_bootstrap(EmbyBootstrap {
        continue_items: Vec::new(),
        views: Vec::new(),
        latest: vec![EmbyLatestSection {
            title: "Movies".into(),
            view_id: "movies".into(),
            items: vec![make_item("New", "Movie")],
        }],
    });

    assert_eq!(
        app.home.latest.len(),
        3,
        "Emby entry replaced, no duplicates"
    );
    // Canonical pill order is Emby, then Audiobookshelf, then Feeds —
    // regardless of arrival order.
    assert!(matches!(
        &app.home.latest[0].1,
        HomeLatestSource::Emby(id) if id == "movies"
    ));
    assert_eq!(app.home.latest[0].2.len(), 1);
    assert_eq!(app.home.latest[0].2[0].display_name(), "New");
    assert!(matches!(
        &app.home.latest[1].1,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-lib"
    ));
    assert!(matches!(&app.home.latest[2].1, HomeLatestSource::Feeds));
    assert_eq!(app.home.latest[2].2.len(), 1);
}

#[test]
fn apply_emby_bootstrap_without_prior_data_populates_emby_only_path() {
    let mut app = make_app_stub();
    assert!(app.home.latest.is_empty());

    app.apply_emby_bootstrap(EmbyBootstrap {
        continue_items: Vec::new(),
        views: Vec::new(),
        latest: vec![
            EmbyLatestSection {
                title: "Movies".into(),
                view_id: "v1".into(),
                items: vec![
                    make_item("Movie one", "Movie"),
                    make_item("Movie two", "Movie"),
                ],
            },
            EmbyLatestSection {
                title: "Shows".into(),
                view_id: "v2".into(),
                items: vec![make_item("Episode one", "Episode")],
            },
        ],
    });

    assert_eq!(app.home.latest.len(), 2);
    assert!(matches!(
        &app.home.latest[0].1,
        HomeLatestSource::Emby(id) if id == "v1"
    ));
    assert_eq!(app.home.latest[0].2.len(), 2);
    assert_eq!(app.home.latest[0].0, "Movies");
    assert!(matches!(
        &app.home.latest[1].1,
        HomeLatestSource::Emby(id) if id == "v2"
    ));
    assert_eq!(app.home.latest[1].2.len(), 1);
    assert!(!app.home_loading);
}

/// Part 2 of #543, Tasks 6.3/7.1/7.2: the shelf-fetch handler caches the
/// `Newest Episodes` items per podcast library and rebuilds Home's pill from
/// the cache; `fetch_home()` rebuilds it again with no network fetch,
/// re-applying `hidden_latest`; book libraries never get a pill.
#[test]
fn shelf_cache_drives_and_hides_audiobookshelf_pills_without_fetching() {
    let mut app = make_app_stub();
    app.audiobookshelf_libraries = vec![
        abs_library("abs-pod", "podcast"),
        abs_library("abs-books", "book"),
    ];

    // The recorded live-server shape: a non-recency shelf plus `Newest
    // episodes`; only the latter feeds Home (Task 6.2/6.3).
    let shelves = vec![
        AudiobookshelfShelf {
            label: "Continue listening".into(),
            entries: vec![AudiobookshelfShelfEntry::Show("show-9".into())],
        },
        AudiobookshelfShelf {
            label: "Newest episodes".into(),
            entries: vec![AudiobookshelfShelfEntry::Episode(abs_episode("1"))],
        },
    ];
    app.audiobookshelf_shelf_cache.insert(
        "abs-pod".into(),
        crate::app::App::newest_episodes_items(shelves),
    );
    app.rebuild_audiobookshelf_latest();

    assert_eq!(
        app.home.latest.len(),
        1,
        "only the podcast library gets a pill"
    );
    let (title, source, items, cursor) = &app.home.latest[0];
    assert_eq!(title, "abs-pod");
    assert!(matches!(
        source,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-pod"
    ));
    assert_eq!(
        items.len(),
        1,
        "show entries and other shelves never reach Home"
    );
    assert_eq!(items[0].display_name(), "Podcast - Episode 1");
    assert_eq!(*cursor, 0);

    // `fetch_home()` with no Emby rebuilds the pill from the cache (no
    // network), preserving the pill and its cursor across the refresh.
    app.home.latest[0].3 = 0; // cursor already 0; selecting would be beyond this test's scope
    assert!(app.fetch_home().is_ok());
    assert!(matches!(
        &app.home.latest[0].1,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-pod"
    ));
    assert_eq!(app.home.latest[0].2.len(), 1);

    // Task 7.2: hiding the library by name (lowercased) drops the pill on the
    // next refresh, even though the cache still holds the shelf data.
    app.hidden_latest = vec!["abs-pod".into()];
    assert!(app.fetch_home().is_ok());
    assert!(
        app.home
            .latest
            .iter()
            .all(|(_, source, _, _)| !matches!(source, HomeLatestSource::Audiobookshelf(_))),
        "hidden library's pill must disappear"
    );
    assert!(
        !app.audiobookshelf_shelf_cache.is_empty(),
        "cache itself is untouched"
    );
}

/// Task 10.1: the flat Home cursor spans continue-items, Emby sections, and
/// Audiobookshelf sections as one list; moving across the boundary lands on
/// items from the other provider.
#[test]
fn flat_cursor_navigation_spans_emby_and_audiobookshelf_sections() {
    let mut app = make_app_stub();

    // One Continue Watching item, then an Emby pill (2 items), then an
    // Audiobookshelf pill (2 items).
    app.home.continue_items = vec![make_item("CW item", "Movie")];
    app.home.latest = vec![
        (
            "Movies".into(),
            HomeLatestSource::Emby("lib-movies".into()),
            vec![
                QueueItem::Emby(Box::new(make_item("Movie one", "Movie"))),
                QueueItem::Emby(Box::new(make_item("Movie two", "Movie"))),
            ],
            0,
        ),
        (
            "Podcasts".into(),
            HomeLatestSource::Audiobookshelf("abs-pod".into()),
            vec![
                QueueItem::Audiobookshelf(abs_episode("1")),
                QueueItem::Audiobookshelf(abs_episode("2")),
            ],
            0,
        ),
    ];

    // Section 0 (Continue Watching): cursor on the CW item.
    app.home_select_section(0);
    app.home.home_cursor = 0;
    assert!(matches!(
        app.home_current_item(),
        Some(QueueItem::Emby(item)) if item.display_name() == "CW item"
    ));

    // Move into the Emby pill, then across its items.
    app.home_select_section(1);
    assert!(matches!(
        app.home_current_item(),
        Some(QueueItem::Emby(item)) if item.display_name() == "Movie one"
    ));
    app.home_move_cursor(1);
    assert!(matches!(
        app.home_current_item(),
        Some(QueueItem::Emby(item)) if item.display_name() == "Movie two"
    ));

    // The Audiobookshelf pill is a selectable section; its flat range sits
    // right after the Emby pill's.
    app.home_select_section(2);
    assert!(matches!(
        app.home_current_item(),
        Some(QueueItem::Audiobookshelf(item)) if item.title == "Episode 1"
    ));
    app.home_move_cursor(1);
    assert!(matches!(
        app.home_current_item(),
        Some(QueueItem::Audiobookshelf(item)) if item.title == "Episode 2"
    ));
    // Clamped at the end of the pill.
    app.home_move_cursor(1);
    assert!(matches!(
        app.home_current_item(),
        Some(QueueItem::Audiobookshelf(item)) if item.title == "Episode 2"
    ));
}

/// Task 10.1: `fetch_home()` refreshes the Audiobookshelf pill from the cache
/// and restores the pill's own cursor (the 4th tuple field) across the
/// refresh, including when Emby is absent.
#[test]
fn fetch_home_restores_per_pill_cursor_for_audiobookshelf() {
    let mut app = make_app_stub();
    app.audiobookshelf_libraries = vec![abs_library("abs-pod", "podcast")];
    let shelves = vec![AudiobookshelfShelf {
        label: "Newest episodes".into(),
        entries: vec![
            AudiobookshelfShelfEntry::Episode(abs_episode("1")),
            AudiobookshelfShelfEntry::Episode(abs_episode("2")),
            AudiobookshelfShelfEntry::Episode(abs_episode("3")),
        ],
    }];
    app.audiobookshelf_shelf_cache.insert(
        "abs-pod".into(),
        crate::app::App::newest_episodes_items(shelves),
    );
    app.rebuild_audiobookshelf_latest();

    // Move the pill's own cursor to the middle item.
    assert_eq!(app.home.latest.len(), 1);
    app.home.latest[0].3 = 1;

    assert!(app.fetch_home().is_ok());
    assert_eq!(app.home.latest.len(), 1);
    let (_, source, items, cursor) = &app.home.latest[0];
    assert!(matches!(
        source,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-pod"
    ));
    assert_eq!(items.len(), 3);
    assert_eq!(*cursor, 1, "per-pill cursor restored across refresh");
    assert_eq!(items[*cursor].display_name(), "Podcast - Episode 2");
}

/// Task 10.2: playing or enqueueing an Audiobookshelf item from a Home pill
/// submits through the shared helper and leaves the Audiobookshelf tab's own
/// cursor/filter state untouched.
#[test]
fn home_play_and_enqueue_leave_audiobookshelf_tab_state_untouched() {
    use crate::app::types_audiobookshelf_browse::{
        AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
    };

    let mut app = make_app_stub();

    // A populated ABS browse tab with a non-default filter, selection, scroll.
    let library = abs_library("abs-pod", "podcast");
    let mut browse = AudiobookshelfBrowseState::new(library.clone());
    browse.episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-1".into(),
            episode_id: "episode-1".into(),
            title: "Episode 1".into(),
            published_at: None,
            duration_seconds: Some(120.0),
        },
    ]);
    browse.episode_filter = AudiobookshelfEpisodeFilter::Unplayed;
    browse.episode_selection = Some(0);
    browse.scroll = 3;
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(browse);

    // Home pill for the same library, with the cursor on the ABS item.
    app.home.latest = vec![(
        "Podcasts".into(),
        HomeLatestSource::Audiobookshelf("abs-pod".into()),
        vec![QueueItem::Audiobookshelf(abs_episode("1"))],
        0,
    )];
    app.home_select_section(1);
    assert!(app.home_current_item().unwrap().is_audiobookshelf());

    app.home_enqueue();
    app.home_play();

    let state = &app.audiobookshelf_browse[0];
    assert_eq!(
        state.episode_filter,
        AudiobookshelfEpisodeFilter::Unplayed,
        "enqueue/play from Home must not touch the ABS tab's filter"
    );
    assert_eq!(
        state.episode_selection,
        Some(0),
        "enqueue/play from Home must not touch the ABS tab's selection"
    );
    assert_eq!(
        state.scroll, 3,
        "enqueue/play from Home must not touch the ABS tab's scroll"
    );
}

/// Task 14.1: the "Latest Feeds" pill is built from `FeedTabState.all_entries`
/// (the combined "All" group, newest-first) and is independent of the Feeds
/// tab's own `selected_group`/`watched_filter` at the time Home populates.
#[test]
fn feeds_pill_reflects_all_entries_newest_first_independent_of_tab_filter() {
    let mut app = make_app_stub();

    fn entry(title: &str, pub_date_secs: Option<u64>, played: bool) -> FeedEntry {
        FeedEntry {
            guid: format!("guid-{title}"),
            title: title.into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played,
        }
    }

    // Two subscriptions; the "All" group interleaves them newest-first.
    app.feed_tab.subscriptions = vec![
        mbv_core::config::FeedSubscription {
            name: "Sub A".into(),
            url: "https://example.test/a".into(),
            kind: mbv_core::config::FeedKind::Audio,
        },
        mbv_core::config::FeedSubscription {
            name: "Sub B".into(),
            url: "https://example.test/b".into(),
            kind: mbv_core::config::FeedKind::Audio,
        },
    ];
    app.feed_tab.entries = vec![
        vec![entry("sub-a-old", Some(100), false)],
        vec![
            entry("sub-b-new", Some(300), true),
            entry("sub-b-mid", Some(200), false),
        ],
    ];
    app.feed_tab.rebuild_all_entries();

    // The Feeds tab itself is viewing a per-subscription group with a
    // watched-state filter active; the Home pill must ignore both.
    app.feed_tab.selected_group = 1;
    app.feed_tab.watched_filter = WatchedFilter::Unwatched;
    app.feed_tab.cursor = 1;
    app.feed_tab.rebuild_filtered_entries();
    assert_eq!(
        app.feed_tab.visible_entries().len(),
        1,
        "tab filter must hide the played entry from the tab"
    );

    assert!(app.fetch_home().is_ok());
    let feeds = app
        .home
        .latest
        .iter()
        .find(|(_, source, _, _)| matches!(source, HomeLatestSource::Feeds))
        .expect("Feeds pill must be present");
    assert_eq!(feeds.0, "Feeds");
    let titles: Vec<String> = feeds.2.iter().map(|i| i.display_name()).collect();
    assert_eq!(
        titles,
        vec!["sub-b-new", "sub-b-mid", "sub-a-old"],
        "pill must be all entries newest-first regardless of tab filter"
    );
    // The played entry is present even though the tab's filter hides it.
    assert!(feeds
        .2
        .iter()
        .any(|i| i.is_feed() && i.display_name() == "sub-b-new"));
}

/// Task 14.2: playing/enqueueing a Feed item from a Home pill submits through
/// the shared helper and leaves the Feeds tab's own cursor/selected
/// group/filter untouched.
#[test]
fn home_play_and_enqueue_leave_feeds_tab_state_untouched() {
    let mut app = make_app_stub();

    fn entry(title: &str) -> FeedEntry {
        FeedEntry {
            guid: format!("guid-{title}"),
            title: title.into(),
            enclosure_url: Some(format!("https://example.test/{title}.mp3")),
            link: None,
            mime_type: Some("audio/mpeg".into()),
            duration_ticks: None,
            pub_date_secs: Some(100),
            feed_kind: Some(mbv_core::config::FeedKind::Audio),
            feed_id: None,
            position_ticks: 0,
            played: false,
        }
    }

    app.feed_tab.entries = vec![vec![entry("Feed one"), entry("Feed two")]];
    app.feed_tab.rebuild_all_entries();
    app.feed_tab.selected_group = 1;
    app.feed_tab.watched_filter = WatchedFilter::Unwatched;
    app.feed_tab.cursor = 1;
    app.feed_tab.rebuild_filtered_entries();
    app.feed_tab.clamp_state();

    // Home pill for the same entries, with the cursor on a Feed item.
    app.home.latest = vec![(
        "Feeds".into(),
        HomeLatestSource::Feeds,
        app.feed_tab
            .all_entries
            .iter()
            .cloned()
            .map(QueueItem::Feed)
            .collect(),
        0,
    )];
    app.home_select_section(1);
    assert!(app.home_current_item().unwrap().is_feed());

    app.home_enqueue();
    app.home_play();

    assert_eq!(
        app.feed_tab.selected_group, 1,
        "enqueue/play from Home must not touch the Feeds tab's group"
    );
    assert_eq!(
        app.feed_tab.watched_filter,
        WatchedFilter::Unwatched,
        "enqueue/play from Home must not touch the Feeds tab's filter"
    );
    assert_eq!(
        app.feed_tab.cursor, 1,
        "enqueue/play from Home must not touch the Feeds tab's cursor"
    );
}

#[test]
fn empty_abs_library_section_is_still_a_selectable_pill() {
    // Home pill convention: every section in `home.latest` is a real pill
    // (an ABS library, an Emby view, or Feeds), empty or not — matching
    // Continue Watching, which always renders and shows "(empty)" when bare.
    // An empty ABS library must be selectable so the feature is discoverable
    // even before any episode has been published/fetched.
    let mut app = make_app_stub();
    app.home.latest = vec![(
        "Podcasts".into(),
        HomeLatestSource::Audiobookshelf("abs-pod".into()),
        Vec::new(),
        0,
    )];

    assert!(
        app.home_section_is_valid(1),
        "an empty ABS library section must still be a valid pill"
    );

    app.home_select_section(1);
    assert_eq!(
        app.home.section, 1,
        "selecting the empty pill keeps section 1"
    );
}

#[test]
fn later_arrivals_do_not_reorder_provider_pills() {
    // Canonical pill order is Emby (0), Audiobookshelf (1), Feeds (2),
    // regardless of async completion order. A Feeds pill that lands before an
    // Audiobookshelf shelf cache, or an Emby bootstrap that lands last, must
    // not reorder the sections.
    let mut app = make_app_stub();
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];

    // Feeds populate first.
    app.rebuild_feeds_latest();
    // Audiobookshelf shelf arrives after: its pill lands after the Feeds one
    // in arrival order, but the merge ranks Audiobookshelf before Feeds.
    app.audiobookshelf_libraries = vec![abs_library("abs-pod", "podcast")];
    app.rebuild_audiobookshelf_latest();
    assert!(matches!(
        &app.home.latest[0].1,
        HomeLatestSource::Audiobookshelf(id) if id == "abs-pod"
    ));
    assert!(matches!(&app.home.latest[1].1, HomeLatestSource::Feeds));

    // Emby bootstrap arriving last sorts before both.
    app.apply_emby_bootstrap(EmbyBootstrap {
        continue_items: Vec::new(),
        views: Vec::new(),
        latest: vec![EmbyLatestSection {
            title: "Movies".into(),
            view_id: "movies".into(),
            items: vec![make_item("New", "Movie")],
        }],
    });
    assert!(matches!(
        &app.home.latest[0].1,
        HomeLatestSource::Emby(id) if id == "movies"
    ));
    assert!(matches!(
        &app.home.latest[1].1,
        HomeLatestSource::Audiobookshelf(id) if id == "abs-pod"
    ));
    assert!(matches!(&app.home.latest[2].1, HomeLatestSource::Feeds));
}

#[test]
fn home_latest_source_pref_key_round_trips() {
    for source in [
        HomeLatestSource::Emby("view-1".into()),
        HomeLatestSource::Audiobookshelf("abs-lib".into()),
        HomeLatestSource::Feeds,
    ] {
        let key = source.pref_key();
        assert_eq!(
            HomeLatestSource::from_pref_key(&key),
            Some(source),
            "pref_key round-trips {key:?}"
        );
    }
    assert_eq!(HomeLatestSource::from_pref_key(""), None);
    assert_eq!(HomeLatestSource::from_pref_key("unknown:2"), None);
}

#[test]
fn home_section_pref_is_empty_for_continue_watching() {
    let mut app = make_app_stub();
    // A populated `latest` is what exposes the off-by-one: with section 0,
    // the old `saturating_sub(1)` returned `latest[0]`'s key (the next pill).
    app.home.latest = vec![
        (
            "Movies".into(),
            HomeLatestSource::Emby("lib-movies".into()),
            vec![QueueItem::Emby(Box::new(make_item("Movie one", "Movie")))],
            0,
        ),
        (
            "Podcasts".into(),
            HomeLatestSource::Audiobookshelf("abs-pod".into()),
            vec![QueueItem::Audiobookshelf(abs_episode("1"))],
            0,
        ),
    ];

    // Section 0 (Continue Watching) must persist as the empty sentinel, never
    // as a `latest` pill's key.
    app.home.section = 0;
    assert!(
        app.home_section_pref().is_empty(),
        "Continue Watching persists as no section key"
    );

    // A real pill index must still persist its own key.
    app.home.section = 1;
    assert_eq!(app.home_section_pref(), "emby:lib-movies");
    app.home.section = 2;
    assert_eq!(app.home_section_pref(), "abs:abs-pod");
}
