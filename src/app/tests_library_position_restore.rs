use super::*;
use crate::app::tests::*;

#[test]
fn restoring_library_position_does_not_eagerly_prefetch_all_items() {
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Minimal local server: we're proving *absence* of a request, so we
    // only need to count accepted connections, not answer them. See
    // crates/mbv-core/src/api.rs's `local_listener_url()` test helper
    // for the same non-blocking-accept-with-deadline idiom.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let connection_count = Arc::new(AtomicUsize::new(0));
    let connection_count_for_thread = connection_count.clone();
    let server_handle = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(400);
        while std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    connection_count_for_thread.fetch_add(1, Ordering::SeqCst);
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    let mut app = make_app_stub();
    {
        let mut client = app.client.lock().unwrap();
        client.config.server_url = base_url;
        client.user_id = "user-1".into();
        client.token = "token-1".into();
    }
    app.panel_focus = PanelFocus::Queue;
    app.library_tab = 1;
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let power_position = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "Power".into(),
            focused_item_id: Some("id1".into()),
            cursor_index: 1,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, power_position.clone());
    // Deliberately do NOT call `ensure_lib_loaded_for(0)` here (unlike
    // the neighboring `..._accepts_restore_from_queue_focus` test): with
    // an empty nav_stack and a saved position, it spawns its own
    // background restore fetch (`spawn_restore_library_position`) that
    // would also connect to the mock server below, confounding the
    // connection count this test asserts on. It isn't needed for
    // `handle_restored_library_position`'s guards to pass -- both
    // `saved_library_position` and `active_library_position_scope_for`
    // are already satisfied by the state set up above.

    // Restore a level that is NOT fully loaded (2 items out of a
    // reported 50) -- this is the condition under which
    // spawn_all_items_prefetch actually does network I/O
    // (is_fully_loaded() is items.len() >= total_count).
    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: power_position.clone(),
        position: power_position,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Power restored".into(),
            items: make_items(2),
            total_count: 50,
            cursor: 1,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
    });

    // Give a background thread every reasonable chance to have
    // connected by now if the eager prefetch were still wired up.
    std::thread::sleep(std::time::Duration::from_millis(300));
    server_handle.join().unwrap();

    assert_eq!(
        connection_count.load(Ordering::SeqCst),
        0,
        "restoring a library position must not eagerly fetch all items \
             (spawn_all_items_prefetch should not be called from \
             handle_restored_library_position -- see #260)"
    );
    assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
    assert!(app.libs[0].nav_stack[0].all_items.is_none());
}

#[test]
fn restoring_pre_pill_feature_position_captures_library_total_and_shows_pills() {
    // Regression test: a `LibraryPosition` saved before the
    // letter-range-pill feature existed carries `library_total: None`
    // and `letter_filter_index: None`. Restoring such a position must
    // still capture `library_total` from the restored level's
    // `total_count` (via `maybe_capture_library_total_and_apply_default_pill`)
    // so `should_show_letter_pills` becomes true for large libraries --
    // otherwise the pill row never appears for any library opened
    // before this feature shipped.
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let pre_feature_position = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            focused_item_id: None,
            cursor_index: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, pre_feature_position.clone());
    app.panel_focus = PanelFocus::Queue;
    app.library_tab = 1;

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: pre_feature_position.clone(),
        position: pre_feature_position,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),
            total_count: 673,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
    });

    assert_eq!(app.libs[0].library_total, Some(673));
    assert!(app.should_show_letter_pills(0));
    assert_eq!(
        app.libs[0].nav_stack[0].letter_filter,
        Some(super::render::LetterFilter::default_filter()),
        "large restored library should get the default A-C pill applied"
    );
}

#[test]
fn restored_default_library_fallback_rewrites_state_file_after_success() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: Some(LibSearch {
            query: "stale".into(),
            items: make_items(1),
            results: vec![0],
            cursor: 0,
            scroll: 0,
            loading: false,
        }),
        feed_home_video: None,
        album_track_focus: Some(0),
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let stale = crate::config::LibraryPosition {
        levels: vec![
            crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                focused_item_id: Some("missing".into()),
                cursor_index: 99,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            },
            crate::config::LibraryPositionLevel {
                parent_id: "missing".into(),
                title: "Gone".into(),
                focused_item_id: Some("id1".into()),
                cursor_index: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            },
        ],
        ..Default::default()
    };
    app.replace_saved_library_position(0, stale);

    let restored_items = make_items(2);
    let restored_nav = vec![BrowseLevel {
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        items: restored_items.clone(),
        total_count: restored_items.len(),
        cursor: 1,
        scroll: 0,
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    }];
    let restored_position = crate::config::LibraryPosition {
        levels: vec![restored_nav[0].to_position_level()],
        ..Default::default()
    };

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: crate::config::load_library_position_state()
            .libraries
            .get("lib-movies")
            .cloned()
            .expect("requested position"),
        position: restored_position.clone(),
        nav_stack: restored_nav,
    });

    // `restored_position` was snapshotted from the nav_stack alone, before
    // `handle_lib_event` ran. The restore also captures the library's
    // true total via `maybe_capture_library_total_and_apply_default_pill`
    // (see #325 follow-up fix), so the state actually persisted carries
    // `library_total: Some(2)` (this level's `total_count`) rather than
    // the `None` `restored_position` was built with.
    let mut expected_position = restored_position;
    expected_position.levels[0].library_total = Some(2);
    let saved = crate::config::load_library_position_state();
    assert_eq!(
        saved.libraries.get("lib-movies").cloned(),
        Some(expected_position)
    );
    assert!(app.libs[0].search.is_none());
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn stale_restore_is_ignored_after_saved_position_is_cleared() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let requested = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            focused_item_id: Some("id1".into()),
            cursor_index: 1,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, requested.clone());
    app.clear_saved_library_position(0);

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: requested.clone(),
        position: requested,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),
            total_count: 2,
            cursor: 1,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
    });

    assert!(app.libs[0].nav_stack.is_empty());
    assert!(!crate::config::load_library_position_state()
        .libraries
        .contains_key("lib-movies"));
}

#[test]
fn stale_restore_is_ignored_when_scope_is_no_longer_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Power".into(),
            items: make_items(2),
            total_count: 2,
            cursor: 1,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "DateCreated".into(),
            sort_order: "Descending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let power_position = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "Power".into(),
            focused_item_id: Some("id1".into()),
            cursor_index: 1,
            item_types: None,
            unplayed_only: false,
            sort_by: "DateCreated".into(),
            sort_order: "Descending".into(),
            letter_filter_index: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, power_position.clone());

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: power_position.clone(),
        position: power_position.clone(),
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Power restored".into(),
            items: make_items(2),
            total_count: 2,
            cursor: 1,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "DateCreated".into(),
            sort_order: "Descending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
    });

    // `library_tab` was never pointed at this library, so
    // `active_library_position_scope_for` says it's not the active
    // library and the restore must be ignored.
    assert_eq!(app.libs[0].nav_stack[0].title, "Power");
    let saved = crate::config::load_library_position_state();
    assert_eq!(
        saved.libraries.get("lib-movies").cloned(),
        Some(power_position)
    );
}
