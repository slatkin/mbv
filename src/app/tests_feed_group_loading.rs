use super::*;
use crate::app::tests::*;

#[test]
fn feed_home_video_root_does_not_auto_push_before_folder_pagination_completes() {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
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
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
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
        level: Box::new(BrowseLevel {
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
            music_grouping: None,
        }),
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

fn make_home_video_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
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
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    app
}

fn seed_home_video_root_loaded(app: &mut App) -> EmbyItem {
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
        level: Box::new(BrowseLevel {
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
            music_grouping: None,
        }),
    });

    active
}

#[test]
fn feed_home_video_loaded_does_not_push_sublevel() {
    let mut app = make_home_video_app();
    seed_home_video_root_loaded(&mut app);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
}

#[test]
fn feed_home_video_aggregated_populates_groups_and_all_items() {
    let mut app = make_home_video_app();
    let active = seed_home_video_root_loaded(&mut app);

    let mut video = make_item("Episode 1", "Movie");
    video.path = "/videos/active/ep1.mp4".into();

    app.handle_lib_event(LibEvent::FeedHomeVideoAggregated {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        all_items: vec![video.clone()],
        groups: vec![FeedHomeVideoGroup {
            folder: active,
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
}

#[test]
fn feed_home_video_aggregated_ensure_group_level_does_not_push_and_resolves_selected_items() {
    let mut app = make_home_video_app();
    let active = seed_home_video_root_loaded(&mut app);

    let mut video = make_item("Episode 1", "Movie");
    video.path = "/videos/active/ep1.mp4".into();

    app.handle_lib_event(LibEvent::FeedHomeVideoAggregated {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        all_items: vec![video.clone()],
        groups: vec![FeedHomeVideoGroup {
            folder: active,
            items: vec![video],
        }],
    });

    app.ensure_feed_home_video_group_level(0);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(
        app.feed_home_video_selected_items(0)[0].path,
        "/videos/active/ep1.mp4"
    );
}

#[test]
fn refreshed_does_not_overwrite_feed_root_with_video_items() {
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
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
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
        ..LibraryTab::new(library)
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
    app.tab = TabSelection::EmbyLibrary(0);
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
        ..LibraryTab::new(library)
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
