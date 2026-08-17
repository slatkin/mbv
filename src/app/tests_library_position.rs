use super::*;
use crate::app::tests::*;

#[test]
fn library_position_snapshot_captures_path_focus_and_feed_group() {
    let mut lib = LibraryTab {
        library: make_item("Movies", "CollectionFolder"),
        search: Some(LibSearch {
            query: "ignored".into(),
            items: make_items(2),
            results: vec![0],
            cursor: 0,
            scroll: 0,
            loading: false,
        }),
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
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 2,
            video_cursor: 4,
            video_scroll: 3,
            ..Default::default()
        }),
        album_track_focus: Some(1),
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
        search: Some(LibSearch {
            query: "ignored".into(),
            items: make_items(2),
            results: vec![0],
            cursor: 0,
            scroll: 0,
            loading: false,
        }),
        nav_stack: Vec::new(),
        feed_home_video: Some(FeedHomeVideoState::default()),
        album_track_focus: Some(2),
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
            music_grouping: None,
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
    app.tab = TabSelection::EmbyLibrary(0);
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
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
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
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
        search: None,
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
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.move_lib_cursor(0, 1);

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
        search: None,
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
        feed_home_video: None,
        album_track_focus: None,
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
fn refresh_lib_clears_saved_position_for_active_library() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
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
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);
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

    app.refresh_lib(0);

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
        search: None,
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
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.tab = TabSelection::EmbyLibrary(0);
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
fn home_navigation_does_not_persist_library_position_state() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.home.continue_items = make_items(3);

    app.cw_move_cursor(1);

    assert!(app.library_position_state.libraries.is_empty());
}
