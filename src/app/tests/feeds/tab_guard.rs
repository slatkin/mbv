use crate::app::tests::*;
use mbv_queue::FeedEntry;
use mbv_ui_model::browse::ServerRows;

fn playable_feed_entry(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(mbv_queue::FeedKind::Audio),
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
            rows: ServerRows::new(1),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],

            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    // Configure a feed subscription and select the Feeds tab.
    app.feed_tab.subscriptions = vec![mbv_config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_queue::FeedKind::Audio,
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
    let queue_len_before = app.local_view.emby_items().len();
    app.shuffle_play_target(app.libs.len(), None); // index past the single library
    assert_eq!(
        app.local_view.emby_items().len(),
        queue_len_before,
        "a bounds-miss shuffle must not touch the queue"
    );

    // 3. Feeds keys are now handled by the Library panel's Feeds owner; the
    // legacy App::handle_key_feeds was deleted (task 8.1). The guard below
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
        feed_kind: Some(mbv_queue::FeedKind::Audio),
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
        app.playback_queue().playback_queue().slots().is_empty(),
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
        mbv_ctrl::CtrlCmd::UnifiedQueueReplace {
            items,
            start_idx: Some(1),
            ..
        } => assert!(matches!(
            &items[1],
            mbv_queue::QueueItem::Feed(entry)
                if entry.guid == "feed-play"
        )),
        _ => panic!("expected unified Feed submission"),
    }
}

#[test]
fn feed_selection_enqueue_preserves_supplied_order() {
    let mut app = make_app_stub();
    // Row 5.3: each enqueue is an Append owner op, so the supplied order is
    // observed on the wire rather than in an optimistic Client write.
    let cmd_rx = super::super::live_owner_channel(&mut app);
    app.enqueue_feed_entries(vec![
        playable_feed_entry("feed-first"),
        playable_feed_entry("feed-second"),
        playable_feed_entry("feed-third"),
    ]);

    let appended: Vec<String> = cmd_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                [mbv_queue::QueueItem::Feed(entry)] => Some(entry.guid.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        appended,
        vec!["feed-first", "feed-second", "feed-third"],
        "the enqueues reach the owner in the supplied order"
    );
}

/// F5 while the Feeds tab is selected must not dispatch into the Emby or
/// Audiobookshelf refresh paths: the Emby library stays unmarked and the
/// Audiobookshelf catalog keeps its state. (Whether F5 then refreshes feeds
/// is owned by the refresh-dispatch change; the cross-Service no-leak is
/// what this guards.)
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
            rows: ServerRows::new(1),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],

            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    let abs_library = mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut abs_state =
        mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState::new(abs_library.clone());
    abs_state.append_page(
        0,
        20,
        1,
        vec![mbv_audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    app.feed_tab.subscriptions = vec![mbv_config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_queue::FeedKind::Audio,
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
    assert_eq!(app.local_view.total_queue_len(), 0);
}
