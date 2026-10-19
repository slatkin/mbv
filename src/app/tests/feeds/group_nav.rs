use crate::app::tests::*;
use mbv_ui_model::browse::BrowseResting;
use mbv_ui_model::browse::ServerRows;
use std::collections::HashMap;

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
            rows: ServerRows::new(1),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder],

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

/// Regression (user report 2026-10-18): the pill row (Latest, All, groups)
/// vanished once the state held nothing listable. A present-but-empty state
/// keeps the group view, per destination-latest-modes ("its Latest pill
/// remains selectable").
#[test]
fn feed_home_video_group_view_survives_an_empty_state() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(0),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![],

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
        // Nothing loaded yet: not loading, no counted folders, no items.
        feed_home_video: Some(FeedHomeVideoState::default()),
        ..LibraryTab::new(library)
    });
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    assert!(app.is_feed_home_video_group_view(0));
}

#[test]
fn select_feed_folder_group_all_uses_the_all_selection_cache() {
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
            rows: ServerRows::new(1),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],

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
            items: HashMap::from([
                (FeedHomeVideoSelection::All, vec![video.clone()]),
                (
                    FeedHomeVideoSelection::Folder(folder.id.clone()),
                    vec![video],
                ),
            ]),
            loading: false,
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
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selection.clone()),
        Some(FeedHomeVideoSelection::All)
    );
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0)[0].id, "video-a1");
}

#[test]
fn select_feed_folder_group_resolves_the_folder_id_from_the_listing() {
    // Design D3: pill selection resolves its folder id from the root
    // listing's counted folders, never from a state-held group list.
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

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(1),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],

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
            folder_counts: HashMap::from([(folder.id.clone(), 2)]),
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    app.select_feed_folder_group(0, 1);

    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selection.clone()),
        Some(FeedHomeVideoSelection::Folder("folder-a".into()))
    );
    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selected_group),
        Some(1)
    );
    assert_eq!(
        app.feed_home_video_selected_parent_id(0).as_deref(),
        Some("folder-a")
    );
}

#[test]
fn feed_home_video_selected_parent_id_follows_the_selection() {
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(0),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![],

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
            selection: FeedHomeVideoSelection::Folder("folder-a".into()),
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });
    assert_eq!(
        app.feed_home_video_selected_parent_id(0).as_deref(),
        Some("folder-a")
    );

    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.selection = FeedHomeVideoSelection::All;
    }
    assert_eq!(
        app.feed_home_video_selected_parent_id(0).as_deref(),
        Some("lib-youtube")
    );
}

#[test]
fn feed_home_video_counted_folders_derive_pills_from_positive_counts() {
    // Task 3.1 shape contract: pill membership comes from the root listing
    // filtered by the per-folder video count map — a folder with no videos
    // gets no pill, a folder with videos (watched or not) keeps its pill,
    // and listing order decides pill order.
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    let mut empty = make_item("Empty Channel", "Folder");
    empty.id = "folder-empty".into();
    empty.is_folder = true;
    let mut active = make_item("Active Channel", "Folder");
    active.id = "folder-active".into();
    active.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(2),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![empty, active.clone()],

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
            folder_counts: HashMap::from([(active.id.clone(), 3)]),
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    let counted = app.feed_home_video_counted_folders(0);
    assert_eq!(
        counted
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec!["folder-active"]
    );
}

#[test]
fn ensure_feed_home_video_group_level_clamps_stale_cursor_to_available_folders() {
    // A stale selected group from a listing change with more counted folders
    // must clamp to the folder pills that exist now.
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
            rows: ServerRows::new(1),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],

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
            selection: FeedHomeVideoSelection::Folder(folder.id.clone()),
            items: HashMap::from([(
                FeedHomeVideoSelection::Folder(folder.id.clone()),
                vec![video],
            )]),
            folder_counts: HashMap::from([(folder.id, 1)]),
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

/// Contract (tasks.md 3.2, rule 5): when the folder listing completes, the
/// persisted pill index (`selected_group`) resolves into the concrete
/// selection against the listing's counted folders (design D3).
#[test]
fn listing_completion_resolves_the_pending_pill_index_into_the_selection() {
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

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(1),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![],

            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 1,
            folder_counts: HashMap::from([("folder-a".to_string(), 2)]),
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: Box::new(BrowseLevel {
            rows: ServerRows::loaded(1, 1),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![folder.clone()],

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
        }),
    }));

    assert_eq!(
        app.libs[0]
            .feed_home_video
            .as_ref()
            .map(|state| state.selection.clone()),
        Some(FeedHomeVideoSelection::Folder("folder-a".into()))
    );
    assert_eq!(
        app.feed_home_video_selected_parent_id(0).as_deref(),
        Some("folder-a")
    );
}
