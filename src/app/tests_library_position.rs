use super::*;
use crate::app::tests::*;

// `item_text_and_style` and its dedicated tests above were deleted
// (#361): its only production caller was the deleted Standard
// `render/library/table/context.rs`.

#[test]
fn queue_restore_uses_saved_cursor_when_last_played_is_missing() {
    let items = make_items(3);
    let cursor = super::actions::queue_restore_cursor(&items, 2, None, false);
    assert_eq!(cursor, 2);
}

#[test]
fn library_position_snapshot_captures_path_focus_and_feed_group() {
    let mut lib = LibraryTab {
        library: make_item("Movies", "CollectionFolder"),
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(3),
            total_count: 3,
            cursor: 1,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: Some(make_items(3)),
            letter_filter: None,
        }],
        search: Some(LibSearch {
            query: "ignored".into(),
            items: make_items(2),
            results: vec![0],
            cursor: 0,
            scroll: 0,
            loading: false,
        }),
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 2,
            video_cursor: 4,
            video_scroll: 3,
            ..Default::default()
        }),
        album_track_focus: Some(1),
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    };
    lib.library.id = "lib-movies".into();

    let position = lib.library_position_snapshot();

    assert_eq!(position.levels.len(), 1);
    assert_eq!(position.levels[0].parent_id, "lib-movies");
    assert_eq!(position.levels[0].focused_item_id.as_deref(), Some("id1"));
    assert_eq!(position.levels[0].cursor_index, 1);
    assert_eq!(position.levels[0].item_types.as_deref(), Some("Movie"));
    assert_eq!(position.feed_selected_group, 2);
    assert_eq!(position.feed_video_cursor, 4);
    assert_eq!(position.feed_video_scroll, 3);
}

#[test]
fn browse_level_restore_prefers_item_id_and_clamps_index_fallback() {
    let mut saved = crate::config::LibraryPositionLevel {
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        focused_item_id: Some("id3".into()),
        cursor_index: 99,
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        letter_filter_index: None,
        library_total: None,
    };

    let level = BrowseLevel::from_position_level(&saved, make_items(5), 5, 3);

    assert_eq!(level.cursor, 3);
    assert_eq!(level.scroll, 1);
    assert_eq!(level.item_types.as_deref(), Some("Movie"));
    assert!(!level.loading);
    assert!(level.all_items.is_none());

    saved.focused_item_id = Some("missing".into());
    let level = BrowseLevel::from_position_level(&saved, make_items(5), 5, 3);

    assert_eq!(level.cursor, 4);
    assert_eq!(level.scroll, 2);
}

#[test]
fn restore_library_position_keeps_saved_path_when_levels_exist() {
    let mut root_a = make_item("A", "Folder");
    root_a.id = "folder-a".into();
    root_a.is_folder = true;
    let mut root_b = make_item("B", "Folder");
    root_b.id = "folder-b".into();
    root_b.is_folder = true;
    let mut leaf = make_item("Leaf", "Movie");
    leaf.id = "leaf-1".into();

    let saved = crate::config::LibraryPosition {
        levels: vec![
            crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                focused_item_id: Some("folder-b".into()),
                cursor_index: 1,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: Some(0),
                library_total: Some(301),
            },
            crate::config::LibraryPositionLevel {
                parent_id: "folder-b".into(),
                title: "B".into(),
                focused_item_id: Some("leaf-1".into()),
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

    let restored = restore_library_position(&saved, 3, |level| match level.parent_id.as_str() {
        "lib-movies" => Ok((vec![root_a.clone(), root_b.clone()], 2)),
        "folder-b" => Ok((vec![leaf.clone()], 1)),
        other => panic!("unexpected level fetch: {other}"),
    })
    .expect("restore result")
    .expect("restored position");

    assert_eq!(restored.0.levels.len(), 2);
    assert_eq!(
        restored.0.levels[0].focused_item_id.as_deref(),
        Some("folder-b")
    );
    assert_eq!(
        restored.0.levels[1].focused_item_id.as_deref(),
        Some("leaf-1")
    );
    assert_eq!(restored.1.len(), 2);
    assert_eq!(restored.1[0].cursor, 1);
    assert_eq!(restored.1[1].cursor, 0);
    assert_eq!(restored.0.levels[0].letter_filter_index, Some(0));
    assert_eq!(restored.0.levels[0].library_total, Some(301));
}

#[test]
fn restore_library_position_clamps_stale_missing_item_to_nearest_fallback() {
    let mut root = make_item("B", "Folder");
    root.id = "folder-b".into();
    root.is_folder = true;
    let mut leaf0 = make_item("Leaf 0", "Movie");
    leaf0.id = "leaf-0".into();
    let mut leaf1 = make_item("Leaf 1", "Movie");
    leaf1.id = "leaf-1".into();

    let saved = crate::config::LibraryPosition {
        levels: vec![
            crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                focused_item_id: Some("folder-b".into()),
                cursor_index: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            },
            crate::config::LibraryPositionLevel {
                parent_id: "folder-b".into(),
                title: "B".into(),
                focused_item_id: Some("missing".into()),
                cursor_index: 1,
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

    let restored = restore_library_position(&saved, 3, |level| match level.parent_id.as_str() {
        "lib-movies" => Ok((vec![root.clone()], 1)),
        "folder-b" => Ok((vec![leaf0.clone(), leaf1.clone()], 2)),
        other => panic!("unexpected level fetch: {other}"),
    })
    .expect("restore result")
    .expect("restored position");

    assert_eq!(
        restored.0.levels[1].focused_item_id.as_deref(),
        Some("leaf-1")
    );
    assert_eq!(restored.1[1].cursor, 1);
}

#[test]
fn restore_library_position_stops_at_deepest_valid_parent() {
    let mut root_a = make_item("A", "Folder");
    root_a.id = "folder-a".into();
    root_a.is_folder = true;
    let mut root_c = make_item("C", "Folder");
    root_c.id = "folder-c".into();
    root_c.is_folder = true;

    let saved = crate::config::LibraryPosition {
        levels: vec![
            crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                focused_item_id: Some("missing-folder".into()),
                cursor_index: 1,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            },
            crate::config::LibraryPositionLevel {
                parent_id: "missing-folder".into(),
                title: "Gone".into(),
                focused_item_id: Some("leaf-1".into()),
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

    let restored = restore_library_position(&saved, 3, |level| match level.parent_id.as_str() {
        "lib-movies" => Ok((vec![root_a.clone(), root_c.clone()], 2)),
        other => panic!("unexpected level fetch: {other}"),
    })
    .expect("restore result")
    .expect("restored position");

    assert_eq!(restored.0.levels.len(), 1);
    assert_eq!(
        restored.0.levels[0].focused_item_id.as_deref(),
        Some("folder-c")
    );
    assert_eq!(restored.1.len(), 1);
    assert_eq!(restored.1[0].cursor, 1);
}

#[test]
fn applying_library_position_clears_non_position_ui_state() {
    let mut lib = LibraryTab {
        library: make_item("Movies", "CollectionFolder"),
        nav_stack: Vec::new(),
        search: Some(LibSearch {
            query: "ignored".into(),
            items: make_items(2),
            results: vec![0],
            cursor: 0,
            scroll: 0,
            loading: false,
        }),
        feed_home_video: Some(FeedHomeVideoState::default()),
        album_track_focus: Some(2),
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    };
    let position = crate::config::LibraryPosition {
        levels: Vec::new(),
        feed_selected_group: 3,
        feed_video_cursor: 5,
        feed_video_scroll: 4,
    };

    lib.apply_library_position(
        position,
        vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(1),
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
    );

    assert_eq!(lib.nav_stack.len(), 1);
    assert!(lib.search.is_none());
    assert!(lib.album_track_focus.is_none());
    let feed = lib.feed_home_video.as_ref().unwrap();
    assert_eq!(feed.selected_group, 3);
    assert_eq!(feed.video_cursor, 5);
    assert_eq!(feed.video_scroll, 4);
}

// #361 collapsed the old default/power two-scope split to one position
// per library, so the three scope-isolation variants of this test
// ("default writes must not clear power position" etc.) no longer have
// a premise -- there is one saved position now, full stop.
#[test]
fn save_default_library_position_persists_focused_item() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(3),
            total_count: 3,
            cursor: 2,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.save_default_library_position(0);

    let position = app
        .library_position_state
        .libraries
        .get("lib-movies")
        .expect("library position entry");
    assert_eq!(position.levels[0].focused_item_id.as_deref(), Some("id2"));
}

#[test]
fn move_lib_cursor_persists_default_library_position() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(3),
            total_count: 3,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.move_lib_cursor(1);

    let saved = app
        .library_position_state
        .libraries
        .get("lib-movies")
        .expect("position saved");
    assert_eq!(saved.levels[0].focused_item_id.as_deref(), Some("id1"));
}

#[test]
fn saving_visible_library_position_keeps_hidden_library_state_entries() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
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
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.library_position_state.libraries.insert(
        "hidden-lib".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "hidden-lib".into(),
                title: "Hidden".into(),
                focused_item_id: Some("id0".into()),
                cursor_index: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            }],
            ..Default::default()
        },
    );

    app.save_default_library_position(0);

    assert!(app
        .library_position_state
        .libraries
        .contains_key("hidden-lib"));
}

#[test]
fn ensure_lib_loaded_for_uses_saved_position_loading_state_without_root_flash() {
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
    app.library_position_state.libraries.insert(
        "lib-movies".into(),
        crate::config::LibraryPosition {
            levels: vec![
                crate::config::LibraryPositionLevel {
                    parent_id: "lib-movies".into(),
                    title: "Movies".into(),
                    focused_item_id: Some("folder-b".into()),
                    cursor_index: 1,
                    item_types: Some("Movie".into()),
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    letter_filter_index: None,
                    library_total: None,
                },
                crate::config::LibraryPositionLevel {
                    parent_id: "folder-b".into(),
                    title: "Folder B".into(),
                    focused_item_id: Some("leaf-1".into()),
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
        },
    );

    app.ensure_lib_loaded_for(0);

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    let level = &app.libs[0].nav_stack[0];
    assert_eq!(level.parent_id, "lib-movies");
    assert_eq!(level.title, "Movies");
    assert!(level.loading);
    assert!(level.items.is_empty());
    assert_eq!(level.item_types.as_deref(), Some("Movie"));
}

#[test]
fn activating_saved_power_position_initializes_feed_home_video_state() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("Youtube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
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
    app.library_position_state.libraries.insert(
        "lib-youtube".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-youtube".into(),
                title: "Youtube".into(),
                focused_item_id: None,
                cursor_index: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            }],
            feed_selected_group: 0,
            feed_video_cursor: 2,
            feed_video_scroll: 1,
        },
    );

    app.activate_library_position(0);

    let feed = app.libs[0]
        .feed_home_video
        .as_ref()
        .expect("saved feed library position should initialize feed state");
    assert!(feed.loading);
    assert_eq!(feed.video_cursor, 2);
    assert_eq!(feed.video_scroll, 1);
    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn ensure_lib_loaded_for_visible_power_library_accepts_restore_from_queue_focus() {
    let mut app = make_app_stub();
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

    app.ensure_lib_loaded_for(0);

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert!(app.libs[0].nav_stack[0].loading);
    assert_eq!(app.libs[0].nav_stack[0].title, "Power");

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: power_position.clone(),
        position: power_position,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Power restored".into(),
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
        }],
    });

    assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
    assert!(!app.libs[0].nav_stack[0].loading);
}

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

// #361: `set_tab` (the Standard tab-switch entry point) is gone; the
// scope-isolation premise this test exercised ("default scope survives
// a power-scope write") no longer applies -- there is one saved
// position per library now (see `save_default_library_position_persists_focused_item`).
#[test]
fn library_tab_next_activates_saved_placeholder() {
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
    app.library_position_state.libraries.insert(
        "lib-movies".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Saved".into(),
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
        },
    );
    app.library_tab = 0;

    app.library_tab_next();

    assert_eq!(app.library_tab, 1);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.libs[0].nav_stack[0].title, "Saved");
    assert!(app.libs[0].nav_stack[0].loading);
}

#[test]
fn library_tab_next_from_queue_focus_accepts_restore_result() {
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
    app.panel_focus = PanelFocus::Queue;
    app.library_tab = 0;

    app.library_tab_next();

    assert_eq!(app.library_tab, 1);
    assert_eq!(app.panel_focus, PanelFocus::Library);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert!(app.libs[0].nav_stack[0].loading);

    app.handle_lib_event(LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: power_position.clone(),
        position: power_position,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Power restored".into(),
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
        }],
    });

    assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
    assert!(!app.libs[0].nav_stack[0].loading);
}

#[test]
fn build_restores_panel_focus_from_prefs_for_both_values() {
    let _guard = crate::config::TestStateDirGuard::new();
    for (pref, expected) in [
        ("queue_side", PanelFocus::Queue),
        ("library_side", PanelFocus::Library),
    ] {
        std::fs::write(
            crate::config::prefs_path(),
            serde_json::json!({ "panel_focus": pref }).to_string(),
        )
        .expect("write prefs");

        let app = make_built_app();

        assert_eq!(app.panel_focus, expected);
    }
}

#[test]
fn save_prefs_persists_panel_focus_for_both_values() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();

    app.set_panel_focus(PanelFocus::Queue);
    let queue_prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(queue_prefs["panel_focus"].as_str(), Some("queue_side"));

    app.set_panel_focus(PanelFocus::Library);
    let library_prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(library_prefs["panel_focus"].as_str(), Some("library_side"));
}

#[test]
fn entering_power_queue_focus_selects_now_playing_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 2;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.player_tab.queue_cursor, 1);
}

#[test]
fn entering_power_queue_focus_preserves_valid_queue_cursor_without_now_playing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 2;

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.player_tab.queue_cursor, 2);
}

#[test]
fn entering_power_queue_focus_defaults_invalid_queue_cursor_to_first_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 99;

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.player_tab.queue_cursor, 0);
}

#[test]
fn building_from_panel_focus_prefs_does_not_mutate_saved_library_positions() {
    let _guard = crate::config::TestStateDirGuard::new();
    let state = crate::config::LibraryPositionState {
        libraries: std::iter::once((
            "lib-movies".into(),
            crate::config::LibraryPosition {
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
            },
        ))
        .collect(),
    };
    crate::config::save_library_position_state(&state);
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "panel_focus": "queue_side" }).to_string(),
    )
    .expect("write prefs");

    let _app = make_built_app();

    assert_eq!(crate::config::load_library_position_state(), state);
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

#[test]
fn refresh_lib_clears_saved_position_for_active_library() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    app.replace_saved_library_position(
        0,
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Saved".into(),
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
        },
    );

    app.refresh_lib();

    assert!(!crate::config::load_library_position_state()
        .libraries
        .contains_key("lib-movies"));
}

#[test]
fn trigger_lib_rescan_clears_only_active_scope() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(2),
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.library_tab = 1;
    app.replace_saved_library_position(
        0,
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Saved".into(),
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
        },
    );

    app.trigger_lib_rescan(0);

    assert!(!crate::config::load_library_position_state()
        .libraries
        .contains_key("lib-movies"));
}

#[test]
fn power_home_navigation_does_not_persist_library_position_state() {
    let mut app = make_app_stub();
    app.library_tab = 0;
    app.home.continue_items = make_items(3);

    app.power_cw_move_cursor(1);

    assert!(app.library_position_state.libraries.is_empty());
}
