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
        search: None,
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
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    assert!(!app.is_feed_home_video_group_view(0));

    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];
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
        search: None,
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
                music_grouping: None,
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
                music_grouping: None,
            },
        ],
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
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];
    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn fetch_home_preserves_feed_home_video_state() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

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
        search: None,
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
            music_grouping: None,
        }],
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
        search: None,
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
            music_grouping: None,
        }],
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
        search: None,
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
            music_grouping: None,
        }],
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
        search: None,
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
            music_grouping: None,
        }],
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
        search: None,
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
            music_grouping: None,
        }],
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
    app.tab = TabSelection::Library(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

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
        search: None,
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
            music_grouping: None,
        }],
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
fn ensure_feed_home_video_group_level_clamps_stale_cursor_to_available_groups() {
    // A stale selected group from a prior aggregation run with more groups
    // must clamp to the groups that actually exist now.
    let mut app = make_app_stub();
    app.tab = TabSelection::Library(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

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
        search: None,
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
            music_grouping: None,
        }],
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
fn refresh_lib_targets_feed_selection() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Library(0);
    app.panel_focus = PanelFocus::Library;
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

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
        search: None,
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
            music_grouping: None,
        }],
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
