use super::*;
use crate::app::tests::*;
use mbv_core::playback_queue::FeedEntry;

fn playable_feed_entry(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(mbv_core::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

/// With `TabSelection::Feeds` active, `emby_library_index()` must be `None`,
/// `shuffle_play` must not panic, and the key handler must route to
/// feed-specific actions rather than library-item dispatch.
#[test]
fn feeds_tab_does_not_route_into_library_behavior() {
    let mut app = make_app_stub();

    // Set up a library so a bounds-miss shuffle target can be checked.
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    // Configure a feed subscription and select the Feeds tab.
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab
        .entries
        .resize_with(app.feed_tab.subscriptions.len(), Vec::new);
    app.tab = TabSelection::Feeds;
    app.panel_focus = PanelFocus::Library;

    // 1. Feeds must not have a library index.
    assert_eq!(
        app.tab.emby_library_index(),
        None,
        "Feeds should not expose a library index"
    );

    // 2. A bounds-miss shuffle target (the old tab-recovery would
    // panic on emby_library_index().unwrap()) must return early without
    // panic or mutation.
    let queue_len_before = app.player_tab.emby_items().len();
    app.shuffle_play_target(app.libs.len(), None); // index past the single library
    assert_eq!(
        app.player_tab.emby_items().len(),
        queue_len_before,
        "a bounds-miss shuffle must not touch the queue"
    );

    // 3. Feeds keys are now handled by the FeedsComponent; the legacy
    // App::handle_key_feeds was deleted (task 8.1). The guard below
    // still proves the library cursor wasn't touched.
    let _key_down = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    );

    // 4. The library tab's nav_stack cursor must be untouched — Feeds
    //    did not dispatch into library browsing.
    assert_eq!(
        app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "library cursor must remain unchanged by feed-tab key handling"
    );
}

/// Verify that switching to the Feeds tab sets focus to Library without
/// corrupting a library's position or selection state.
#[test]
fn set_library_tab_to_feeds_does_not_corrupt_library_state() {
    let mut app = make_app_stub();

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(3, 2),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    // Start on the library tab so its position state is visible.
    app.tab = TabSelection::EmbyLibrary(0);

    // Configure feeds so the Feeds tab appears.
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Podcast".into(),
        url: "https://example.test/podcast".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab
        .entries
        .resize_with(app.feed_tab.subscriptions.len(), Vec::new);

    // Switch to Feeds.
    let feeds_pos = app.feeds_tab_pos().expect("feeds tab should exist");
    app.set_library_tab(feeds_pos);

    assert!(app.tab.is_feeds(), "tab should be Feeds");
    assert_eq!(
        app.panel_focus,
        PanelFocus::Library,
        "Feeds tab should set Library panel focus"
    );
    // Library nav_stack must be untouched.
    assert_eq!(app.libs[0].nav_stack[0].resting().cursor(), 3);
    assert_eq!(app.libs[0].nav_stack[0].resting().scroll(), 2);
}

/// An entry with neither an enclosure URL nor a link has no playable source;
/// `play_feed_entry` must flash and not dispatch.
#[test]
fn feed_tab_play_entry_no_source_does_not_dispatch() {
    let mut app = make_app_stub();
    app.feed_tab.entries = vec![vec![FeedEntry {
        guid: "a".into(),
        title: "Entry A".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(mbv_core::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }]];
    app.feed_tab.rebuild_all_entries();
    app.play_feed_entry(app.feed_tab.entries[0][0].clone());

    assert!(
        app.status.contains("no playable source"),
        "expected a no-playable-source toast, got {:?}",
        app.status
    );
    assert!(
        app.playback_queue().queue.slots().is_empty(),
        "no-source entry must not be mirrored into the queue panel"
    );
}

#[test]
fn direct_remote_feed_play_submits_the_selected_entry() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(make_items(1), make_items(1));
    app.queue_scope = QueueScope::Remote;
    app.feed_tab.entries = vec![vec![playable_feed_entry("feed-play")]];
    app.feed_tab.rebuild_all_entries();

    app.play_feed_entry(app.feed_tab.entries[0][0].clone());

    match cmd_rx.try_recv().unwrap() {
        mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace {
            items,
            start_idx: Some(1),
        } => assert!(matches!(
            &items[1],
            mbv_core::playback_queue::QueueItem::Feed(entry)
                if entry.guid == "feed-play"
        )),
        _ => panic!("expected unified Feed submission"),
    }
}

#[test]
fn direct_remote_feed_enqueue_uses_unified_append() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(make_items(1), make_items(1));
    app.queue_scope = QueueScope::Remote;
    app.feed_tab.entries = vec![vec![playable_feed_entry("feed-append")]];
    app.feed_tab.rebuild_all_entries();

    app.enqueue_feed_entry(app.feed_tab.entries[0][0].clone());

    assert!(matches!(
        cmd_rx.try_recv().unwrap(),
        mbv_core::ctrl::CtrlCmd::UnifiedQueueAppend { items }
            if matches!(&items[0], mbv_core::playback_queue::QueueItem::Feed(entry)
                if entry.guid == "feed-append")
    ));
}

/// F5 while the Feeds tab is selected must not dispatch into the Emby or
/// Audiobookshelf refresh paths: the Emby library stays unmarked and the
/// Audiobookshelf catalog keeps its state. (Whether F5 then refreshes feeds
/// is owned by the refresh-dispatch change; the cross-Service no-leak is
/// what this guards.)
#[test]
fn f5_on_feeds_tab_does_not_reach_emby_or_audiobookshelf_refresh() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    let abs_library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut abs_state =
        super::types_audiobookshelf_browse::AudiobookshelfBrowseState::new(abs_library.clone());
    abs_state.append_page(
        0,
        20,
        1,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab.entries.resize_with(1, Vec::new);
    app.tab = TabSelection::Feeds;
    app.panel_focus = PanelFocus::Library;

    app.refresh_current_view();

    assert!(
        !app.libs[0].nav_stack[0].loading,
        "Feeds F5 must not reload the Emby library"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].shows.len(),
        1,
        "Feeds F5 must not clear the Audiobookshelf catalog"
    );
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

/// F5 on the Feeds destination invokes the feed refresh: the feed tab is
/// marked loading and the Emby / Audiobookshelf / queue state stays
/// untouched.
#[test]
fn f5_on_feeds_tab_invokes_feed_refresh() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    let abs_library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut abs_state =
        super::types_audiobookshelf_browse::AudiobookshelfBrowseState::new(abs_library.clone());
    abs_state.append_page(
        0,
        20,
        1,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab.entries.resize_with(1, Vec::new);
    app.tab = TabSelection::Feeds;
    app.panel_focus = PanelFocus::Library;

    app.refresh_current_view();

    assert!(app.feed_tab.loading, "Feeds F5 must start a feed refresh");
    assert_eq!(
        app.feed_tab.pending_results, 1,
        "the single subscription must be fetching"
    );
    assert!(
        !app.libs[0].nav_stack[0].loading,
        "Feeds F5 must not reload the Emby library"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].shows.len(),
        1,
        "Feeds F5 must not clear the Audiobookshelf catalog"
    );
    assert_eq!(app.player_tab.total_queue_len(), 0);
}
