use super::*;
use crate::app::tests::*;

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
