use crate::app::state::types::browse::BrowseResting;
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
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder],
            total_count: 1,
            resting: BrowseResting::new(0, 0),
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
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });
    assert!(!app.is_feed_home_video_group_view(0));

    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];
    assert!(app.is_feed_home_video_group_view(0));
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
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
            total_count: 1,
            resting: BrowseResting::new(0, 0),
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
        ..LibraryTab::new(library)
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
fn ensure_feed_home_video_group_level_clamps_stale_cursor_to_available_groups() {
    // A stale selected group from a prior aggregation run with more groups
    // must clamp to the groups that actually exist now.
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
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
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],
            total_count: 1,
            resting: BrowseResting::new(0, 0),
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
        ..LibraryTab::new(library)
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
