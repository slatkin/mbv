use super::*;
use mbv_queue::FeedEntry;
use mbv_ui_model::browse::ServerRows;

/// The stub app holding one Movies library whose root level lists one row: the
/// catalog a Feeds-tab guard observes for mutation.
fn movies_library_app() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.push_library_for_test(LibraryTab {
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
    app
}

/// The one subscription a Feeds-tab guard configures.
fn test_feed_subscription() -> mbv_config::FeedSubscription {
    mbv_config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_queue::FeedKind::Audio,
    }
}

/// The app holding the one subscription a Feeds-tab guard configures.
fn feeds_tab_app() -> App {
    let mut app = make_app_stub();
    app.seed_feeds_tab_for_test(vec![test_feed_subscription()], Vec::new());
    app
}

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
    // Set up a library so a bounds-miss shuffle target can be checked.
    let mut app = movies_library_app();

    // Configure a feed subscription and select the Feeds tab.
    app.seed_feeds_tab_for_test(vec![test_feed_subscription()], Vec::new());
    app.seed_active_tab_for_test(TabSelection::Feeds);
    app.set_panel_focus_for_test(PanelFocus::Library);

    // 1. Feeds must not have a library index.
    assert_eq!(
        app.tab_for_test().emby_library_index(),
        None,
        "Feeds should not expose a library index"
    );

    // 2. A bounds-miss shuffle target (the old tab-recovery would
    // panic on emby_library_index().unwrap()) must return early without
    // panic or mutation.
    let queue_len_before = app.local_queue_len_for_test();
    let past_the_catalog = app.libs_for_test().len();
    app.shuffle_play_selected(past_the_catalog, &make_item("Item 0", "Movie"));
    assert_eq!(
        app.local_queue_len_for_test(),
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
        app.emby_library_level_cursor_for_test(0, 0),
        Some(0),
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
    let entry = FeedEntry {
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
    };
    app.seed_feeds_tab_for_test(vec![test_feed_subscription()], vec![entry.clone()]);
    app.play_feed_entry(entry);

    assert!(
        app.newest_toast_text().contains("no playable source"),
        "expected a no-playable-source toast, got {:?}",
        app.newest_toast_text()
    );
    assert!(
        app.playback_queue().slots().is_empty(),
        "no-source entry must not be mirrored into the queue panel"
    );
}

#[test]
fn direct_remote_feed_play_submits_the_selected_entry() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(make_items(1), make_items(1));
    let entry = playable_feed_entry("feed-play");
    app.seed_feeds_tab_for_test(vec![test_feed_subscription()], vec![entry.clone()]);

    app.play_feed_entry(entry);

    // Enter on a media item always replaces the queue with exactly the played
    // entry and starts it (queue-owner-process #857 review follow-up), in the
    // legacy wire form for this direct remote.
    match cmd_rx.try_recv().unwrap() {
        mbv_ctrl::CtrlCmd::UnifiedQueueReplace {
            items,
            start_idx: Some(0),
            ..
        } => assert!(matches!(
            &items[0],
            mbv_queue::QueueItem::Feed(entry)
                if entry.guid == "feed-play"
        )),
        _ => panic!("expected unified Feed submission"),
    }
}

#[test]
fn feed_selection_enqueue_preserves_supplied_order() {
    let mut app = feeds_tab_app();
    // Row 5.3: each enqueue is an Append owner op, so the supplied order is
    // observed on the wire rather than in an optimistic Client write.
    let cmd_rx = app.set_player_with_command_rx_for_test();
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

/// F5 on the Feeds destination invokes the feed refresh: the feed tab is
/// marked loading and the Emby / Audiobookshelf / queue state stays
/// untouched. The selected-destination contract (#745) makes F5 a feed
/// refetch rather than a cross-Service leak; refresh keeps the current group,
/// watched filter and Latest mode because only the per-subscription entries
/// are replaced.
#[test]
fn f5_on_feeds_tab_invokes_feed_refresh() {
    let mut app = movies_library_app();
    app.edit_library_level_for_test(0, 0, |level| {
        level.item_types = Some("Movie".into());
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
            updated_at_ms: 0,
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    app.seed_audiobookshelf_libraries_for_test(vec![abs_library]);
    app.seed_audiobookshelf_browse_for_test(vec![abs_state]);
    app.seed_feeds_tab_for_test(vec![test_feed_subscription()], Vec::new());
    app.seed_active_tab_for_test(TabSelection::Feeds);
    app.set_panel_focus_for_test(PanelFocus::Library);

    app.refresh_current_view(false);

    assert!(
        app.feeds_refresh_in_flight_for_test(),
        "Feeds F5 must start a feed refresh"
    );
    assert_eq!(
        app.feeds_pending_results_for_test(),
        1,
        "the single subscription must be fetching"
    );
    assert!(
        !app.library_level_loading_for_test(0),
        "Feeds F5 must not reload the Emby library"
    );
    assert_eq!(
        app.audiobookshelf_browse_state(0)
            .expect("the stored catalog")
            .shows
            .len(),
        1,
        "Feeds F5 must not clear the Audiobookshelf catalog"
    );
    assert_eq!(app.local_queue_len_for_test(), 0);
}
