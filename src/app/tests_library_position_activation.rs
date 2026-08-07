use super::*;
use crate::app::tests::*;

#[test]
fn ensure_lib_loaded_for_uses_saved_position_loading_state_without_root_flash() {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: Vec::new(),
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
        search: None,
        nav_stack: Vec::new(),
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
        search: None,
        nav_stack: Vec::new(),
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
            music_grouping: None,
        }],
    });

    assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
    assert!(!app.libs[0].nav_stack[0].loading);
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
        search: None,
        nav_stack: Vec::new(),
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
        search: None,
        nav_stack: Vec::new(),
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
            music_grouping: None,
        }],
    });

    assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
    assert!(!app.libs[0].nav_stack[0].loading);
}
