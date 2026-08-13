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

/// One Emby library, one populated Audiobookshelf library, and a feed
/// subscription, so a mis-targeted `refresh_current_view` would have other
/// destinations' state to disturb.
fn mixed_services_app() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
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
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
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
    abs_state.episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);
    app.audiobookshelf_libraries.push(abs_library);
    app.audiobookshelf_browse.push(abs_state);
    app.feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: mbv_core::config::FeedKind::Audio,
    }];
    app.feed_tab.entries.resize_with(1, Vec::new);
    app
}

/// F5 with a library selected reloads only that Emby library: it marks the
/// matched library's browse level loading and leaves the Audiobookshelf
/// catalog and the Feeds tab untouched.
#[test]
fn refresh_current_view_targets_the_focused_emby_library_only() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;

    app.refresh_current_view();

    assert!(
        app.libs[0].nav_stack[0].loading,
        "focused Emby library must be marked loading"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].shows.len(),
        1,
        "Audiobookshelf catalog must not be cleared"
    );
    assert!(
        app.audiobookshelf_browse[0].episodes.is_some(),
        "Audiobookshelf episodes must be preserved"
    );
    assert!(!app.feed_tab.loading, "Feeds must not be refreshed");
}

/// F5 with the queue panel focused refreshes only the visible queue and
/// leaves every browse destination (Emby, Audiobookshelf, Feeds) untouched.
#[test]
fn refresh_current_view_with_queue_focus_leaves_browse_destinations_untouched() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Queue;

    app.refresh_current_view();

    assert!(
        !app.libs[0].nav_stack[0].loading,
        "queue refresh must not reload the Emby library"
    );
    assert_eq!(app.audiobookshelf_browse[0].shows.len(), 1);
    assert!(app.audiobookshelf_browse[0].episodes.is_some());
    assert!(!app.feed_tab.loading);
}

/// A stale Emby `lib_idx` reaching a bounds-checked browse helper mutates
/// nothing and never corrupts another library via a library-zero fallback.
#[test]
fn stale_emby_lib_index_mutates_no_library() {
    let mut app = two_emby_libraries_app();
    let stale = app.libs.len() + 1;
    app.move_lib_cursor(stale, 1);
    app.jump_lib_cursor(stale, true);
    app.go_back(stale);
    app.shuffle_play(stale);
    app.refresh_lib(stale);
    for idx in 0..2 {
        let lvl = &app.libs[idx].nav_stack[0];
        assert_eq!(lvl.cursor, 0, "lib {idx} cursor");
        assert!(!lvl.loading, "lib {idx} not loading");
    }
}
/// Two Emby libraries (each a two-item top browse level) so a stale index
/// that wrongly fallbacked to library zero could corrupt real state.
fn two_emby_libraries_app() -> App {
    let mut app = make_app_stub();
    for (id, title) in [("lib-movies", "Movies"), ("lib-music", "Music")] {
        let mut library = make_item(title, "CollectionFolder");
        library.id = id.into();
        let level = BrowseLevel {
            parent_id: id.into(),
            title: title.into(),
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
        };
        app.libs.push(LibraryTab {
            library,
            search: None,
            nav_stack: vec![level],
            feed_home_video: None,
            album_track_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
    }
    app
}
/// A stale Service library index (removed or replaced while selected) must
/// select Home and report that the triggering destination-specific
/// operation must stop.
#[test]
fn normalize_stale_browse_destination_resolves_stale_service_indexes_to_home() {
    // Stale Emby index with no Emby libraries.
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    assert!(app.normalize_stale_browse_destination());
    assert!(app.tab.is_home());

    // Stale Audiobookshelf index with no Audiobookshelf libraries.
    let mut app = make_app_stub();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    assert!(app.normalize_stale_browse_destination());
    assert!(app.tab.is_home());

    // A valid Audiobookshelf index is not stale even when Emby has none.
    let mut app = mixed_services_app();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::AudiobookshelfLibrary(0));
}

/// Valid destinations and the non-Service tabs must be left unchanged and
/// report `false`.
#[test]
fn normalize_stale_browse_destination_leaves_valid_destinations_alone() {
    let mut app = mixed_services_app();
    app.tab = TabSelection::EmbyLibrary(0);
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));

    app.tab = TabSelection::Home;
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::Home);

    app.tab = TabSelection::Feeds;
    assert!(!app.normalize_stale_browse_destination());
    assert_eq!(app.tab, TabSelection::Feeds);
}
