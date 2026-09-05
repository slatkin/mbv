//! Part 1 of #543: Home's population is decoupled from Emby's connection
//! state. `fetch_home()` no longer errors without an Emby client, and both
//! Emby writers (sync `fetch_home()` and async `apply_emby_bootstrap()`)
//! merge their entries by `HomeLatestSource` instead of replacing Home's
//! data outright, so entries from other providers survive. Task 5.3d:
//! `fetch_home()`/`apply_emby_bootstrap()` compute a `HomeContent` snapshot
//! (Model-owned; the shell assigns it to `Model.home_content`), so these
//! tests assert on the returned content instead of a deleted `App.home`.

use crate::app::shell::Model;
use crate::app::tests::*;
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

    let content = app.fetch_home().expect("no-Emby refresh must not fail");

    // Feeds and Audiobookshelf pills are both rebuilt from their sources.
    assert_eq!(content.latest.len(), 2, "both non-Emby pills must exist");
    let feeds = content
        .latest
        .iter()
        .find(|(_, source, _, _)| matches!(source, HomeLatestSource::Feeds))
        .expect("Feeds pill must survive");
    assert_eq!(feeds.2.len(), 1);
    assert_eq!(feeds.2[0].display_name(), "Feed one");
    assert!(content
        .latest
        .iter()
        .any(|(_, source, _, _)| matches!(source, HomeLatestSource::Audiobookshelf(_))));

    // Second refresh: still no Emby error, both pills intact.
    let content = app.fetch_home().expect("second refresh must not fail");
    let feeds = content
        .latest
        .iter()
        .find(|(_, source, _, _)| matches!(source, HomeLatestSource::Feeds))
        .expect("Feeds pill must survive the second refresh");
    assert_eq!(feeds.2.len(), 1);
}

/// Task 5.3d: `apply_emby_bootstrap` merges the Emby entries into the
/// caller-supplied prior pills (the Model-owned `home_content.latest` the
/// shell passes in) and returns the computed `HomeContent`.
#[test]
fn apply_emby_bootstrap_merges_only_emby_entries() {
    let mut app = make_app_stub();

    // Pre-existing mixed Home: an Audiobookshelf and a Feeds entry alongside
    // an Emby entry (view id "movies") that the bootstrap will replace.
    // Canonical pill order (Emby, Audiobookshelf, Feeds) is applied by the
    // merge regardless of this arrival order.
    let prior_latest = vec![
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

    let content = app.apply_emby_bootstrap(
        EmbyBootstrap {
            continue_items: Vec::new(),
            views: Vec::new(),
            latest: vec![EmbyLatestSection {
                title: "Movies".into(),
                view_id: "movies".into(),
                items: vec![make_item("New", "Movie")],
            }],
        },
        &prior_latest,
    );

    assert_eq!(
        content.latest.len(),
        3,
        "Emby entry replaced, no duplicates"
    );
    // Canonical pill order is Emby, then Audiobookshelf, then Feeds —
    // regardless of arrival order.
    assert!(matches!(
        &content.latest[0].1,
        HomeLatestSource::Emby(id) if id == "movies"
    ));
    assert_eq!(content.latest[0].2.len(), 1);
    assert_eq!(content.latest[0].2[0].display_name(), "New");
    assert!(matches!(
        &content.latest[1].1,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-lib"
    ));
    assert!(matches!(&content.latest[2].1, HomeLatestSource::Feeds));
    assert_eq!(content.latest[2].2.len(), 1);
}

#[test]
fn apply_emby_bootstrap_without_prior_data_populates_emby_only_path() {
    let mut app = make_app_stub();

    let content = app.apply_emby_bootstrap(
        EmbyBootstrap {
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
        },
        &[],
    );

    assert_eq!(content.latest.len(), 2);
    assert!(matches!(
        &content.latest[0].1,
        HomeLatestSource::Emby(id) if id == "v1"
    ));
    assert_eq!(content.latest[0].2.len(), 2);
    assert_eq!(content.latest[0].0, "Movies");
    assert!(matches!(
        &content.latest[1].1,
        HomeLatestSource::Emby(id) if id == "v2"
    ));
    assert_eq!(content.latest[1].2.len(), 1);
    assert!(!content.loading);
}

/// Part 2 of #543, Tasks 6.3/7.1/7.2: the shelf-fetch handler caches the
/// `Newest Episodes` items per podcast library and Home's pill is rebuilt
/// from the cache; `fetch_home()` rebuilds it again with no network fetch,
/// re-applying `hidden_latest`; book libraries never get a pill. Task 5.3d:
/// the rebuild targets the computed `HomeContent`'s `latest`.
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
    let content = app.fetch_home().expect("fetch succeeds without Emby");

    assert_eq!(
        content.latest.len(),
        1,
        "only the podcast library gets a pill"
    );
    let (title, source, items, _cursor) = &content.latest[0];
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

    // `fetch_home()` with no Emby rebuilds the pill from the cache (no
    // network), preserving the pill across the refresh.
    let content = app.fetch_home().expect("fetch succeeds without Emby");
    assert!(matches!(
        &content.latest[0].1,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-pod"
    ));
    assert_eq!(content.latest[0].2.len(), 1);

    // Task 7.2: hiding the library by name (lowercased) drops the pill on the
    // next refresh, even though the cache still holds the shelf data.
    app.hidden_latest = vec!["abs-pod".into()];
    let content = app.fetch_home().expect("fetch succeeds without Emby");
    assert!(
        content
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

/// Task 5.3d: `Model::home_flat_target` resolves the flat cursor across
/// continue-items, Emby pills, and Audiobookshelf pills as one list, using
/// the supplied explicit flat target (the flat cursor is component-owned, so
/// resolution is pinned with explicit target indices, not a deleted App
/// cursor).
#[test]
fn flat_cursor_resolution_spans_emby_and_audiobookshelf_sections() {
    let mut model = Model::new(make_app_stub());

    // One Continue Watching item, then an Emby pill (2 items), then an
    // Audiobookshelf pill (2 items).
    model.home_content.continue_items = vec![make_item("CW item", "Movie")];
    model.home_content.latest = vec![
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

    // Flat index 0 is the Continue Watching item.
    assert!(matches!(
        model.home_flat_target(0),
        Some((QueueItem::Emby(item), true)) if item.display_name() == "CW item"
    ));

    // The Emby pill's flat range sits right after Continue Watching: flat
    // index 1 is "Movie one", 2 is "Movie two".
    assert!(matches!(
        model.home_flat_target(1),
        Some((QueueItem::Emby(item), false)) if item.display_name() == "Movie one"
    ));
    assert!(matches!(
        model.home_flat_target(2),
        Some((QueueItem::Emby(item), false)) if item.display_name() == "Movie two"
    ));

    // The Audiobookshelf pill's flat range sits right after the Emby pill's:
    // flat index 3 is "Episode 1", 4 is "Episode 2".
    assert!(matches!(
        model.home_flat_target(3),
        Some((QueueItem::Audiobookshelf(item), false)) if item.title == "Episode 1"
    ));
    assert!(matches!(
        model.home_flat_target(4),
        Some((QueueItem::Audiobookshelf(item), false)) if item.title == "Episode 2"
    ));
}

/// Task 10.1: `fetch_home()` refreshes the Audiobookshelf pill from the cache
/// (the pill's own per-section cursor is a preserved-but-vestigial tuple
/// field now — the mounted `HomeComponent` owns the real flat cursors, task
/// 5.3d — so the refresh restores the pill and its items, not a cursor).
#[test]
fn fetch_home_refreshes_audiobookshelf_pill_from_cache() {
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

    let content = app.fetch_home().expect("fetch succeeds without Emby");
    assert_eq!(content.latest.len(), 1);
    let (_, source, items, _cursor) = &content.latest[0];
    assert!(matches!(
        source,
        HomeLatestSource::Audiobookshelf(lib) if lib == "abs-pod"
    ));
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].display_name(), "Podcast - Episode 1");

    // A second refresh restores the same pill and items.
    let content = app.fetch_home().expect("fetch succeeds without Emby");
    assert_eq!(content.latest.len(), 1);
    assert_eq!(content.latest[0].2.len(), 3);
}

/// Task 10.2: playing or enqueueing an Audiobookshelf item from a Home pill
/// submits through the shared helper and leaves the Audiobookshelf tab's own
/// cursor/filter state untouched. Task 5.3d: the item is resolved at the
/// Model boundary (`home_flat_target`) and passed into the App effect.
#[test]
fn home_play_and_enqueue_leave_audiobookshelf_tab_state_untouched() {
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;

    let mut app = make_app_stub();

    // A populated ABS browse tab.
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
    browse.selected_id = Some("show-1".into());
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(browse);

    // Home pill for the same library, with the cursor on the ABS item.
    let mut model = Model::new(app);
    model.home_content.latest = vec![(
        "Podcasts".into(),
        HomeLatestSource::Audiobookshelf("abs-pod".into()),
        vec![QueueItem::Audiobookshelf(abs_episode("1"))],
        0,
    )];
    let (item, from_cw) = model.home_flat_target(0).expect("flat target 0");
    assert!(item.is_audiobookshelf());
    assert!(!from_cw);

    model.app.home_enqueue_target(item.clone(), from_cw);
    model.app.home_play_target(item, from_cw);

    let state = &model.app.audiobookshelf_browse[0];
    assert_eq!(
        state.selected_id.as_deref(),
        Some("show-1"),
        "enqueue/play from Home must not touch the ABS tab's resting selection"
    );
}

/// Task 14.1: the "Latest Feeds" pill is built from `FeedTabState.all_entries`
/// (the combined "All" group, newest-first) and is independent of the Feeds
/// component's local selection at the time Home populates.
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

    let content = app.fetch_home().expect("fetch succeeds without Emby");
    let feeds = content
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
/// the shared helper without mutating the Feeds shell snapshot. Task 5.3d:
/// the item is resolved at the Model boundary and passed into the App effect.
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
    // Home pill for the same entries, with the cursor on a Feed item.
    let mut model = Model::new(app);
    model.home_content.latest = vec![(
        "Feeds".into(),
        HomeLatestSource::Feeds,
        model
            .app
            .feed_tab
            .all_entries
            .iter()
            .cloned()
            .map(QueueItem::Feed)
            .collect(),
        0,
    )];
    let (item, from_cw) = model.home_flat_target(0).expect("flat target 0");
    assert!(item.is_feed());
    assert!(!from_cw);

    model.app.home_enqueue_target(item.clone(), from_cw);
    model.app.home_play_target(item, from_cw);

    assert_eq!(model.app.feed_tab.all_entries.len(), 2);
}

/// Task 5.3d: canonical pill order (Emby, Audiobookshelf, Feeds) holds
/// regardless of arrival order. The App-side writers deliver section deltas
/// (feeds/ABS) or a bootstrap to the shell, which splices them into
/// Model-owned `latest` through the shared `merge_home_sections`; this test
/// drives that splice directly with the pure section getters to pin the
/// ordering.
#[test]
fn later_arrivals_do_not_reorder_provider_pills() {
    let mut app = make_app_stub();
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];

    // A local `latest` accumulates deltas in arrival order, like the
    // Model-owned splice: Feeds first, then Audiobookshelf.
    let mut latest: Vec<(String, HomeLatestSource, Vec<QueueItem>, usize)> = Vec::new();
    crate::app::library_load_actions::merge_home_sections(
        &mut latest,
        app.feeds_latest_section().into_iter().collect(),
        |source| matches!(source, HomeLatestSource::Feeds),
    );
    // Audiobookshelf shelf arrives after: its pill lands after the Feeds one
    // in arrival order, but the merge ranks Audiobookshelf before Feeds.
    app.audiobookshelf_libraries = vec![abs_library("abs-pod", "podcast")];
    crate::app::library_load_actions::merge_home_sections(
        &mut latest,
        app.audiobookshelf_latest_sections(),
        |source| matches!(source, HomeLatestSource::Audiobookshelf(_)),
    );
    assert!(matches!(
        &latest[0].1,
        HomeLatestSource::Audiobookshelf(id) if id == "abs-pod"
    ));
    assert!(matches!(&latest[1].1, HomeLatestSource::Feeds));

    // Emby bootstrap arriving last sorts before both.
    let content = app.apply_emby_bootstrap(
        EmbyBootstrap {
            continue_items: Vec::new(),
            views: Vec::new(),
            latest: vec![EmbyLatestSection {
                title: "Movies".into(),
                view_id: "movies".into(),
                items: vec![make_item("New", "Movie")],
            }],
        },
        &latest,
    );
    assert!(matches!(
        &content.latest[0].1,
        HomeLatestSource::Emby(id) if id == "movies"
    ));
    assert!(matches!(
        &content.latest[1].1,
        HomeLatestSource::Audiobookshelf(id) if id == "abs-pod"
    ));
    assert!(matches!(&content.latest[2].1, HomeLatestSource::Feeds));
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

/// Task 5.3d, Home persisted-section identity seam: while a one-time persisted
/// restore is still pending (its source has not arrived), an actual async
/// section rebuild/clamp path must not replace the loaded semantic source with
/// the temporary numeric section identity (still Continue Watching / section
/// 0). An unrelated `save_prefs()` after that clamp must keep the pending
/// source identity on disk.
#[test]
fn async_clamp_keeps_pending_home_source_until_restored() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "home_section": "abs:book-lib" }).to_string(),
    )
    .expect("write prefs");
    let mut model = Model::new(make_app_stub());

    // Simulate an actual async clamp/rebuild path. With no Emby client and no
    // cached Audiobookshelf/Feeds sections this rebuilds an empty `latest`
    // while the pending restore remains in Model-owned shell state.
    let _content = model
        .app
        .fetch_home()
        .expect("fetch_home succeeds with no sources");

    assert_eq!(
        model.home_section_pending,
        Some(HomeLatestSource::Audiobookshelf("book-lib".into())),
        "pending restore must remain pending while the source is absent"
    );
    assert_eq!(
        model.home_section_pref(),
        "abs:book-lib",
        "async clamp must not clear the pending semantic source"
    );

    // An unrelated App preference save must retain the shell-owned pending
    // source identity on disk, not overwrite it with Continue Watching.
    model.app.save_prefs();
    let saved = crate::config::prefs_path();
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(saved).expect("prefs written")).unwrap();
    assert_eq!(
        parsed["home_section"], "abs:book-lib",
        "unrelated save must keep the pending Home source while restoration is pending"
    );
}
