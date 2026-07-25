use super::ui_util::fmt_duration;
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use mbv_core::api::device_name;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use unicode_width::UnicodeWidthStr;

pub(crate) fn make_item(name: &str, item_type: &str) -> MediaItem {
    MediaItem {
        id: "id".into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

pub(crate) fn make_session(device_name: &str, client: &str) -> mbv_core::api::SessionInfo {
    mbv_core::api::SessionInfo {
        id: "sess-1".into(),
        device_name: device_name.into(),
        client: client.into(),
        user_name: "user".into(),
        host: "127.0.0.1".into(),
        supported_commands: Vec::new(),
        now_playing: None,
        now_playing_item_id: None,
        position_s: 0,
        runtime_s: 0,
        is_paused: false,
        volume: 100,
        sub_index: -1,
        audio_index: 1,
        muted: false,
        media_info: mbv_core::api::SessionMediaInfo::default(),
    }
}

/// Minimal daemon-side protocol handshake for tests that need a real
/// TCP socket `RemotePlayer::connect_endpoint` can connect to (#233):
/// sends the protocol hello, drains the client's hello line, then
/// sends an empty initial state. Returns the accepted `TcpStream` so
/// the caller can observe what happens to it afterward (e.g. that the
/// client shuts it down).
pub(crate) fn run_stub_daemon_handshake(stream: std::net::TcpStream) -> std::net::TcpStream {
    use std::io::{BufRead, BufReader, Write};
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    let hello = serde_json::to_string(&mbv_core::ctrl::CtrlEvent::Hello(
        mbv_core::ctrl::CtrlHello::current(),
    ))
    .unwrap();
    writeln!(writer, "{hello}").unwrap();

    let mut client_hello = String::new();
    reader.read_line(&mut client_hello).unwrap();

    let initial_state = serde_json::to_string(&mbv_core::ctrl::CtrlEvent::State(
        mbv_core::ctrl::CtrlState {
            status: mbv_core::player::PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
        },
    ))
    .unwrap();
    writeln!(writer, "{initial_state}").unwrap();

    stream
}

// ── fmt_duration ─────────────────────────────────────────────────────────

#[test]
fn fmt_duration_zero() {
    assert_eq!(fmt_duration(0), "0:00");
}

#[test]
fn fmt_duration_seconds_only() {
    assert_eq!(fmt_duration(45), "0:45");
}

#[test]
fn fmt_duration_minutes_and_seconds() {
    assert_eq!(fmt_duration(90), "1:30");
    assert_eq!(fmt_duration(3599), "59:59");
}

#[test]
fn fmt_duration_hours() {
    assert_eq!(fmt_duration(3600), "1:00:00");
    assert_eq!(fmt_duration(3661), "1:01:01");
    assert_eq!(fmt_duration(7384), "2:03:04");
}

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

// ── test helpers ─────────────────────────────────────────────────────────

pub(crate) fn make_items(n: usize) -> Vec<MediaItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("Item {i}"), "Movie");
            item.id = format!("id{i}");
            item
        })
        .collect()
}

pub(crate) fn make_audio_items(n: usize) -> Vec<MediaItem> {
    (0..n)
        .map(|i| {
            let mut item = make_item(&format!("Track {i}"), "Audio");
            item.id = format!("id{i}");
            item.media_type = "Audio".into();
            item
        })
        .collect()
}

/// Minimal App stub for logic-only tests.
pub(crate) fn make_app_stub() -> App {
    use mbv_core::player::{PlayerProxy, PlayerStatus};
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(PlayerStatus {
        volume_max: 100,
        ..Default::default()
    }));

    let (_, player_rx) = std::sync::mpsc::channel();
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (lib_tx, lib_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    // No worker thread spawned here: `image_picker` is always `None` in
    // this stub, so no `ThreadProtocol` is ever built and nothing sends
    // on `resize_register_tx`/reads `resize_response_rx`.
    let (resize_register_tx, _resize_register_rx) = std::sync::mpsc::channel();
    let (_resize_response_tx, resize_response_rx) = std::sync::mpsc::channel();
    let (notif_action_tx, notif_action_rx) = std::sync::mpsc::channel::<String>();
    let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();
    let (search_tx, search_rx) = std::sync::mpsc::channel::<Result<Vec<MediaItem>, String>>();

    let player = PlayerProxy::stub(status.clone());

    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    let client = EmbyClient::new(Config::default());

    App {
        _test_state_dir_guard: crate::config::TestStateDirGuard::new_if_unset(),
        client: Arc::new(Mutex::new(client)),
        player,
        mpris: None,
        launched_as_remote: false,
        player_rx,
        ws_rx,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::HashMap::new(),
        hidden_latest: Vec::new(),
        music_levels: Vec::new(),
        album_indexes: std::collections::HashMap::new(),
        player_tab: PlayerTab::default(),
        remote_player_tab: None,
        home: HomePane {
            continue_items: Vec::new(),
            continue_cursor: 0,
            latest: Vec::new(),
            section: 0,
            home_cursor: 0,
            home_scroll: 0,
        },
        libs: Vec::new(),
        status: String::new(),
        status_expires: None,
        layout: layout::AppLayout::default(),
        terminal_width: 80,
        terminal_height: 24,

        home_loading: false,
        mouse_col: 0,
        mouse_row: 0,
        last_click_time: std::time::Instant::now(),
        last_drag_seek: std::time::Instant::now(),
        last_click_pos: (u16::MAX, u16::MAX),
        last_space_press: None,
        last_esc_press: None,
        confirm_remove_idx: None,
        pending_delete_idx: None,
        pending_queue_removal: None,
        confirm_clear_queue: false,
        queue_undo_stack: Vec::new(),
        remote_queue_undo_stack: Vec::new(),
        pending_remote_move_cursor: None,
        skip_intro_end_ticks: None,
        next_up_item: None,
        panel_focus: PanelFocus::default(),
        library_tab: 0,
        queue_column_width: POWER_LEFT_WIDTH_DEFAULT,
        queue_column_collapsed: false,
        library_tab_pending: 0,
        queue_scroll: 0,
        last_played_item_id: None,
        last_played_completed: false,
        card_image_states: std::collections::HashMap::new(),
        card_image_loading: std::collections::HashSet::new(),
        last_card_height: 0,
        card_image_tx,
        card_image_rx,
        resize_register_tx,
        resize_response_rx,
        image_picker: None,
        show_help: false,
        show_settings: false,
        settings_cursor: 0,
        settings_scroll: 0,
        settings_save_at: None,
        confirm_logout: false,
        multiselect_popup: None,
        library_routes_popup: None,
        help_scroll: 0,
        system_notifications: false,
        notif_failed: false,
        notif_action_tx,
        notif_action_rx,
        context_menu: None,
        lib_tx,
        lib_rx,
        search: SearchSubsystem::new(search_tx, search_rx),
        force_clear: false,
        tab_scroll: 0,
        ui_volume: 100,
        pre_mute_volume: None,
        mute_on: false,
        sessions: Vec::new(),
        sessions_cursor: 0,
        sessions_scroll: 0,
        sessions_loading: false,
        show_sessions: false,
        playlists: Vec::new(),
        playlists_cursor: 0,
        playlists_scroll: 0,
        playlists_loading: false,
        show_playlists: false,
        playlists_open: None,
        playlists_open_items: Vec::new(),
        playlists_open_cursor: 0,
        playlists_open_scroll: 0,
        playlists_open_loading: false,
        queue_source: crate::config::QueueSource::Unknown,
        queue_dirty: false,
        pending_queue_action: None,
        show_save_playlist_modal: false,
        use_nerd_fonts: false,
        indicator_style: Default::default(),
        ws_send_tx: None,
        last_keepalive: Instant::now(),
        last_capabilities: Instant::now(),
        sessions_tx,
        sessions_rx,
        connected_session_id: None,
        connected_session_state: None,
        direct_remote_connected: false,
        direct_remote_label: None,
        last_session_poll: std::time::Instant::now(),
        session_miss_count: 0,
        remote_pos_s: 0,
        remote_pos_at: std::time::Instant::now(),
        remote_api_pos_advanced_at: std::time::Instant::now() - Duration::from_secs(60),
        remote_seek_pending_until: std::time::Instant::now() - Duration::from_secs(1),
        runtime_zero_since: None,
        suspended_local: None,
        active_route: None,
        library_route_cache: std::collections::HashMap::new(),
        last_scroll_at: Instant::now() - Duration::from_secs(1),
        last_nav_at: Instant::now() - Duration::from_secs(1),
        last_power_library_nav_at: Instant::now() - Duration::from_secs(1),
        refocus_at: None,
        album_artist_cache: std::collections::HashMap::new(),
        album_artist_loading: std::collections::HashSet::new(),
        pending_album_artist_fetches: std::collections::VecDeque::new(),
        album_artist_fetches_active: 0,
        album_tracks_cache: std::collections::HashMap::new(),
        album_tracks_loading: std::collections::HashSet::new(),
        series_detail_cache: std::collections::HashMap::new(),
        series_detail_loading: std::collections::HashSet::new(),
        save_playlist_dialog: None,
        image_lru: std::collections::VecDeque::new(),
        pending_image_fetches: std::collections::VecDeque::new(),
        image_fetches_active: 0,
        image_cache_size: 50,
        image_protocol: None,
        image_protocol_enabled: false,
        confirm_rescan: false,
        pending_rescan_lib_idx: None,
        library_position_state: crate::config::LibraryPositionState::default(),
        queue_scope: QueueScope::Local,
        stay_alive_ctrl: None,
        attached: true,
    }
}

// #202: the in-app quit-key path used to join the player thread
// unboundedly, so a hanging shutdown-time network call could hold the
// single-instance flock indefinitely. `teardown` is the extracted,
// testable sequence both the normal quit-key path and the
// SIGHUP/SIGTERM watchdog path now share; this proves it actually
// returns within its bounded window against a player thread that
// hangs well past the configured `quit_timeout`, without needing a
// real tty (`run()` itself stays untested end-to-end, unchanged status
// quo).
#[test]
fn teardown_bounds_previously_unbounded_normal_quit_join() {
    use mbv_core::player::PlayerProxy;

    let mut app = make_app_stub();
    let status = app.player.status.clone();
    // Thread hangs for 10s; quit_timeout below is 200ms, so a correctly
    // bounded teardown (outer_bound = quit_timeout + quit_timeout/2 +
    // 3s cushion ~= 3.3s here) must return well short of the full hang.
    app.player = PlayerProxy::stub_with_hung_thread(status, Duration::from_secs(10));

    let started = std::time::Instant::now();
    app.teardown(Duration::from_millis(200));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "teardown should return within its bounded window (outer_bound \
             ~= 3.3s here), took {elapsed:?} — previously unbounded on the \
             normal in-app quit path, which is the #202 bug"
    );
}

#[test]
fn teardown_fast_when_player_thread_is_not_hung() {
    let mut app = make_app_stub();

    let started = std::time::Instant::now();
    app.teardown(Duration::from_secs(5));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "teardown against a player with no thread to join should return \
             promptly, not wait anywhere near the quit_timeout budget, took {elapsed:?}"
    );
}

#[test]
fn teardown_persists_active_library_route_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    app.active_route = Some("music".to_string());

    app.teardown(Duration::from_secs(1));

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

#[test]
fn teardown_persists_connected_session_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    app.connected_session_id = Some(sess.id.clone());
    app.connected_session_state = Some(sess);

    app.teardown(Duration::from_secs(1));

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
    );
}

#[test]
fn teardown_persists_direct_remote_when_auto_reconnect_enabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    let sess = make_session("living-room-mbv", "mbv");
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_direct_remote(&sess, remote, remote_rx);
    app.teardown(Duration::from_secs(1));

    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string()
        })
    );
}

#[test]
fn teardown_clears_persisted_connection_when_exiting_local() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.teardown(Duration::from_secs(1));

    assert_eq!(crate::config::load_last_remote_connection().unwrap(), None);
}

#[test]
fn teardown_never_touches_persisted_state_when_auto_reconnect_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    assert!(!app.client.lock().unwrap().config.auto_reconnect);
    app.active_route = None;

    app.teardown(Duration::from_secs(1));

    // Feature is off: the file from before this test's own `app` even
    // existed must be left exactly as it was, not cleared just because
    // `active_route` is currently `None`.
    assert_eq!(
        crate::config::load_last_remote_connection().unwrap(),
        Some(crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string()
        })
    );
}

pub(crate) fn make_built_app() -> App {
    use mbv_core::player::{PlayerProxy, PlayerStatus};
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(PlayerStatus {
        volume_max: 100,
        ..Default::default()
    }));

    let (_, player_rx) = std::sync::mpsc::channel();
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (lib_tx, lib_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    let (notif_action_tx, notif_action_rx) = std::sync::mpsc::channel::<String>();
    let (sessions_tx, sessions_rx) = std::sync::mpsc::channel();
    let (search_tx, search_rx) = std::sync::mpsc::channel::<Result<Vec<MediaItem>, String>>();

    let player = PlayerProxy::stub(status);

    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    let client = EmbyClient::new(Config::default());

    App::build(AppInit {
        client: Arc::new(Mutex::new(client)),
        player,
        player_rx,
        ws_rx,
        ws_send_tx: None,
        player_tab: PlayerTab::default(),
        remote_player_tab: None,
        initial_queue_scope: QueueScope::Local,
        system_notifications: false,
        image_protocol: None,
        image_protocol_enabled: false,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::HashMap::new(),
        hidden_latest: Vec::new(),
        music_levels: Vec::new(),
        use_nerd_fonts: false,
        indicator_style: render::indicators::IndicatorStyle::default(),
        image_cache_size: 50,
        lib_tx,
        lib_rx,
        sessions_tx,
        sessions_rx,
        card_image_tx,
        card_image_rx,
        notif_action_tx,
        notif_action_rx,
        search_tx,
        search_rx,
        stay_alive_ctrl: None,
    })
}

// ── wants_terminal_render (#156: detached stay-alive must not touch
// the terminal — Terminal::clear() blocks on a cursor-position DSR
// query nobody answers once the terminal-client has detached) ────────

#[test]
fn wants_terminal_render_true_when_attached_and_due() {
    let mut app = make_app_stub();
    app.attached = true;
    let stale = Instant::now() - Duration::from_secs(10);
    assert!(app.wants_terminal_render(false, stale, Duration::from_secs(1)));
}

#[test]
fn wants_terminal_render_false_when_detached_even_with_events_and_force_clear() {
    let mut app = make_app_stub();
    app.attached = false;
    app.force_clear = true;
    let stale = Instant::now() - Duration::from_secs(10);
    // had_events, force_clear, and an elapsed render_interval would all
    // independently demand a render while attached -- none of them may
    // override `attached == false`, or the run loop calls
    // Terminal::clear()/draw() with nobody left to answer the pty.
    assert!(!app.wants_terminal_render(true, stale, Duration::from_secs(1)));
}

#[test]
fn wants_terminal_render_false_when_detached_and_idle() {
    let app = make_app_stub();
    let mut app = app;
    app.attached = false;
    let recent = Instant::now();
    assert!(!app.wants_terminal_render(false, recent, Duration::from_secs(1)));
}

// A compact-banner poster fetch (or any list-image prefetch) can easily
// outlast the idle render cadence: with nothing playing and no remote
// session, the run loop only repaints once a second unless something
// sets `had_events` (a key/mouse event, or the fetch itself completing).
// That meant a loading placeholder was computed correctly by
// `compact_banner_layout` but never actually painted -- the only two
// frames drawn were "just navigated, fetch not even started yet" and
// "fetch just completed", with nothing in between showing the reserved
// placeholder box. Treating an in-flight image fetch the same as active
// playback (fast 150ms cadence instead of the 1s idle one) gives the
// loop a reason to repaint while the placeholder should be visible.
#[test]
fn render_interval_is_fast_while_a_card_image_fetch_is_in_flight() {
    let mut app = make_app_stub();
    app.card_image_loading.insert("movie-1:cmp_primary".into());
    assert_eq!(app.render_interval(), Duration::from_millis(150));
}

#[test]
fn render_interval_is_slow_when_idle_with_no_fetches_in_flight() {
    let app = make_app_stub();
    assert_eq!(app.render_interval(), Duration::from_secs(1));
}

#[test]
fn try_quit_bare_mode_does_not_touch_attached() {
    let mut app = make_app_stub();
    app.attached = true;
    // No `stay_alive_ctrl` -> bare mode -> `attached` is irrelevant and
    // must stay untouched (it's never consulted outside stay-alive).
    let _ = app.try_quit();
    assert!(app.attached);
}

#[test]
fn try_quit_stay_alive_detach_clears_attached_and_notifies_relay() {
    let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut app = make_app_stub();
    app.attached = true;
    app.client.lock().unwrap().config.stay_alive = true;
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));

    let quit_loop_should_exit = app.try_quit();

    assert!(
        !quit_loop_should_exit,
        "stay-alive `q` must detach, never quit the run loop"
    );
    assert!(
        !app.attached,
        "detach must clear `attached` so the run loop skips terminal I/O \
             until the next reattach (#156)"
    );

    // And it must have actually told the relay to detach, not just
    // flipped local state.
    use std::io::Read;
    relay_end.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 32];
    let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
    assert_eq!(&buf[..n], b"DETACH\n");
}

#[test]
fn try_quit_stay_alive_session_exits_when_setting_disabled() {
    let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut app = make_app_stub();
    app.attached = true;
    app.client.lock().unwrap().config.stay_alive = false;
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));

    let quit_loop_should_exit = app.try_quit();

    assert!(
        quit_loop_should_exit,
        "disabling Stay alive on exit must make the next `q` quit this attached session"
    );
    assert!(
        app.attached,
        "real quit should not flip the detached-session guard"
    );

    use std::io::Read;
    relay_end.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 32];
    let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
    assert_eq!(
        n, 0,
        "runtime quit path must not tell the relay to detach once Stay alive on exit is disabled"
    );
}

#[test]
fn stay_alive_settings_toggle_changes_current_session_q_behavior_both_ways() {
    let (app_end, relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.stay_alive = true;
    app.attached = true;
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));
    app.settings_cursor = (0..settings::settings_total_rows())
        .find(|&idx| settings::settings_cursor_to_key(idx) == SettingKey::StayAlive)
        .expect("StayAlive setting row must exist");

    app.handle_settings_activate();
    assert!(
        !app.client.lock().unwrap().config.stay_alive,
        "settings toggle should disable Stay alive on exit for the current session too"
    );
    assert!(
        app.try_quit(),
        "disabling Stay alive on exit should make q quit even while a relay control channel exists"
    );

    app.handle_settings_activate();
    assert!(
        app.client.lock().unwrap().config.stay_alive,
        "re-enabling Stay alive on exit should restore detach-on-q in the same stay-alive session"
    );
    assert!(
        !app.try_quit(),
        "re-enabling Stay alive on exit should restore detach-on-q in the same stay-alive session"
    );

    use std::io::Read;
    relay_end.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 32];
    let n = relay_end.take(32).read(&mut buf).unwrap_or(0);
    assert_eq!(&buf[..n], b"DETACH\n");
}

#[test]
fn auto_reconnect_settings_row_displays_and_toggles_current_session() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = false;
    app.settings_cursor = (0..settings::settings_total_rows())
        .find(|&idx| settings::settings_cursor_to_key(idx) == SettingKey::AutoReconnect)
        .expect("AutoReconnect setting row must exist");

    let cfg = app.client.lock().unwrap().config.clone();
    assert_eq!(
        settings::setting_label(SettingKey::AutoReconnect),
        "Auto reconnect"
    );
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "off"
    );

    app.handle_settings_activate();
    let cfg = app.client.lock().unwrap().config.clone();
    assert!(cfg.auto_reconnect);
    assert_eq!(
        settings::setting_value(SettingKey::AutoReconnect, &cfg, &app.ui_config_snapshot()),
        "on"
    );
    assert!(
        app.settings_save_at.is_some(),
        "settings toggle must use the delayed save path"
    );

    app.handle_settings_activate();
    assert!(!app.client.lock().unwrap().config.auto_reconnect);
}

fn left_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();

    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render_app_to_terminal(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term
}

// ── transport_prev_next_available (issue #112) ─────────────────────────
// Drives whether playback transport is currently available at the queue
// boundaries. The header uses the `next` half directly, while the `P`/`N`
// keys still reuse both halves.

#[test]
fn transport_prev_next_unavailable_when_player_inactive() {
    let app = make_app_stub();
    assert!(!app.player.status.lock().unwrap().active);
    assert_eq!(app.transport_prev_next_available(), (false, false));
}

#[test]
fn transport_prev_next_both_available_mid_queue() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 1;
    }
    assert_eq!(app.transport_prev_next_available(), (true, true));
}

#[test]
fn transport_prev_unavailable_on_first_item() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 0;
    }
    assert_eq!(app.transport_prev_next_available(), (false, true));
}

#[test]
fn transport_next_unavailable_on_last_item() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 2;
    }
    assert_eq!(app.transport_prev_next_available(), (true, false));
}

#[test]
fn transport_prev_next_both_unavailable_on_single_item_queue() {
    let app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
    }
    assert_eq!(app.transport_prev_next_available(), (false, false));
}

#[test]
fn transport_prev_next_both_available_for_connected_remote_session_regardless_of_local_status() {
    // SessionInfo (see mbv_core::api::SessionInfo) exposes no
    // queue-position/length fields, so there's no boundary to check for a
    // connected remote session. Local status here is deliberately set to
    // "last item" to prove it's ignored while a session is connected.
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 3;
        st.current_idx = 2;
    }
    assert_eq!(app.transport_prev_next_available(), (true, true));
}

fn make_remote_app_stub(local_items: Vec<MediaItem>, remote_items: Vec<MediaItem>) -> App {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
    app.player_tab.items = local_items;
    app.player_tab.queue_cursor = 0;
    app
}

fn make_remote_app_stub_with_cmd_rx(
    local_items: Vec<MediaItem>,
    remote_items: Vec<MediaItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_core::ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(remote_items, 0);
    let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
    app.player_tab.items = local_items;
    app.player_tab.queue_cursor = 0;
    (app, cmd_rx)
}

fn make_v2_remote_app_stub_with_cmd_rx(
    local_items: Vec<MediaItem>,
    remote_items: Vec<MediaItem>,
) -> (App, std::sync::mpsc::Receiver<mbv_core::ctrl::CtrlCmd>) {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_v2_with_command_rx(remote_items, 0);
    let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
    app.player_tab.items = local_items;
    app.player_tab.queue_cursor = 0;
    (app, cmd_rx)
}

fn make_local_daemon_app_stub(remote_items: Vec<MediaItem>) -> App {
    use crate::config::Config;
    use mbv_core::api::EmbyClient;

    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, true)
}

#[test]
fn local_daemon_bootstrap_adopts_saved_local_queue_and_source() {
    let items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Playlist {
                id: Some("pl1".into()),
                name: "Saved".into(),
            },
            items,
            cursor: 1,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.items.len(), 2);
    assert_eq!(bootstrap.player_tab.queue_cursor, 1);
    assert!(matches!(
        bootstrap.queue_source,
        crate::config::QueueSource::Playlist { ref name, .. } if name == "Saved"
    ));
    assert!(matches!(
        bootstrap.adopt_queue,
        Some((_, 1, crate::config::QueueSource::Playlist { ref name, .. })) if name == "Saved"
    ));
}

#[test]
fn failed_local_daemon_adoption_routes_through_remote_disconnected() {
    // #119 task 5: a swallowed `adopt_queue()` send-failure must not
    // leave the app silently sitting on optimistic queue state the
    // daemon never received — it routes through the same handling a
    // live `PlayerEvent::RemoteDisconnected` uses.
    let mut app = make_local_daemon_app_stub(Vec::new());
    assert_eq!(app.queue_scope, QueueScope::Local);

    app.handle_failed_local_daemon_adoption();

    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert!(app.status.contains("daemon connection lost"));
}

#[test]
fn remote_app_starts_on_local_queue_when_remote_queue_is_empty() {
    let app = make_remote_app_stub(make_items(2), Vec::new());

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn remote_app_starts_on_remote_queue_when_remote_queue_has_items() {
    let app = make_remote_app_stub(make_items(2), make_items(1));

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
}

#[test]
fn local_daemon_bootstrap_carries_saved_positions_for_enrichment() {
    let items = make_items(2);
    let mut positions = std::collections::HashMap::new();
    positions.insert(items[0].id.clone(), 999);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items,
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: positions.clone(),
        }),
    );

    assert_eq!(bootstrap.positions, positions);
}

#[test]
fn local_daemon_bootstrap_has_no_positions_without_saved_state() {
    let bootstrap =
        bootstrap_local_daemon_queue(Vec::new(), 0, crate::config::QueueSource::Unknown, None);

    assert!(bootstrap.positions.is_empty());
}

#[test]
fn local_daemon_bootstrap_uses_restore_cursor_and_carries_last_played_state() {
    let items = make_items(3);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: items.clone(),
            cursor: 0,
            last_played_item_id: Some(items[1].id.clone()),
            last_played_completed: true,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.queue_cursor, 2);
    assert_eq!(
        bootstrap.last_played_item_id.as_deref(),
        Some(items[1].id.as_str())
    );
    assert!(bootstrap.last_played_completed);
}

#[test]
fn local_daemon_bootstrap_prefers_existing_daemon_queue_state() {
    let remote_items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        remote_items.clone(),
        0,
        crate::config::QueueSource::Playlist {
            id: Some("daemon".into()),
            name: "Daemon Queue".into(),
        },
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Playlist {
                id: Some("local".into()),
                name: "Local Saved".into(),
            },
            items: make_items(1),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.items.len(), 2);
    assert_eq!(bootstrap.player_tab.items[0].id, remote_items[0].id);
    assert!(matches!(
        bootstrap.queue_source,
        crate::config::QueueSource::Playlist { ref name, .. } if name == "Daemon Queue"
    ));
    assert!(bootstrap.adopt_queue.is_none());
}

#[test]
fn session_direct_endpoint_prefers_advertised_tcp_port() {
    let app = make_app_stub();
    let mut sess = make_session("remote-host", "mbv");
    sess.host = "192.168.1.20".into();
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];
    assert_eq!(
        app.session_direct_endpoint(&sess),
        Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
            "192.168.1.20:47788".parse().unwrap()
        ))
    );
}

#[test]
fn session_direct_endpoint_rejects_non_mbv_without_local_fallback() {
    let app = make_app_stub();
    let sess = make_session("other-host", "Emby");
    assert_eq!(app.session_direct_endpoint(&sess), None);
}

#[test]
fn session_direct_endpoint_falls_back_to_local_socket_for_same_host_session() {
    let app = make_app_stub();
    let device_name = app.client.lock().unwrap().device_name.clone();
    let sess = make_session(&device_name, "mbv");
    assert_eq!(
        app.session_direct_endpoint(&sess),
        Some(mbv_core::remote_player::DaemonEndpoint::Local)
    );
}

#[test]
fn f3_direct_upgrade_with_empty_device_name_remains_disconnectable() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
    let mut app = make_app_stub();
    let mut sess = make_session("", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.remote_player_tab.is_some());
    assert!(app.connected_session_id.is_none());
    assert!(app.direct_remote_label.is_none());
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(!app.player.is_remote());
    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.status, "Disconnected from direct remote session");
}

#[test]
fn connect_to_session_preserves_direct_upgrade_failure_status_after_fallback() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("incompatible daemon protocol version: peer=1 local=3".to_string())
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_failure);
    let mut app = make_app_stub();
    let mut sess = make_session("remote-mbv", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.connected_session_id.as_deref(), Some("sess-1"));
    assert_eq!(
            app.status,
            "Direct mbv control failed: incompatible daemon protocol version: peer=1 local=3; using attached session remote-mbv"
        );
}

#[test]
fn connect_to_session_tears_down_an_active_library_route_via_restore_local_mode() {
    // Regression guard: `connect_to_session`'s direct-upgrade attempt
    // is itself gated on `!self.player.is_remote()`, so
    // `switch_to_direct_remote`'s already-remote branch is never
    // reached from here -- a bare `self.active_route = None;` right
    // before that call would be dead code for this scenario. The fix
    // is to tear down any active library route through
    // `restore_local_mode` at the top of the function instead, which
    // both clears `active_route` AND restores the suspended local
    // `Player` (via the real `switch_to_library_route` path, not a
    // manually-poked field), so the subsequent `!self.player.is_remote()`
    // check is true and the direct-upgrade attempt actually runs.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
    let mut app = make_app_stub();
    // Really go through a library route (#223), not a manually-poked
    // field, so `suspended_local` is populated the way it is in
    // production and `restore_local_mode` has real state to restore.
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx);
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());

    let mut sess = make_session("remote-mbv", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    // The direct-upgrade attempt ran (not skipped) because the library
    // route's remote player was properly restored to local first, so
    // the app ends up on the Sessions-panel direct remote, not stuck
    // on the stale library-route connection.
    assert!(app.player.is_remote());
    assert!(app.direct_remote_label.is_some());
}

#[test]
fn connect_to_session_is_a_no_op_teardown_when_no_library_route_is_active() {
    // The new top-of-function teardown must not disturb the existing,
    // already-covered "plain local player" path when there is no
    // library route to tear down.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    fn direct_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(direct_success);
    let mut app = make_app_stub();
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());

    let mut sess = make_session("remote-mbv", "mbv");
    sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(47788)];

    app.connect_to_session(&sess);

    *DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    assert!(app.player.is_remote());
    assert!(app.direct_remote_label.is_some());
}

#[test]
fn try_daemon_route_connect_returns_remote_player_on_successful_connect() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);
    let app = make_app_stub();
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(result.is_ok());
}

#[test]
fn try_auto_reconnect_restores_a_persisted_library_route() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());

    app.try_auto_reconnect();

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
}

#[test]
fn try_auto_reconnect_falls_back_to_local_when_route_no_longer_configured() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;
    // No `library_routes` entry for "music" this time -- config changed
    // since the last exit.

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_auto_reconnect_restores_a_persisted_direct_session() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
    fn sessions_with_living_room(
        _client: &mbv_core::api::EmbyClient,
    ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        Ok(vec![make_session("living-room-mbv", "mbv")])
    }
    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(sessions_with_living_room);

    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.try_auto_reconnect();

    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.connected_session_id.as_deref(), Some("sess-1"));
}

#[test]
fn try_auto_reconnect_falls_back_to_local_when_device_not_found() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
    fn sessions_without_living_room(
        _client: &mbv_core::api::EmbyClient,
    ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
        Ok(vec![])
    }
    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(sessions_without_living_room);

    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string(),
        },
    ));
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.try_auto_reconnect();

    *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
    assert!(app.connected_session_id.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_auto_reconnect_is_a_no_op_when_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _ = crate::config::save_last_remote_connection(Some(
        &crate::config::LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        },
    ));
    let mut app = make_app_stub();
    assert!(!app.client.lock().unwrap().config.auto_reconnect);
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_auto_reconnect_is_a_no_op_when_nothing_was_persisted() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.auto_reconnect = true;

    app.try_auto_reconnect();

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure() {
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    let app = make_app_stub();
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Unix(std::path::PathBuf::from(
        "/tmp/mbv-music.sock",
    ));

    let result = app.try_daemon_route_connect(&endpoint, "Music");

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    // `RemotePlayer` derives only `Clone` (no `PartialEq`/`Debug` --
    // confirmed against `crates/mbv-core/src/remote_player.rs`), so the
    // whole `Result` can't go through `assert_eq!` directly; match out
    // the `Err` payload instead.
    match result {
        Ok(_) => panic!("expected a connect failure to return Err, got Ok"),
        Err(message) => {
            assert_eq!(
                message,
                "\u{26a0} Music route unreachable, using local playback (mbv.log)"
            );
        }
    }
    // The primitive itself must never flash -- that is the caller's
    // job (see Architecture). `make_app_stub()` starts with an empty
    // status, so this pins down that `try_daemon_route_connect` left
    // it untouched.
    assert!(app.status.is_empty());
}

#[test]
fn app_construction_never_attempts_a_daemon_route_connect() {
    // #222 acceptance criterion: "No connection attempt happens before
    // the first play/enqueue action that needs one." There is no
    // production call site wiring `try_daemon_route_connect` into
    // startup yet (that wiring is #223's job) -- this test pins the
    // invariant down as a regression guard so a future startup-time
    // call is caught immediately instead of silently reintroducing the
    // eager-connect behavior #222 replaces.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    fn counting_connect(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0))
    }

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(counting_connect);
    let _app = make_app_stub();
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn apply_route_for_playback_swaps_to_routed_daemon_on_success() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
}

#[test]
fn apply_route_for_playback_falls_back_to_local_with_warning_on_connect_failure() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert!(app.status.contains("unreachable"));
}

#[test]
fn apply_route_for_playback_is_noop_when_item_already_matches_active_route() {
    // #256: resolution is now a pure config read -- no live session
    // lookup, no SESSIONS_LOAD_OVERRIDE seam needed to reach the no-op
    // branch (`name == current`), even though this test's whole point
    // is that no *connect* attempt happens.
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.active_route = Some("music".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.apply_route_for_playback(&item);

    // No connect attempt was needed (no DAEMON_ROUTE_CONNECT_OVERRIDE
    // set, so a real connect attempt would panic/hang if this weren't
    // a no-op) -- active_route and local-ness are unchanged.
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(!app.player.is_remote());
}

#[test]
fn apply_route_for_playback_restores_local_when_item_has_no_route() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx);
    let mut movies_item = make_item("Movies", "CollectionFolder");
    movies_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: movies_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();

    app.apply_route_for_playback(&item);

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn apply_route_for_playback_restores_local_via_restore_local_mode_when_swap_to_a_different_route_fails(
) {
    // Regression guard for the `Err` branch's `was_routed.is_some()`
    // arm: already on a different route ("music"), an item resolving
    // to a *new* route ("movies") whose connect attempt fails must be
    // torn down through `restore_local_mode` -- not just flashed a
    // warning while silently staying attached to the stale "music"
    // remote player.
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_failure(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Err("connection refused".to_string())
    }

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    app.library_routes
        .insert("movies".to_string(), "tcp://127.0.0.1:9001".to_string());
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx);
    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());

    let mut lib_item = make_item("Movies", "CollectionFolder");
    lib_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Movie", "Movie");
    item.id = "movie-1".to_string();
    app.library_tab = 1;

    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_failure);
    app.apply_route_for_playback(&item);
    *DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert!(app.status.contains("unreachable"));
}

#[test]
fn remote_position_extrapolation_does_not_round_up_partial_seconds() {
    assert_eq!(
        App::extrapolated_remote_position(10, Duration::from_millis(1600)),
        11
    );
    assert_eq!(
        App::extrapolated_remote_position(10, Duration::from_secs(2)),
        12
    );
}

#[test]
fn feed_home_video_group_view_requires_homevideos_and_feed_config() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    assert!(!app.is_feed_home_video_group_view(0));

    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];
    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn feed_home_video_group_view_stays_enabled_with_cached_groups() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![
            BrowseLevel {
                parent_id: "lib-youtube".into(),
                title: "YouTube".into(),
                items: vec![folder.clone()],
                total_count: 1,
                cursor: 1,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
                letter_filter: None,
            },
            BrowseLevel {
                parent_id: "folder-a".into(),
                title: "Channel A".into(),
                items: vec![video.clone()],
                total_count: 1,
                cursor: 0,
                item_types: Some("Video".into()),
                unplayed_only: true,
                sort_by: "DateCreated".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: Some(vec![video.clone()]),
                letter_filter: None,
            },
        ],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];
    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn fetch_home_preserves_feed_home_video_state() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library: library.clone(),
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video.clone()],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.rebuild_library_tabs_from_views(&[library]);

    assert_eq!(app.libs.len(), 1);
    assert!(app.is_feed_home_video_group_view(0));
    let feed = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(feed.groups.len(), 1);
    assert_eq!(feed.groups[0].items.len(), 1);
    assert_eq!(feed.groups[0].items[0].id, "video-a1");
}

#[test]
fn feed_home_video_root_does_not_auto_push_before_folder_pagination_completes() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    let mut folders = Vec::new();
    for idx in 0..100 {
        let mut folder = make_item(&format!("Channel {idx}"), "Folder");
        folder.id = format!("folder-{idx}");
        folder.is_folder = true;
        folders.push(folder);
    }

    app.handle_lib_event(LibEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: folders,
            total_count: 101,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        },
    });

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.libs[0].nav_stack[0].items.len(), 100);
    assert_eq!(app.libs[0].nav_stack[0].total_count, 101);
    // Pagination must keep going even though the root cursor (0) is nowhere
    // near the loaded edge -- the feed-home-video root isn't scrolled by the
    // user, so it has to paginate to completion on its own or aggregation
    // (and therefore the grouped view) would never be able to start.
    assert!(
        app.libs[0].nav_stack[0].loading,
        "expected the next folder page to be fetched automatically"
    );
}

#[test]
fn select_feed_folder_group_pushes_video_level_for_selected_folder() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    let mut first = make_item("Channel A", "Folder");
    first.id = "folder-a".into();
    first.is_folder = true;
    let mut second = make_item("Channel B", "Folder");
    second.id = "folder-b".into();
    second.is_folder = true;
    let mut second_video = make_item("B1", "Movie");
    second_video.id = "video-b1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![first.clone(), second.clone()],
            total_count: 2,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![second_video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder: second.clone(),
                items: vec![second_video.clone()],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.select_feed_folder_group(0, 1);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(1)
    );
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-b1");
}

#[test]
fn select_feed_folder_group_zero_pushes_all_videos_level() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video.clone()],
            }],
            loading: false,
            selected_group: 1,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.select_feed_folder_group(0, 0);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(0)
    );
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-a1");
}

#[test]
fn select_feed_folder_group_uses_client_side_all_items_cache() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    let mut first = make_item("Channel A", "Folder");
    first.id = "folder-a".into();
    first.is_folder = true;
    first.path = "/videos/a".into();
    let mut second = make_item("Channel B", "Folder");
    second.id = "folder-b".into();
    second.is_folder = true;
    second.path = "/videos/b".into();

    let mut a_video = make_item("A1", "Movie");
    a_video.id = "video-a1".into();
    a_video.path = "/videos/a/one.mp4".into();
    let mut b_video = make_item("B1", "Movie");
    b_video.id = "video-b1".into();
    b_video.path = "/videos/b/one.mp4".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![first.clone(), second.clone()],
            total_count: 2,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![a_video.clone(), b_video.clone()],
            groups: vec![
                FeedHomeVideoGroup {
                    folder: first.clone(),
                    items: vec![a_video.clone()],
                },
                FeedHomeVideoGroup {
                    folder: second.clone(),
                    items: vec![b_video.clone()],
                },
            ],
            loading: false,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.select_feed_folder_group(0, 2);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(2)
    );
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-b1");

    app.go_back();
    app.select_feed_folder_group(0, 0);
    assert_eq!(app.feed_home_video_selected_items(0).len(), 2);
}

#[test]
fn select_feed_folder_group_updates_feed_state_when_detail_level_exists() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    let mut first = make_item("Channel A", "Folder");
    first.id = "folder-a".into();
    first.is_folder = true;
    let mut second = make_item("Channel B", "Folder");
    second.id = "folder-b".into();
    second.is_folder = true;

    let mut a_video = make_item("A1", "Movie");
    a_video.id = "video-a1".into();
    let mut b_video = make_item("B1", "Movie");
    b_video.id = "video-b1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![first.clone(), second.clone()],
            total_count: 2,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![a_video.clone(), b_video.clone()],
            groups: vec![
                FeedHomeVideoGroup {
                    folder: first,
                    items: vec![a_video],
                },
                FeedHomeVideoGroup {
                    folder: second,
                    items: vec![b_video.clone()],
                },
            ],
            loading: false,
            selected_group: 1,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.select_feed_folder_group(0, 2);
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(2)
    );
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-b1");
}

#[test]
fn go_back_keeps_feed_home_video_group_view_intact() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video],
            }],
            loading: false,
            selected_group: 1,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.go_back();
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(1)
    );
}

#[test]
fn feed_home_video_root_filters_groups_from_all_video_paths() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    let mut empty = make_item("Empty Channel", "Folder");
    empty.id = "folder-empty".into();
    empty.is_folder = true;
    empty.path = "/videos/empty".into();

    let mut active = make_item("Active Channel", "Folder");
    active.id = "folder-active".into();
    active.is_folder = true;
    active.path = "/videos/active".into();

    app.handle_lib_event(LibEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![empty, active.clone()],
            total_count: 2,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        },
    });

    assert_eq!(app.libs[0].nav_stack.len(), 1);

    let mut video = make_item("Episode 1", "Movie");
    video.path = "/videos/active/ep1.mp4".into();

    app.handle_lib_event(LibEvent::FeedHomeVideoAggregated {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        all_items: vec![video.clone()],
        groups: vec![FeedHomeVideoGroup {
            folder: active.clone(),
            items: vec![video],
        }],
    });

    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.groups.len()),
        Some(1)
    );
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .and_then(|state| state.groups.first())
            .map(|group| group.folder.id.as_str()),
        Some("folder-active")
    );
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.all_items.len()),
        Some(1)
    );
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    app.ensure_feed_home_video_group_level(0);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(
        app.feed_home_video_selected_items(0)[0].path,
        "/videos/active/ep1.mp4"
    );
}

#[test]
fn ensure_feed_home_video_group_level_clamps_stale_cursor_to_available_groups() {
    // A stale selected group from a prior aggregation run with more groups
    // must clamp to the groups that actually exist now.
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let video = make_item("A1", "Movie");

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video],
            }],
            loading: false,
            selected_group: 5,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.ensure_feed_home_video_group_level(0);

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(1)
    );
}

#[test]
fn refresh_lib_targets_power_feed_selection() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.panel_focus = PanelFocus::Library;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video],
            }],
            loading: false,
            selected_group: 1,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.refresh_lib();

    assert!(app.libs[0].nav_stack[0].loading);
    assert!(app.libs[0]
        .feed_home_video
        .as_ref()
        .map(|state| state.loading)
        .unwrap_or(false));
}

#[test]
fn podcast_library_detects_collection_type() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
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

    assert!(app.is_podcast_library(0));
}

#[test]
fn podcast_library_detects_name_when_collection_type_missing() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
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

    assert!(app.is_podcast_library(0));
}

#[test]
fn podcast_folder_context_menu_uses_play_labels_and_item_state() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    let mut show = make_item("Show A", "Folder");
    show.id = "show-a".into();
    show.is_folder = true;
    show.unplayed_item_count = 0;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-podcasts".into(),
            title: "Podcasts".into(),
            items: vec![show],
            total_count: 1,
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
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.library_tab = 1;

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert!(labels.contains(&"Mark Unplayed"));
    assert!(!labels.contains(&"Mark Played"));
    assert!(!labels.contains(&"Mark Watched"));
    assert!(!labels.contains(&"Mark Unwatched"));
}

#[test]
fn podcast_folder_context_menu_shows_mark_played_when_unplayed_items_remain() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    let mut show = make_item("Show A", "Folder");
    show.id = "show-a".into();
    show.is_folder = true;
    show.unplayed_item_count = 3;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-podcasts".into(),
            title: "Podcasts".into(),
            items: vec![show],
            total_count: 1,
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
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.library_tab = 1;

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert!(labels.contains(&"Mark Played"));
    assert!(!labels.contains(&"Mark Unplayed"));
}

#[test]
fn power_view_podcast_context_menu_uses_left_pane_library_context() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    let mut show = make_item("Show A", "Folder");
    show.id = "show-a".into();
    show.is_folder = true;
    show.unplayed_item_count = 0;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-podcasts".into(),
            title: "Podcasts".into(),
            items: vec![show],
            total_count: 1,
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

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert!(labels.contains(&"Mark Unplayed"));
    assert!(!labels.contains(&"Mark Watched"));
    assert!(!labels.contains(&"Mark Unwatched"));
}

#[test]
fn power_view_podcast_context_menu_offers_mark_all_played_for_selected_show() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    let mut show = make_item("Show A", "Folder");
    show.id = "show-a".into();
    show.is_folder = true;

    let mut first = make_item("Episode 1", "Audio");
    first.id = "ep-1".into();
    first.media_type = "Audio".into();
    let mut second = make_item("Episode 2", "Audio");
    second.id = "ep-2".into();
    second.media_type = "Audio".into();
    second.played = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-podcasts".into(),
            title: "Podcasts".into(),
            items: vec![show.clone()],
            total_count: 1,
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
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![first.clone(), second.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder: show,
                items: vec![first.clone(), second],
            }],
            loading: false,
            selected_group: 1,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert!(labels.contains(&"────────"));
    assert!(labels.contains(&"Mark All Played"));
    assert!(labels.contains(&"Mark All Unplayed"));
    let sep_idx = labels
        .iter()
        .position(|label| *label == "────────")
        .unwrap();
    let all_played_idx = labels
        .iter()
        .position(|label| *label == "Mark All Played")
        .unwrap();
    let all_unplayed_idx = labels
        .iter()
        .position(|label| *label == "Mark All Unplayed")
        .unwrap();
    assert!(sep_idx < all_played_idx);
    assert!(all_played_idx < all_unplayed_idx);
    assert_eq!(sep_idx, labels.len() - 3);
    assert_eq!(all_played_idx, labels.len() - 2);
    assert_eq!(all_unplayed_idx, labels.len() - 1);
    assert!(menu.entries.iter().any(|entry| {
        matches!(
            entry.action.as_ref(),
            Some(ContextAction::MarkItemsPlayed(ids)) if ids == &vec!["ep-1".to_string()]
        )
    }));
    assert!(menu.entries.iter().any(|entry| {
        matches!(
            entry.action.as_ref(),
            Some(ContextAction::MarkItemsUnplayed(ids)) if ids == &vec!["ep-2".to_string()]
        )
    }));
}

#[test]
fn power_view_podcast_context_menu_mark_all_played_uses_all_pill_selection() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    let mut first_show = make_item("Show A", "Folder");
    first_show.id = "show-a".into();
    first_show.is_folder = true;
    let mut second_show = make_item("Show B", "Folder");
    second_show.id = "show-b".into();
    second_show.is_folder = true;

    let mut first = make_item("Episode 1", "Audio");
    first.id = "ep-1".into();
    first.media_type = "Audio".into();
    let mut second = make_item("Episode 2", "Audio");
    second.id = "ep-2".into();
    second.media_type = "Audio".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-podcasts".into(),
            title: "Podcasts".into(),
            items: vec![first_show.clone(), second_show.clone()],
            total_count: 2,
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
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![first.clone(), second.clone()],
            groups: vec![
                FeedHomeVideoGroup {
                    folder: first_show,
                    items: vec![first.clone()],
                },
                FeedHomeVideoGroup {
                    folder: second_show,
                    items: vec![second.clone()],
                },
            ],
            loading: false,
            selected_group: 0,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert_eq!(labels[labels.len() - 3], "────────");
    assert_eq!(labels[labels.len() - 2], "Mark All Played");
    assert_eq!(labels[labels.len() - 1], "Mark All Unplayed");
    assert!(menu.entries.iter().any(|entry| {
        matches!(
            entry.action.as_ref(),
            Some(ContextAction::MarkItemsPlayed(ids))
                if ids == &vec!["ep-1".to_string(), "ep-2".to_string()]
        )
    }));
    assert!(menu.entries.iter().any(|entry| {
        matches!(
            entry.action.as_ref(),
            Some(ContextAction::MarkItemsUnplayed(ids)) if ids.is_empty()
        )
    }));
}

#[test]
fn refreshed_does_not_overwrite_feed_root_with_video_items() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
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
        }],
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            all_items: vec![video.clone()],
            groups: vec![FeedHomeVideoGroup {
                folder,
                items: vec![video.clone()],
            }],
            loading: false,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.handle_lib_event(LibEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: Some("Video".into()),
        unplayed_only: true,
        items: vec![video],
        total_count: 1,
    });

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.libs[0].nav_stack[0].item_types, None);
    assert_eq!(app.libs[0].nav_stack[0].items.len(), 1);
    assert!(app.libs[0].nav_stack[0].items[0].is_folder);
    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn refreshed_restores_feed_loading_state_when_feed_state_is_missing() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut folder = make_item("Channel A", "Folder");
    folder.id = "folder-a".into();
    folder.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder],
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
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.handle_lib_event(LibEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: Some("Video".into()),
        unplayed_only: true,
        items: vec![video],
        total_count: 1,
    });

    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.libs[0].nav_stack[0].item_types, None);
    assert!(app.libs[0].feed_home_video.as_ref().unwrap().loading);
    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn stale_remote_queue_scope_falls_back_to_local_when_not_in_direct_remote_mode() {
    let mut app = make_app_stub();
    app.remote_player_tab = Some(PlayerTab::new(make_items(2), 1));
    app.queue_scope = QueueScope::Remote;

    assert_eq!(app.visible_queue_scope(), QueueScope::Local);

    app.set_queue_scope(QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert_eq!(app.queue_scope, QueueScope::Local);
}

#[test]
fn queue_scope_resolution_matrix_without_remote_queue() {
    let mut app = make_app_stub();
    app.queue_scope = QueueScope::Local;

    assert!(!app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_stale_remote_scope_without_direct_remote() {
    let mut app = make_app_stub();
    app.remote_player_tab = Some(PlayerTab::new(make_items(2), 0));
    app.queue_scope = QueueScope::Remote;

    assert!(!app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_direct_remote_displaying_local() {
    let local_items = make_items(1);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items, remote_items);
    app.queue_scope = QueueScope::Local;

    assert!(app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_direct_remote_displaying_remote() {
    let local_items = make_items(1);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items, remote_items);
    app.queue_scope = QueueScope::Remote;

    assert!(app.has_direct_remote_queue());
    assert_eq!(app.playback_target_queue_scope(), QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn power_queue_renders_scope_pills_and_hitboxes_for_direct_remote() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Library;
    app.set_queue_scope(QueueScope::Local);

    let rendered = render_app_to_string(&mut app, 90, 28);
    let device_name = device_name();
    let upper_device_name = device_name.to_uppercase();

    assert!(
        rendered.contains(&format!(" {} ", upper_device_name)),
        "expected power queue local/session pills to use the device name:\n{rendered}"
    );
    assert!(app.layout.main.queue_scope_local_area.width >= device_name.width() as u16);
    assert!(app.layout.main.queue_scope_remote_area.width >= device_name.width() as u16);
    assert!(app.layout.main.queue_scope_remote_area.x > app.layout.main.queue_scope_local_area.x);
}

#[test]
fn power_queue_scope_switch_via_keyboard_works_from_queue_focus() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Local);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_left_focus_brackets_do_not_switch_queue_scope() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Library;
    app.set_queue_scope(QueueScope::Local);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_queue_scope_switch_via_click_uses_rendered_hitboxes() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Library;
    app.set_queue_scope(QueueScope::Local);
    let _ = render_app_to_string(&mut app, 90, 28);

    let remote = app.layout.main.queue_scope_remote_area;
    app.handle_mouse(left_down(remote.x, remote.y));
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);

    let local = app.layout.main.queue_scope_local_area;
    app.handle_mouse(left_down(local.x, local.y));
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_scope_keys_are_ignored_outside_queue_tab() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.set_queue_scope(QueueScope::Local);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn power_view_shift_resize_grows_from_queue_focus_and_persists_pref() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.panel_focus = PanelFocus::Queue;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

    assert!(!handled);
    assert_eq!(app.status, "Power view width: 45 cols");
    let prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(prefs["queue_column_width"].as_u64(), Some(45));
}

#[test]
fn power_view_shift_resize_is_blocked_by_help_overlay() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.show_help = true;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

    assert!(!handled);
    assert!(app.show_help);
    assert!(app.status.is_empty());
    assert!(!crate::config::prefs_path().exists());
}

#[test]
fn power_view_shift_resize_clamps_and_reports_minimum_and_maximum() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));

    let handled = app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));
    assert!(!handled);
    assert_eq!(app.status, "Power view width already at minimum (40 cols)");
    assert!(!crate::config::prefs_path().exists());

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(app.status, "Power view width: 45 cols");

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(app.status, "Power view width: 48 cols");

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(app.status, "Power view width already at maximum (48 cols)");

    let prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(prefs["queue_column_width"].as_u64(), Some(48));
}

#[test]
fn power_view_render_normalizes_saved_left_width_and_updates_layout() {
    let _guard = crate::config::TestStateDirGuard::new();
    let prefs = serde_json::json!({
        "queue_column_width": 70,
    });
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::to_string(&prefs).expect("prefs json"),
    )
    .expect("write prefs");

    let mut app = make_remote_app_stub(make_items(1), make_items(2));

    let _ = render_app_to_string(&mut app, 70, 28);

    assert_eq!(app.layout.main.queue_area.width, 38);
    let saved: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(saved["queue_column_width"].as_u64(), Some(42));
}

#[test]
fn power_view_render_uses_resized_width_on_next_frame() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));

    let _ = render_app_to_string(&mut app, 100, 28);
    assert_eq!(app.layout.main.queue_area.width, 36);

    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));
    assert!(!app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)));

    let _ = render_app_to_string(&mut app, 100, 28);
    assert_eq!(app.layout.main.queue_area.width, 46);
}

#[test]
fn local_daemon_queue_has_no_scope_affordance_or_remote_switch() {
    let mut app = make_local_daemon_app_stub(make_items(2));
    let rendered = render_app_to_string(&mut app, 90, 24);

    assert!(!app.has_direct_remote_queue());
    assert_eq!(
        app.layout.main.queue_scope_local_area,
        ratatui::layout::Rect::default()
    );
    assert_eq!(
        app.layout.main.queue_scope_remote_area,
        ratatui::layout::Rect::default()
    );
    assert!(
        !rendered.contains(" Local ") && !rendered.contains(" Remote "),
        "local-daemon queue should not render split-scope pills:\n{rendered}"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn attached_session_only_queue_has_no_scope_affordance_or_remote_switch() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    let rendered = render_app_to_string(&mut app, 90, 24);

    assert!(!app.has_direct_remote_queue());
    assert_eq!(
        app.layout.main.queue_scope_local_area,
        ratatui::layout::Rect::default()
    );
    assert_eq!(
        app.layout.main.queue_scope_remote_area,
        ratatui::layout::Rect::default()
    );
    assert!(
        !rendered.contains(" Local ") && !rendered.contains(" Remote "),
        "attached-session queue should not render split-scope pills:\n{rendered}"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert!(!handled);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn status_bar_row_is_always_present_and_holds_status_labels() {
    let mut app = make_app_stub();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("\u{1F5AD}  none"),
        "expected the playlist status on the final screen row:\n{rendered}"
    );
    // The status labels must not render inside the tab row (first line).
    let first_line = rendered.lines().next().unwrap();
    assert!(
        !first_line.contains('\u{1F5AD}'),
        "status labels must stay off the tab row:\n{first_line}"
    );
    // TABBAR_LEFT_RESERVE is 0 and the first tab has no left gutter, so
    // the tab row is left-aligned flush with the left edge -- the pill
    // that used to live here now renders in the status bar.
    let first_non_space = first_line.find(|c: char| c != ' ').unwrap_or(0);
    assert_eq!(
            first_non_space, 0,
            "expected the tab row's first tab to start flush at the left edge (col 0), got col {first_non_space}:\n{first_line}"
        );
}

#[test]
fn direct_remote_play_items_keeps_local_queue_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let replacement = make_items(4);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    app.queue_source = crate::config::QueueSource::Album;

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: replacement.clone(),
        start_idx: 2,
        source: crate::config::QueueSource::Shuffle,
    });

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        replacement
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Album
    ));
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
}

#[test]
fn direct_remote_track_changes_do_not_clobber_local_last_played() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    app.last_played_item_id = Some(local_items[1].id.clone());
    app.last_played_completed = true;

    app.handle_player_event(PlayerEvent::TrackChanged(2));

    assert_eq!(
        app.last_played_item_id.as_deref(),
        Some(local_items[1].id.as_str())
    );
    assert!(app.last_played_completed);
}

#[test]
fn command_rejected_flashes_the_daemon_supplied_reason() {
    let mut app = make_app_stub();

    app.handle_player_event(PlayerEvent::CommandRejected(
        "Daemon is running in audio-only mode; can't play video items".to_string(),
    ));

    assert_eq!(
        app.status,
        "Daemon is running in audio-only mode; can't play video items"
    );
}

#[test]
fn stopped_progress_updates_the_queue_model_not_just_the_shadow() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_id = app.player_tab.queue.slots()[0].slot_id;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    let slot = app.player_tab.queue.slot(slot_id).unwrap();
    assert_eq!(
        slot.item.playback_position_ticks, 600_000_000,
        "progress must be applied to the queue model, not only the display shadow"
    );
}

#[test]
fn stopped_with_accepted_report_marks_pending_sync_and_clears_active_slot() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(1);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_id = app.player_tab.queue.slots()[0].slot_id;
    app.handle_player_event(PlayerEvent::TrackChanged(0));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
    }

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: true,
        error: None,
    });

    let slot = app.player_tab.queue.slot(slot_id).unwrap();
    assert_eq!(
        slot.progress_state
            .pending_sync
            .as_ref()
            .map(|progress| progress.position_ticks),
        Some(600_000_000)
    );
    assert_eq!(app.player_tab.queue.active_slot_id(), None);
}

#[test]
fn stopped_consume_removes_the_right_slot_occurrence() {
    // Duplicate item ids: two occurrences of the same underlying item.
    // Stopping+consuming the second occurrence must remove that slot
    // specifically — never the first, which happens to share an id.
    let mut app = make_app_stub();
    let mut items = make_items(3);
    items[0].id = "dup".into();
    items[2].id = "dup".into();
    app.player_tab.items = items;
    app.player_tab.sync_queue_model_from_items_if_needed();
    let first_dup = app.player_tab.queue.slots()[0].slot_id;
    let second_dup = app.player_tab.queue.slots()[2].slot_id;
    app.client.lock().unwrap().config.consume_videos = true;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 2,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert!(app.player_tab.queue.slot(first_dup).is_some());
    assert!(app.player_tab.queue.slot(second_dup).is_none());
}

#[test]
fn stopped_delete_removes_the_active_now_playing_slot() {
    // The confirmed "remove now-playing item and stop playback" flow:
    // pending_delete_idx marks the active slot for removal, then a Stopped
    // event drives it. Now that TrackChanged populates the model's
    // active_slot_id in real playback, the gated remove_slot path would
    // refuse the active slot — the confirmed delete must bypass that gate.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    // TrackChanged(0) activates slot 0, mirroring real playback where the
    // model's active_slot_id becomes Some before the delete.
    app.handle_player_event(PlayerEvent::TrackChanged(0));
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    app.pending_delete_idx = Some(0);

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "the confirmed delete must remove the active now-playing slot"
    );
    assert_eq!(
        app.queue_undo_stack.len(),
        1,
        "delete must push an undo entry"
    );
}

#[test]
fn stopped_path_consumes_the_last_audio_item_in_the_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    // When the last item in the queue finishes, the player thread sends a
    // Stopped event (not TrackCompleted/TrackChanged) since there's no next
    // track to advance to. consume_audio must still remove it, mirroring how
    // consume_videos already works for a video's Stopped-path removal.
    let items = make_audio_items(1);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_audio = true;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert!(
        app.player_tab.items.is_empty(),
        "the last audio item should be consumed via the Stopped-path when consume_audio is on"
    );
}

#[test]
fn stopped_path_does_not_consume_audio_when_consume_audio_is_off() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(1);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_audio = false;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consume_audio is off, so the item must stay in the queue"
    );
}

#[test]
fn track_completed_progress_follows_slot_after_earlier_removal() {
    // queue: [a, b, c]; a is removed (indices shift: b now at 0, c at 1),
    // then a completion event for the player's post-removal index of b
    // (0) arrives. Progress must land on slot b regardless of the churn.
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;
    let slot_a = app.player_tab.queue.slots()[0].slot_id;
    assert!(matches!(
        app.player_tab.queue.remove_slot(slot_a),
        RemoveSlotResult::Removed(_)
    ));
    app.player_tab.sync_items_from_queue_model();

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: false,
    });

    let slot = app.player_tab.queue.slot(slot_b).unwrap();
    assert_eq!(slot.item.playback_position_ticks, 600_000_000);
}

#[test]
fn track_completed_for_removed_slot_does_not_mutate_queue() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let ids_before: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|s| s.slot_id)
        .collect();

    // index 5 does not exist
    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 5,
        position_ticks: 600_000_000,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    let ids_after: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|s| s.slot_id)
        .collect();
    assert_eq!(ids_before, ids_after);
    assert!(app.pending_queue_removal.is_none());
}

#[test]
fn track_changed_activates_the_current_slot() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;

    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.queue.active_slot_id(),
        Some(slot_b),
        "TrackChanged must set the model's active slot by identity, not just move the raw cursor"
    );
}

#[test]
fn track_changed_activates_slot_and_consumes_deferred_slot() {
    // [a, b, c]; complete+consume a (deferred), then TrackChanged to b.
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;
    app.client.lock().unwrap().config.consume_videos = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    assert!(app.pending_queue_removal.is_some());

    app.handle_player_event(PlayerEvent::TrackChanged(1)); // player reports b at old idx 1

    // a was consumed; queue is [b, c]; b is active.
    assert_eq!(app.player_tab.queue.slots().len(), 2);
    assert_eq!(app.player_tab.queue.active_slot_id(), Some(slot_b));
    assert!(app.pending_queue_removal.is_none());
}

#[test]
fn consuming_a_video_without_autosave_marks_queue_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.save_playlist_on_consume = false;

    // First item finishes playing and is consumed while advancing to the next track.
    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed item should be removed from the local queue"
    );
    assert!(
        app.queue_dirty,
        "consuming an item changes the saved playlist's contents; without \
             save_playlist_on_consume the queue must be marked dirty so the user is still \
             prompted to save before quitting/replacing the queue"
    );
}

#[test]
fn consuming_a_video_resyncs_the_players_own_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    // The player thread (QueueSession) keeps its own separate copy of the
    // items list, independent of `player_tab.items`. If consume only shrinks
    // the app-side queue and never tells the player, the player's internal
    // index space permanently diverges from the displayed queue after the
    // first consume — any later index-based command (Enter on a queue row,
    // JumpTo, next natural advance) then lands on the wrong item.
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_videos = true;
    let cmd_rx = app.player.spy_on_commands();

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(crate::player::PlayerCommand::QueueRemove(0))
        ),
        "consuming idx=0 must tell the player to remove idx=0 from its own \
             internal queue, keeping it in sync with the app-side queue"
    );
}

#[test]
fn consuming_a_video_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed item should be removed from the local queue"
    );
    assert!(
        !app.queue_dirty,
        "with save_playlist_on_consume enabled, consuming from a saved playlist should \
             trigger an immediate re-save to Emby (mirroring the manual save-playlist flow), \
             so the queue is no longer considered dirty"
    );
}

#[test]
fn consuming_a_video_on_direct_remote_queue_does_not_touch_local_queue_or_dirty_flag() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    // The local queue happens to be a saved playlist with autosave-on-consume enabled —
    // the trap scenario: before the scope gate, consuming on the *remote* queue would
    // still fire save_playlist_to_emby() and push the unrelated, unmodified local
    // playlist to Emby.
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().items.len(),
        1,
        "consumed item should still be removed from the remote queue"
    );
    assert_eq!(
        app.player_tab.items.len(),
        local_items.len(),
        "consume on a direct-remote queue must not touch the unrelated local playlist"
    );
    assert!(
        !app.queue_dirty,
        "consume on a direct-remote queue must not mark the local queue dirty or trigger \
             an autosave of the local playlist — the change happened on the remote queue"
    );
}

#[test]
fn consuming_an_audio_item_without_autosave_marks_queue_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_audio = true;
    app.client
        .lock()
        .unwrap()
        .config
        .save_playlist_on_consume_audio = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed audio item should be removed from the local queue"
    );
    assert!(
        app.queue_dirty,
        "consuming an audio item changes the saved playlist's contents; without \
             save_playlist_on_consume_audio the queue must be marked dirty so the user is \
             still prompted to save before quitting/replacing the queue"
    );
}

#[test]
fn consuming_an_audio_item_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_audio = true;
    app.client
        .lock()
        .unwrap()
        .config
        .save_playlist_on_consume_audio = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed audio item should be removed from the local queue"
    );
    assert!(
        !app.queue_dirty,
        "with save_playlist_on_consume_audio enabled, consuming from a saved playlist \
             should trigger an immediate re-save to Emby, so the queue is no longer dirty"
    );
}

#[test]
fn consume_videos_flag_does_not_consume_audio_items() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.consume_audio = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "consume_videos must not remove an audio item; consume_audio is off"
    );
}

#[test]
fn consume_audio_flag_does_not_consume_video_items() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_audio = true;
    app.client.lock().unwrap().config.consume_videos = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "consume_audio must not remove a video item; consume_videos is off"
    );
}

#[test]
fn ctrl_a_enqueues_from_home_view() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert!(!handled);
    assert_eq!(app.player_tab.items.len(), 1);
    assert_eq!(app.player_tab.items[0].id, "id0");
}

#[test]
fn ctrl_a_appends_to_direct_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(local_items, remote_items.clone());
    app.queue_scope = QueueScope::Remote;
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert!(!handled);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .chain(std::iter::once("id0"))
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        cmd_rx.try_recv(),
        Ok(mbv_core::ctrl::CtrlCmd::PlayerCmd(
            mbv_core::ctrl::WireCommand::QueueAppend { items }
        )) if items.len() == 1 && items[0].id == "id0"
    ));
    assert!(
        cmd_rx.try_recv().is_err(),
        "Ctrl+A append must not follow QueueAppend with ReplaceQueue"
    );
}

#[test]
fn ctrl_a_rejects_v2_direct_remote_append_without_replace_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_v2_remote_app_stub_with_cmd_rx(local_items, remote_items);
    app.queue_scope = QueueScope::Remote;
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert!(!handled);
    assert!(
        cmd_rx.try_recv().is_err(),
        "v2 direct remote append must not fall back to ReplaceQueue"
    );
    assert_eq!(
        app.status,
        "Remote append is not supported by this direct mbv peer"
    );
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["id0", "id1", "id2"]
    );
}

#[test]
fn rejected_v2_direct_remote_append_preserves_remote_undo_slot_identity() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, _cmd_rx) = make_v2_remote_app_stub_with_cmd_rx(local_items, remote_items);
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_up();
    let moved_slot = app
        .remote_player_tab
        .as_ref()
        .unwrap()
        .resolve_slot_at(0)
        .expect("moved slot should be at destination");

    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));

    assert!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .slot_id_matches_at(0, moved_slot),
        "rejected append rollback must preserve existing remote queue slot IDs"
    );

    app.undo_last_queue_edit(QueueScope::Remote);

    assert_ne!(app.status, "Can't undo move: queue changed since then");
    assert!(app.remote_queue_undo_stack.is_empty());
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["id0", "id1", "id2"]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
}

#[test]
fn clearing_local_queue_in_direct_remote_mode_leaves_remote_queue_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items, remote_items.clone());
    app.set_queue_scope(QueueScope::Local);
    app.queue_source = crate::config::QueueSource::Album;
    app.queue_dirty = true;

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.player_tab.items.is_empty());
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Unknown
    ));
    assert!(!app.queue_dirty);
}

#[test]
fn clearing_remote_queue_in_direct_remote_mode_leaves_local_queue_metadata_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { .. }
    ));
    assert!(app.queue_dirty);
}

#[test]
fn removing_from_local_queue_in_direct_remote_mode_does_not_touch_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Local);

    app.remove_from_queue(1);

    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![local_items[0].id.as_str(), local_items[2].id.as_str()]
    );
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(app.queue_dirty);
    assert_eq!(app.remote_queue_undo_stack.len(), 0);
}

#[test]
fn removing_from_remote_queue_in_direct_remote_mode_does_not_touch_local_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());

    app.remove_from_queue(1);

    assert_eq!(app.remote_player_tab.as_ref().unwrap().items.len(), 2);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![remote_items[0].id.as_str(), remote_items[2].id.as_str()]
    );
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(!app.queue_dirty);
    assert_eq!(app.queue_undo_stack.len(), 0);
    assert_eq!(app.remote_queue_undo_stack.len(), 1);
}

#[test]
fn clearing_remote_queue_does_not_prompt_to_save_local_playlist() {
    let mut app = make_remote_app_stub(make_items(2), make_items(3));
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;

    app.replace_queue_or_prompt(PendingQueueAction::ClearQueue);

    assert!(!app.show_save_playlist_modal);
    assert!(app.pending_queue_action.is_none());
    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
    assert!(app.queue_dirty);
}

#[test]
fn removing_from_inactive_remote_queue_is_rejected() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items, remote_items.clone());
    app.player.status.lock().unwrap().active = false;

    app.remove_from_queue(1);

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.status, "Remote queue can only be edited while active");
}

#[test]
fn context_menu_remove_targets_displayed_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

    app.open_context_menu();

    let action = app
        .context_menu
        .as_ref()
        .expect("context menu")
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("remove from queue action");
    assert_eq!(action, 2);

    app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)));

    let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(item_ids(&app.player_tab.items), item_ids(&local_items));
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().items),
        vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
    );
    assert_eq!(app.remote_queue_undo_stack.len(), 1);
}

#[test]
fn stale_context_menu_remove_remote_queue_index_is_ignored() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

    app.open_context_menu();

    let action = app
        .context_menu
        .as_ref()
        .expect("context menu")
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("remove from queue action");
    app.remote_player_tab.as_mut().unwrap().items.truncate(2);

    app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)));

    let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(item_ids(&app.player_tab.items), item_ids(&local_items));
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().items),
        vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert!(app.remote_queue_undo_stack.is_empty());
}

#[test]
fn move_queue_item_up_swaps_items_and_cursor_follows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;

    app.move_queue_item_up();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            items[1].id.as_str(),
            items[0].id.as_str(),
            items[2].id.as_str()
        ]
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(app.queue_undo_stack.len(), 1);
}

#[test]
fn move_queue_item_down_swaps_items_and_cursor_follows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;

    app.move_queue_item_down();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            items[0].id.as_str(),
            items[2].id.as_str(),
            items[1].id.as_str()
        ]
    );
    assert_eq!(app.player_tab.queue_cursor, 2);
    assert_eq!(app.queue_undo_stack.len(), 1);
}

#[test]
fn move_queue_item_up_is_noop_at_start_of_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_up();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert!(app.queue_undo_stack.is_empty());
}

#[test]
fn move_queue_item_down_is_noop_at_end_of_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 2;

    app.move_queue_item_down();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 2);
    assert!(app.queue_undo_stack.is_empty());
}

#[test]
fn undo_reverses_a_move_and_cursor_follows_back() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;

    app.move_queue_item_up();
    assert_eq!(app.player_tab.queue_cursor, 0);

    app.undo_last_queue_edit(QueueScope::Local);

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 1);
    assert!(app.queue_undo_stack.is_empty());
}

#[test]
fn undo_of_move_does_not_disturb_prior_removal_undo_history() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    // A removal, then a move -- undoing once should only reverse the move.
    app.remove_from_queue(0);
    app.player_tab.queue_cursor = 0;
    app.move_queue_item_down();
    assert_eq!(app.queue_undo_stack.len(), 2);

    app.undo_last_queue_edit(QueueScope::Local);

    assert_eq!(app.queue_undo_stack.len(), 1);
    assert!(matches!(
        app.queue_undo_stack.last(),
        Some(UndoEntry::Remove(0, _))
    ));
}

#[test]
fn undo_of_move_is_refused_if_the_moved_item_is_no_longer_at_to() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_down(); // items[0] now sits at index 1
    assert_eq!(app.queue_undo_stack.len(), 1);

    // Something untracked by this undo stack happens to the queue
    // afterwards (e.g. a natural consume) removing the item that's now
    // at index 1, so the undo entry's `to` position no longer holds the
    // item that was actually moved.
    app.player_tab.items.remove(1);

    app.undo_last_queue_edit(QueueScope::Local);

    // Refused rather than blindly swapping whatever now sits at 0/1.
    assert_eq!(app.status, "Can't undo move: queue changed since then");
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![items[1].id.as_str(), items[2].id.as_str()]
    );
}

#[test]
fn undo_of_move_is_refused_when_duplicate_id_masks_changed_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut items = make_items(3);
    items[0].id = "duplicate".into();
    items[0].name = "First duplicate".into();
    items[0].playlist_item_id = "slot-a".into();
    items[1].id = "duplicate".into();
    items[1].name = "Second duplicate".into();
    items[1].playlist_item_id = "slot-b".into();
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_down(); // First duplicate now sits at index 1.
    assert_eq!(app.queue_undo_stack.len(), 1);

    app.player_tab.items.remove(1);
    app.player_tab.items.insert(1, items[1].clone());

    app.undo_last_queue_edit(QueueScope::Local);

    assert_eq!(app.status, "Can't undo move: queue changed since then");
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Second duplicate", "Second duplicate", "Item 2"]
    );
}

#[test]
fn resolve_slot_at_maps_index_to_slot_and_rejects_out_of_range() {
    let tab = PlayerTab::new(make_items(3), 0);
    let s0 = tab.queue.slots()[0].slot_id;
    let s2 = tab.queue.slots()[2].slot_id;
    assert_eq!(tab.resolve_slot_at(0), Some(s0));
    assert_eq!(tab.resolve_slot_at(2), Some(s2));
    assert_eq!(tab.resolve_slot_at(3), None);
}

#[test]
fn queue_edit_preserves_updated_item_fields_after_shadow_model_was_built() {
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(2), 0);
    let _slot_id = app.player_tab.slot_id_at(0).unwrap();

    app.player_tab.items[0].playback_position_ticks = 42;
    app.player_tab.items[0].played = true;

    app.player_tab.append_item(make_item("new", "Movie"));

    assert_eq!(app.player_tab.items[0].playback_position_ticks, 42);
    assert!(app.player_tab.items[0].played);
}

#[test]
fn move_queue_item_for_remote_scope_sends_move_command_and_preserves_local_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) =
        make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_up();

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            remote_items[1].id.as_str(),
            remote_items[0].id.as_str(),
            remote_items[2].id.as_str()
        ]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(!app.queue_dirty);
    assert_eq!(app.queue_undo_stack.len(), 0);
    assert_eq!(app.remote_queue_undo_stack.len(), 1);
    assert!(matches!(
        cmd_rx.try_recv(),
        Ok(mbv_core::ctrl::CtrlCmd::PlayerCmd(
            mbv_core::ctrl::WireCommand::QueueMove(1, 0)
        ))
    ));
}

#[test]
fn move_queue_item_for_inactive_remote_scope_is_rejected() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(local_items, remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;
    app.player.status.lock().unwrap().active = false;

    app.move_queue_item_up();

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert_eq!(app.status, "Remote queue can only be edited while active");
    assert!(cmd_rx.try_recv().is_err());
}

#[test]
fn remote_queue_update_reconciles_remote_queue_without_touching_local_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    let updated_remote = vec![
        remote_items[2].clone(),
        remote_items[0].clone(),
        remote_items[1].clone(),
    ];

    app.handle_player_event(PlayerEvent::QueueUpdated {
        items: updated_remote.clone(),
        cursor: 2,
        source: crate::config::QueueSource::Remote,
    });

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        updated_remote
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn remote_queue_update_after_move_keeps_cursor_on_moved_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, _cmd_rx) =
        make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_up();

    app.handle_player_event(PlayerEvent::QueueUpdated {
        items: vec![
            remote_items[1].clone(),
            remote_items[0].clone(),
            remote_items[2].clone(),
        ],
        cursor: 1,
        source: crate::config::QueueSource::Remote,
    });

    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn remote_queue_update_after_move_tracks_duplicate_item_by_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let mut remote_items = make_items(3);
    remote_items[1].id = remote_items[0].id.clone();
    let (mut app, _cmd_rx) =
        make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_down();

    app.handle_player_event(PlayerEvent::QueueUpdated {
        items: vec![
            remote_items[0].clone(),
            remote_items[2].clone(),
            remote_items[1].clone(),
        ],
        cursor: 0,
        source: crate::config::QueueSource::Remote,
    });

    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn moving_now_playing_item_keeps_cursor_on_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    // `PlayerProxy::stub` (used by `make_app_stub`) has no live cmd channel to
    // assert against, so this only covers the app-side item/cursor bookkeeping;
    // `player::tests` covers the mpv-side PlaylistMove handling directly.
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }

    app.move_queue_item_down();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            items[0].id.as_str(),
            items[2].id.as_str(),
            items[1].id.as_str()
        ]
    );
    assert_eq!(app.player_tab.queue_cursor, 2);
}

#[test]
fn remote_slot_state_is_off_for_local_only_app() {
    let app = make_app_stub();
    assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
}

#[test]
fn app_stub_starts_with_no_active_library_route() {
    let app = make_app_stub();
    assert!(app.active_route.is_none());
    assert!(app.library_routes.is_empty());
    assert!(app.library_route_cache.is_empty());
}

#[test]
fn remote_slot_state_is_attached_session_when_connected_to_remote_session() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());

    assert_eq!(app.remote_slot_state(), RemoteSlotState::AttachedSession);
    assert!(app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [d]disc [r]refresh [Esc]close"
    );
}

#[test]
fn remote_slot_state_direct_remote_display_does_not_imply_sessions_panel_disconnect() {
    let app = make_remote_app_stub(make_items(2), make_items(3));

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
}

#[test]
fn direct_remote_connect_keeps_local_scope_when_remote_queue_is_empty() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx);

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn direct_remote_connect_switches_to_remote_scope_when_remote_queue_has_items() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items.clone(), 0);
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx);

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().items[0].id,
        remote_items[0].id
    );
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn switch_to_direct_remote_rebinds_mpris_to_the_new_remote_status() {
    // #175: before `switch_to_direct_remote` called `mpris::rebind`,
    // MPRIS stayed wired to whatever `PlayerStatus` was live when the
    // D-Bus service was first registered (almost always the initial
    // local `Player`'s), so local desktop MPRIS never picked up a
    // remote daemon's playback after a mid-session "Direct Remote"
    // takeover -- exactly the bug this issue reports. This drives the
    // real `App` method (not just `mpris::rebind` in isolation) to
    // prove the wiring at the call site is actually in place.
    let mut app = make_app_stub();
    let local_status = app.player.status.clone();
    app.mpris = Some(crate::mpris::test_handle(
        local_status.clone(),
        |_| {},
        None,
    ));

    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let remote_status = remote.status.clone();
    let sess = make_session("remote-host", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx);

    let handle = app.mpris.as_ref().expect("mpris handle still present");
    let bound_status = crate::mpris::test_status(handle);
    assert!(
        Arc::ptr_eq(&bound_status, &remote_status),
        "switch_to_direct_remote must rebind MPRIS to the new remote's status"
    );
    assert!(!Arc::ptr_eq(&bound_status, &local_status));
}

#[test]
fn switch_to_direct_remote_disconnects_the_previous_remote_on_a_remote_to_remote_swap() {
    // Same #233 regression, but for the Sessions-panel direct-remote
    // path's already-remote branch (a second "Direct Remote" upgrade
    // while already on one).
    use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
    use std::io::Read as _;
    use std::net::TcpListener;

    let listener_a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_a = listener_a.local_addr().unwrap();
    let daemon_a = std::thread::spawn(move || {
        let (stream, _) = listener_a.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let listener_b = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_b = listener_b.local_addr().unwrap();
    let daemon_b = std::thread::spawn(move || {
        let (stream, _) = listener_b.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let mut app = make_app_stub();
    let sess_a = make_session("daemon-a", "mbv");
    let (remote_a, remote_a_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_a), "token").unwrap();
    app.switch_to_direct_remote(&sess_a, remote_a, remote_a_rx);

    let sess_b = make_session("daemon-b", "mbv");
    let (remote_b, remote_b_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_b), "token").unwrap();
    app.switch_to_direct_remote(&sess_b, remote_b, remote_b_rx);

    let mut daemon_a_stream = daemon_a.join().unwrap();
    daemon_a_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut buf = [0u8; 8];
    let n = daemon_a_stream.read(&mut buf).unwrap_or(usize::MAX);
    assert_eq!(
        n, 0,
        "old direct-remote client socket must be shut down after the swap"
    );

    drop(daemon_b);
    let _ = addr_b;
}

#[test]
fn switch_to_library_route_sets_active_route_and_suspends_local() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route("music", remote, remote_rx);

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    // Must stay independent of the Sessions-panel direct-remote fields.
    assert!(app.connected_session_id.is_none());
    assert!(app.direct_remote_label.is_none());
}

#[test]
fn route_owned_transport_is_not_sessions_panel_disconnectable() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);

    app.switch_to_library_route("music", remote, remote_rx);
    app.status.clear();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::DirectRemote);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );

    app.disconnect_remote();

    assert_eq!(app.active_route.as_deref(), Some("music"));
    assert!(app.player.is_remote());
    assert!(app.suspended_local.is_some());
    assert!(app.remote_player_tab.is_some());
    assert_eq!(app.status, "No session connected");
}

#[test]
fn switch_to_library_route_sets_remote_queue_scope_when_daemon_has_items() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(2), 0);

    app.switch_to_library_route("music", remote, remote_rx);

    assert!(app.has_direct_remote_queue());
}

#[test]
fn switch_to_library_route_disconnects_the_previous_remote_on_a_route_to_route_swap() {
    // #233 regression guard: swapping from one active library route
    // straight to another (the already-remote branch) must tear down
    // the OLD RemotePlayer's connection before replacing it, not just
    // let it leak via Drop. Uses two real TCP loopback "daemons" (not
    // RemotePlayer::stub, which has no real socket to observe) so the
    // first daemon's accepted connection can observe its client side
    // actually closing.
    use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
    use std::io::Read as _;
    use std::net::TcpListener;

    let listener_a = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_a = listener_a.local_addr().unwrap();
    let daemon_a = std::thread::spawn(move || {
        let (stream, _) = listener_a.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let listener_b = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_b = listener_b.local_addr().unwrap();
    let daemon_b = std::thread::spawn(move || {
        let (stream, _) = listener_b.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let mut app = make_app_stub();
    let (remote_a, remote_a_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_a), "token").unwrap();
    app.switch_to_library_route("music", remote_a, remote_a_rx);
    assert!(!app.player.is_remote_disconnected());

    let (remote_b, remote_b_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr_b), "token").unwrap();
    app.switch_to_library_route("movies", remote_b, remote_b_rx);

    // The OLD (music) connection's daemon-side accept handle should
    // see its client hang up shortly after the swap -- proof the
    // reader thread actually exited instead of leaking.
    let mut daemon_a_stream = daemon_a.join().unwrap();
    daemon_a_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut buf = [0u8; 8];
    let n = daemon_a_stream.read(&mut buf).unwrap_or(usize::MAX);
    assert_eq!(
        n, 0,
        "old library route's client socket must be shut down after the swap"
    );

    drop(daemon_b);
    let _ = addr_b; // silence unused warning if daemon_b's thread hasn't been joined
}

#[test]
fn restore_local_mode_rebinds_mpris_back_to_the_suspended_local_status() {
    // #175 follow-through: after a Direct Remote takeover ends (however
    // it ends -- disconnect, user action, etc.), MPRIS must follow
    // playback back to the restored local `Player`, not stay wired to
    // the now-defunct remote session.
    let mut app = make_app_stub();
    let local_status = app.player.status.clone();
    app.mpris = Some(crate::mpris::test_handle(
        local_status.clone(),
        |_| {},
        None,
    ));

    let remote_items = make_items(1);
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let remote_status = remote.status.clone();
    let sess = make_session("remote-host", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx);

    app.restore_local_mode("test: ending direct remote session");

    let handle = app.mpris.as_ref().expect("mpris handle still present");
    let bound_status = crate::mpris::test_status(handle);
    assert!(
        Arc::ptr_eq(&bound_status, &local_status),
        "restore_local_mode must rebind MPRIS back to the restored local status"
    );
    assert!(!Arc::ptr_eq(&bound_status, &remote_status));
}

#[test]
fn restore_local_mode_clears_active_route() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.switch_to_library_route("music", remote, remote_rx);
    assert_eq!(app.active_route.as_deref(), Some("music"));

    app.restore_local_mode("Local playback restored");

    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
}

#[test]
fn restore_local_mode_disconnects_the_remote_before_restoring_local() {
    // #233 follow-up regression guard: `restore_local_mode` is the
    // shared "go back to local" tail. `self.player.join()` is a
    // documented no-op for a remote player, so the subsequent
    // `self.player = suspended.player` reassignment used to drop the
    // old RemotePlayer without ever disconnecting it, leaking its
    // reader thread exactly like the two already-fixed remote-to-remote
    // swap branches. Uses a real TCP loopback "daemon" (not
    // RemotePlayer::stub, which has no real socket to observe) so the
    // daemon's accepted connection can observe its client side actually
    // closing.
    use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};
    use std::io::Read as _;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        crate::app::tests::run_stub_daemon_handshake(stream)
    });

    let mut app = make_app_stub();
    let (remote, remote_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();
    app.switch_to_library_route("music", remote, remote_rx);
    assert!(!app.player.is_remote_disconnected());

    app.restore_local_mode("test: ending library route session");

    // The OLD (music) connection's daemon-side accept handle should see
    // its client hang up shortly after `restore_local_mode` runs --
    // proof the reader thread actually exited instead of leaking.
    let mut daemon_stream = daemon.join().unwrap();
    daemon_stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut buf = [0u8; 8];
    let n = daemon_stream.read(&mut buf).unwrap_or(usize::MAX);
    assert_eq!(
        n, 0,
        "old remote's client socket must be shut down after restore_local_mode"
    );
}

#[test]
fn remote_slot_state_is_local_daemon_for_thin_client_mode() {
    let app = make_local_daemon_app_stub(make_items(3));

    assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
    assert!(!app.can_disconnect_remote());
    assert_eq!(
        app.sessions_overlay_footer(),
        "[↵]conn [r]refresh [Esc]close"
    );
}

#[test]
fn attached_session_state_wins_over_local_daemon_indicator() {
    let mut app = make_local_daemon_app_stub(make_items(3));
    app.connected_session_id = Some("session-1".into());

    assert_eq!(app.remote_slot_state(), RemoteSlotState::AttachedSession);
    assert!(app.can_disconnect_remote());
}

#[test]
fn disconnect_remote_does_not_exit_local_daemon_mode() {
    let mut app = make_local_daemon_app_stub(make_items(3));

    app.disconnect_remote();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::LocalDaemon);
    assert!(app.player.is_remote());
    assert!(!app.can_disconnect_remote());
    assert_eq!(app.status, "No session connected");
}

#[test]
fn disconnect_remote_clears_attached_remote_session() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session-1".into());
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.session_miss_count = 2;
    app.remote_pos_s = 120;

    app.disconnect_remote();

    assert_eq!(app.remote_slot_state(), RemoteSlotState::Off);
    assert!(app.connected_session_id.is_none());
    assert!(app.connected_session_state.is_none());
    assert_eq!(app.session_miss_count, 0);
    assert_eq!(app.remote_pos_s, 0);
    assert_eq!(app.status, "Disconnected from remote session");
}

#[test]
fn disconnect_remote_restores_local_for_sessions_panel_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let sess = make_session("music", "mbv");

    app.switch_to_direct_remote(&sess, remote, remote_rx);

    assert_eq!(app.direct_remote_label.as_deref(), Some("music"));
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(app.direct_remote_label.is_none());
    assert!(app.active_route.is_none());
    assert!(!app.player.is_remote());
    assert_eq!(app.status, "Disconnected from direct remote session");
}

#[test]
fn disconnecting_attached_session_preserves_sessions_panel_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let direct_session = make_session("music", "mbv");
    let attached_session = make_session("living-room", "Emby");

    app.switch_to_direct_remote(&direct_session, remote, remote_rx);
    app.connect_to_session(&attached_session);

    assert!(app.direct_remote_connected);
    assert!(app.connected_session_id.is_some());

    app.disconnect_remote();

    assert!(app.player.is_remote());
    assert!(app.direct_remote_connected);
    assert!(app.can_disconnect_remote());

    app.disconnect_remote();

    assert!(!app.player.is_remote());
    assert!(!app.direct_remote_connected);
}

#[test]
fn displayed_queue_playback_state_stays_active_for_local_daemon_queue() {
    let app = make_local_daemon_app_stub(make_items(3));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 2;
        status.position_ticks = 42;
        status.runtime_ticks = 84;
        status.paused = true;
    }

    assert_eq!(
        app.displayed_queue_playback_state(),
        PlaybackState {
            active: true,
            active_idx: 2,
            position_ticks: 42,
            runtime_ticks: 84,
            paused: true,
        }
    );
}

#[test]
fn local_daemon_consume_adjusts_active_idx_after_removal_shift() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_local_daemon_app_stub(make_items(4));
    app.client.lock().unwrap().config.consume_videos = true;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 1,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    {
        let mut status = app.player.status.lock().unwrap();
        // Thin-client path: the remote player updates status.current_idx
        // from the daemon's TrackChanged event before App handles the
        // pending consume removal, so App must correct the shifted index.
        status.current_idx = 2;
    }
    app.handle_player_event(PlayerEvent::TrackChanged(2));

    assert_eq!(app.player_tab.queue_cursor, 1);
    assert_eq!(
        app.displayed_queue_playback_state().active_idx,
        1,
        "after removing the completed item, the active index must shift to \
             the now-playing item's new slot instead of following the stale \
             pre-removal numeric index"
    );
}

#[test]
fn direct_remote_consume_adjusts_active_idx_after_removal_shift() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(4);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.client.lock().unwrap().config.consume_videos = true;
    app.set_queue_scope(QueueScope::Remote);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 1,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    {
        let mut status = app.player.status.lock().unwrap();
        // Network direct-remote path receives the same raw pre-removal
        // TrackChanged index from the daemon as the same thin-client
        // control path covered above.
        status.current_idx = 2;
    }
    app.handle_player_event(PlayerEvent::TrackChanged(2));

    let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(&app.player_tab.items).unwrap(),
        serde_json::to_value(&local_items).unwrap()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().items),
        vec![
            remote_items[0].id.clone(),
            remote_items[2].id.clone(),
            remote_items[3].id.clone(),
        ]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert_eq!(
        app.displayed_queue_playback_state().active_idx,
        1,
        "after removing the completed remote item, the active index must \
             shift to the now-playing item's new remote-queue slot"
    );
}

#[test]
fn displayed_queue_playback_state_is_inactive_for_non_playback_scope() {
    let mut app = make_remote_app_stub(make_items(2), make_items(3));
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.connected_session_state
        .as_mut()
        .unwrap()
        .now_playing_item_id = Some("id1".into());
    app.set_queue_scope(QueueScope::Local);

    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
    assert_eq!(
        app.displayed_queue_playback_state(),
        PlaybackState::default()
    );
}

// ── cursor preservation during home refresh ──────────────────────────────

fn sections(n: usize) -> Vec<(String, String, Vec<MediaItem>, usize)> {
    (0..n)
        .map(|i| (format!("Sec {i}"), format!("lib{i}"), make_items(3), 0))
        .collect()
}

#[test]
fn home_refresh_preserves_cursor_by_lib_id() {
    // Simulate what init_home does: old_cursors keyed by lib_id.
    let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![
        (
            "Latest Movies".into(),
            "lib-movies".into(),
            make_items(10),
            7,
        ),
        ("Latest TV".into(), "lib-tv".into(), make_items(5), 3),
    ];
    let old_cursors: std::collections::HashMap<String, usize> = old_latest
        .iter()
        .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
        .collect();

    // New fetch returns same libs but with fresh items.
    let new_items_movies = make_items(12);
    let new_items_tv = make_items(4);

    let cursor_movies = old_cursors
        .get("lib-movies")
        .copied()
        .unwrap_or(0)
        .min(new_items_movies.len().saturating_sub(1));
    let cursor_tv = old_cursors
        .get("lib-tv")
        .copied()
        .unwrap_or(0)
        .min(new_items_tv.len().saturating_sub(1));

    assert_eq!(cursor_movies, 7, "cursor preserved when within bounds");
    assert_eq!(cursor_tv, 3, "cursor preserved when within bounds");
}

#[test]
fn home_refresh_clamps_cursor_when_new_list_is_shorter() {
    let old_latest: Vec<(String, String, Vec<MediaItem>, usize)> = vec![(
        "Latest Movies".into(),
        "lib-movies".into(),
        make_items(10),
        9,
    )];
    let old_cursors: std::collections::HashMap<String, usize> = old_latest
        .iter()
        .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
        .collect();

    let new_items = make_items(5); // shorter than before
    let cursor = old_cursors
        .get("lib-movies")
        .copied()
        .unwrap_or(0)
        .min(new_items.len().saturating_sub(1));

    assert_eq!(cursor, 4, "cursor clamped to new last index");
}

#[test]
fn home_refresh_cursor_defaults_zero_for_new_library() {
    let old_cursors: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let new_items = make_items(8);
    let cursor = old_cursors
        .get("brand-new-lib")
        .copied()
        .unwrap_or(0)
        .min(new_items.len().saturating_sub(1));
    assert_eq!(cursor, 0);
}

#[test]
fn home_section_clamped_after_refresh_removes_sections() {
    let mut app = make_app_stub();
    app.home.latest = sections(4); // 5 total
    app.home.section = 4;

    // Simulate refresh that returns fewer sections.
    app.home.latest = sections(1); // now only 2 total
    let n = 1 + app.home.latest.len();
    if app.home.section >= n {
        app.home.section = n.saturating_sub(1);
    }
    assert_eq!(app.home.section, 1);
}

// ── status_bar (Task 2: session/connection label + unsaved marker) ───────
//
// The remote/session-label resolution tests that used to live here
// (direct-remote label, attached-session device name, direct-upgrade
// session name, local-status default, left-side ordering, icon/label
// coloring) exercised the status bar's own `remote_status_spans`
// rendering with `show_session_pill: true` -- a mode only the deleted
// Standard view's status-bar call site used. Power View's status bar
// has always called `render_status_bar(.., false, true)` (unchanged by
// this diff -- confirmed via `git diff origin/main`), because Power
// surfaces the same remote/session info via the queue column's
// Local/Remote title pills instead (`render_power_queue_title` in
// `render/power/queue.rs`, which calls the same shared
// `remote_status_spans` helper). That underlying logic still matters in
// production, so those tests were moved to `render/mod.rs`'s test
// module, rewritten to call `render_status_bar` directly with
// `show_session_pill: true` (the same pattern already used by
// `status_bar_remote_hitbox_tracks_visible_pill_after_alive_marker`
// etc.) rather than deleted outright.

#[test]
fn status_bar_shows_emby_server_on_the_right_side() {
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.server_url = "http://emby.local:8096".into();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("emby.local"),
        "expected Emby server host on the right side of the status bar:\n{last_line}"
    );
    assert!(
        last_line.trim_end().ends_with("emby.local"),
        "Emby server host should be the rightmost status item:\n{last_line}"
    );
    assert!(
        !last_line.contains("server:"),
        "server host should not be prefixed on the right side:\n{last_line}"
    );
}

#[test]
fn status_bar_right_side_separates_items_with_spaces_and_keeps_server_rightmost() {
    let mut app = make_app_stub();
    // The AUTOSAVE/queue-source segment is only shown while the queue
    // side is focused (see `render_status_bar`'s `source_label` /
    // `autosave_on` gating) -- equivalent to the old "Queue tab" state.
    app.panel_focus = PanelFocus::Queue;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".into()),
        name: "Road Trip".into(),
    };
    {
        let mut cfg = app.client.lock().unwrap();
        cfg.config.server_url = "http://emby.local:8096".into();
        cfg.config.save_playlist_on_consume = true;
    }

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    let autosave_pos = last_line.find("AUTOSAVE").unwrap();
    let server_pos = last_line.find("emby.local").unwrap();
    assert!(
        autosave_pos < server_pos,
        "expected AUTOSAVE to remain left of the server pill:\n{last_line}"
    );
}

#[test]
fn status_bar_shows_muted_text_without_mute_glyph() {
    let mut app = make_app_stub();
    app.mute_on = true;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("muted"),
        "expected text label for muted state:\n{last_line}"
    );
    assert!(
        !last_line.contains(" m "),
        "muted state should not use the old single-letter mute glyph:\n{last_line}"
    );
    // Ordering relative to the remote pill isn't checked here: Power
    // View's status bar never shows the session/remote pill (that lives
    // in the queue column's Local/Remote title pills instead -- see
    // `render_power_queue_title`), so "muted last among left-side
    // statuses" no longer has a remote pill to be last *after*.
    let playlist_pos = last_line.find("\u{1F5AD}  none").unwrap();
    let muted_pos = last_line.find("muted").unwrap();
    assert!(
            playlist_pos < muted_pos,
            "muted should be the last left-side status so appearing/disappearing does not shift earlier statuses:\n{last_line}"
        );
}

#[test]
fn status_bar_hides_mute_indicator_when_not_muted() {
    let mut app = make_app_stub();
    app.mute_on = false;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("mute"),
        "unmuted state should not render a mute indicator:\n{last_line}"
    );
    assert_eq!(
        app.layout.playback.ind_mu,
        ratatui::layout::Rect::default(),
        "hidden mute indicator should not leave an invisible click target"
    );
}

#[test]
fn status_bar_stay_alive_heart_uses_row_background_not_pill_background() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.set_queue_scope(QueueScope::Remote);
    app.client.lock().unwrap().config.daemon_client_endpoint = "tcp://music.local:8097".into();
    let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));
    app.use_nerd_fonts = false;

    let term = render_app_to_terminal(&mut app, 80, 24);
    let buf = term.backend().buffer();
    let last_y = buf.area().height - 1;
    let heart_x = (0..buf.area().width)
        .find(|&x| buf[(x, last_y)].symbol() == "\u{2665}")
        .unwrap();

    assert_eq!(
        buf[(heart_x, last_y)].bg,
        // The status row itself renders on DARK_BG (not the old
        // transparent BAR_BG) -- see render_status_bar's `bar_style`.
        palette::DARK_BG,
        "expected the stay-alive heart to stay on the row background, not a pill background"
    );
}

#[test]
fn status_bar_has_no_session_or_daemon_label_when_remote_slot_is_off() {
    let mut app = make_app_stub();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("attached:") && !last_line.contains("daemon:"),
        "expected no attached-session or daemon label when nothing is connected:\n{last_line}"
    );
}

#[test]
fn status_bar_shows_playlist_status_when_none_is_active() {
    let mut app = make_app_stub();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("\u{1F5AD}  none"),
        "expected playlist glyph as the playlist label prefix:\n{last_line}"
    );
}

#[test]
fn status_bar_shows_active_playlist_name_next_to_playlist_glyph() {
    let mut app = make_app_stub();
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".into()),
        name: "Road Trip".into(),
    };

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("\u{1F5AD}  Road Trip"),
        "expected playlist glyph as the active playlist label prefix:\n{last_line}"
    );
}

#[test]
fn status_bar_has_surrounding_row_background_and_pill_cells() {
    let mut app = make_app_stub();

    let term = render_app_to_terminal(&mut app, 80, 24);
    let buf = term.backend().buffer();
    let last_y = buf.area().height - 1;
    let test_bg = palette::STATUS_PILL_BG;
    let mut saw_row_bg = false;
    let mut saw_pill_bg = false;

    for x in 0..buf.area().width {
        let bg = buf[(x, last_y)].bg;
        // The status row itself renders on DARK_BG (not the old
        // transparent BAR_BG) so the pill segments read as sitting on
        // top of it -- see render_status_bar's `bar_style`.
        saw_row_bg |= bg == palette::DARK_BG;
        saw_pill_bg |= bg == test_bg;
    }

    assert!(
        saw_row_bg,
        "expected the status bar row background to be visible"
    );
    assert!(
        saw_pill_bg,
        "expected pill-colored cells to sit on top of the row background"
    );
}

#[test]
fn status_bar_shows_unsaved_marker_on_any_tab_when_queue_is_dirty() {
    let mut app = make_app_stub();
    app.queue_dirty = true;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
            last_line.contains("UNSAVED"),
            "expected an UNSAVED marker regardless of the active tab when the queue is dirty:\n{last_line}"
        );
}

#[test]
fn status_bar_right_unsaved_does_not_touch_left_segment_when_space_is_tight() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.mute_on = false;
    app.client.lock().unwrap().config.daemon_client_endpoint = "tcp://music.local:8097".into();
    app.set_queue_scope(QueueScope::Remote);
    let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    app.stay_alive_ctrl = Some(stay_alive::StayAliveCtrl::for_test(app_end));
    app.queue_dirty = true;

    let rendered = render_app_to_string(&mut app, 39, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("aliveUNSAVED"),
        "right-side UNSAVED must not attach to the left status cluster:\n{last_line}"
    );
}

#[test]
fn status_bar_uses_unsaved_in_autosave_slot_when_dirty() {
    let mut app = make_app_stub();
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Road Trip".into(),
    };
    app.client.lock().unwrap().config.save_playlist_on_consume = true;
    app.queue_dirty = true;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("UNSAVED"),
        "dirty saved-playlist queue should show UNSAVED in the save-state slot:\n{last_line}"
    );
    assert!(
        !last_line.contains("AUTOSAVE"),
        "UNSAVED should replace AUTOSAVE while the queue is dirty:\n{last_line}"
    );
}

// ── status_bar (Task 3: right-aligned queue-state segment) ────────────────

#[test]
fn status_bar_shows_queue_source_label_on_queue_tab() {
    let mut app = make_app_stub();
    // Equivalent of the old "Queue tab": the queue side is focused, so
    // the queue-source label is relevant and should render.
    app.panel_focus = PanelFocus::Queue;
    app.queue_source = crate::config::QueueSource::Album;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("ALBUM"),
        "expected an ALBUM queue-source label when the queue side is focused:\n{last_line}"
    );
}

#[test]
fn status_bar_hides_queue_segment_outside_queue_and_power_view() {
    let mut app = make_app_stub();
    app.queue_source = crate::config::QueueSource::Album;

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
            !last_line.contains("ALBUM"),
            "queue source/autosave/scope detail must not leak onto tabs where it isn't relevant:\n{last_line}"
        );
}

#[test]
fn status_bar_omits_redundant_remote_queue_label() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.set_queue_scope(QueueScope::Remote);

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        !last_line.contains("REMOTE QUEUE"),
        "queue scope is already apparent from the UI and should not be repeated:\n{last_line}"
    );
}

// ── status_bar (Task 4: toast overlay replacement) ──────────────────────

#[test]
fn toast_renders_in_status_bar_without_covering_main_content_above_it() {
    let mut app = make_app_stub();
    app.status = "Saved [Y]".to_string();
    app.status_expires = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));

    let rendered = render_app_to_string(&mut app, 80, 24);
    let lines: Vec<&str> = rendered.lines().collect();
    let last_line = lines.last().unwrap();

    assert!(
        last_line.contains("Saved"),
        "expected the toast text on the final row:\n{last_line}"
    );
    // Old behavior covered 3 rows with Clear+overlay; new behavior must
    // only touch the single bottom row, leaving the row above untouched.
    let second_to_last = lines[lines.len() - 2];
    assert!(
        !second_to_last.contains("Saved"),
        "toast must not spill onto the row above the status bar:\n{second_to_last}"
    );
}

#[test]
fn status_bar_shows_normal_content_when_no_toast_is_active() {
    let mut app = make_app_stub();
    app.status = String::new();

    let rendered = render_app_to_string(&mut app, 80, 24);
    let last_line = rendered.lines().last().unwrap();

    assert!(
        last_line.contains("\u{1F5AD}  none"),
        "expected status labels to still render when no toast is active:\n{last_line}"
    );
}

// `status_bar_pill_click_regions_stay_populated_during_toast` (deleted):
// it asserted `layout.playback.ind_rc` (the remote-cycle click hitbox)
// stays populated while a toast covers the status bar. `ind_rc` is only
// ever populated when `render_status_bar` is called with
// `show_session_pill: true` -- the Standard-only call site that this PR
// removes. Power View's status bar call (`render/power/mod.rs`,
// unchanged by this diff) has always passed `show_session_pill: false`,
// so `ind_rc` never populates in Power regardless of any toast --
// remote/local cycling in Power happens by clicking the queue column's
// own Local/Remote title pills (`queue_scope_local_area` /
// `queue_scope_remote_area`, handled in `input.rs`), a separate row the
// status-bar toast never covers. The bug this test guarded (hitbox goes
// stale specifically *during* a toast) has no Power analog to regress.
