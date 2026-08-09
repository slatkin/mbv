use super::*;
use crate::app::tests::*;
use mbv_core::playback_queue::FeedEntry;

/// With `TabSelection::Feeds` active, `library_index()` must be `None`,
/// `shuffle_play` must not panic, and the key handler must route to
/// feed-specific actions rather than library-item dispatch.
#[test]
fn feeds_tab_does_not_route_into_library_behavior() {
    let mut app = make_app_stub();

    // Set up a library so `shuffle_play` would panic if it tried to index.
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
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
        app.tab.library_index(),
        None,
        "Feeds should not expose a library index"
    );

    // 2. shuffle_play must return early without panicking on the unwrap.
    app.shuffle_play(); // would panic on library_index().unwrap() without the guard

    // 3. Down-cursor key must move the feed cursor, not a library cursor.
    app.feed_tab.entries[0] = vec![
        FeedEntry {
            guid: "a".into(),
            title: "Entry A".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: Some(100),
            feed_kind: mbv_core::config::FeedKind::Audio,
        },
        FeedEntry {
            guid: "b".into(),
            title: "Entry B".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: Some(200),
            feed_kind: mbv_core::config::FeedKind::Audio,
        },
    ];
    app.feed_tab.rebuild_all_entries();
    app.feed_tab.selected_group = 0; // "All"
    app.feed_tab.cursor = 0;

    let key_down = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    );
    let consumed = app.handle_feed_tab_key(key_down);
    assert_eq!(consumed, Some(false), "feed tab should consume Down key");
    assert_eq!(
        app.feed_tab.cursor, 1,
        "cursor should advance within feed entries"
    );

    // 4. The library tab's nav_stack cursor must be untouched — Feeds
    //    did not dispatch into library browsing.
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 0,
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
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            cursor: 3,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 2,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    // Start on the library tab so its position state is visible.
    app.tab = TabSelection::Library(0);

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
    assert_eq!(app.libs[0].nav_stack[0].cursor, 3);
    assert_eq!(app.libs[0].nav_stack[0].scroll, 2);
}

/// `feed_tab_play_selected` on an empty entry list (or a stale cursor past
/// the end of a non-empty list) must return without dispatching -- the
/// `.get(cursor)` bounds check is the only guard, since `clamp_state`
/// isn't guaranteed to have run.
#[test]
fn feed_tab_play_selected_out_of_range_cursor_is_noop() {
    let mut app = make_app_stub();

    // Empty list: cursor 0 is already out of range.
    app.feed_tab_play_selected();
    assert!(
        app.status.is_empty(),
        "empty list must not flash or dispatch"
    );

    // Non-empty list with a stale cursor past the end.
    app.feed_tab.entries = vec![vec![FeedEntry {
        guid: "a".into(),
        title: "Entry A".into(),
        enclosure_url: Some("https://example.test/a.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: mbv_core::config::FeedKind::Audio,
    }]];
    app.feed_tab.rebuild_all_entries();
    app.feed_tab.selected_group = 0;
    app.feed_tab.cursor = 5;
    app.feed_tab_play_selected();
    assert!(
        app.status.is_empty(),
        "out-of-range cursor must not flash or dispatch"
    );
}

/// An entry with neither an enclosure URL nor a link has no playable
/// source; `feed_tab_play_selected` must flash and not dispatch.
#[test]
fn feed_tab_play_selected_no_source_entry_does_not_dispatch() {
    let mut app = make_app_stub();
    app.feed_tab.entries = vec![vec![FeedEntry {
        guid: "a".into(),
        title: "Entry A".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: mbv_core::config::FeedKind::Audio,
    }]];
    app.feed_tab.rebuild_all_entries();
    app.feed_tab.selected_group = 0;
    app.feed_tab.cursor = 0;

    app.feed_tab_play_selected();

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
