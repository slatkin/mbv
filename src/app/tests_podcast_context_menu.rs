use super::tests_podcast::{add_emby_movie_library, audiobookshelf_app};
use super::*;
use crate::app::tests::*;

#[test]
fn podcast_library_detects_collection_type() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.collection_type = "podcasts".into();
    library.is_folder = true;

    app.libs.push(LibraryTab::new(library));

    assert!(app.is_podcast_library(0));
}

#[test]
fn podcast_library_detects_name_when_collection_type_missing() {
    let mut app = make_app_stub();
    let mut library = make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.is_folder = true;

    app.libs.push(LibraryTab::new(library));

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
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    app.tab = TabSelection::EmbyLibrary(0);

    app.open_context_menu();

    let menu = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu"),
    };
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
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    app.tab = TabSelection::EmbyLibrary(0);

    app.open_context_menu();

    let menu = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu"),
    };
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert!(labels.contains(&"Mark Played"));
    assert!(!labels.contains(&"Mark Unplayed"));
}

#[test]
fn podcast_context_menu_offers_mark_all_played_for_selected_show() {
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
            music_grouping: None,
        }],
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
        ..LibraryTab::new(library)
    });
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    app.open_context_menu();

    let menu = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu"),
    };
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
fn podcast_context_menu_mark_all_played_uses_all_pill_selection() {
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
            music_grouping: None,
        }],
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
        ..LibraryTab::new(library)
    });
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    app.open_context_menu();

    let menu = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu"),
    };
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

/// F5 on the Audiobookshelf destination clears the current catalog and then
/// restarts the catalog request from the first page: shows/total/episodes are
/// reset, page 0 is marked pending, and neither the Emby library nor the
/// queue is touched.
#[test]
fn audiobookshelf_f5_restarts_catalog_after_clear() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::AudiobookshelfLibrary(0);

    app.refresh_current_view();

    let state = &app.audiobookshelf_browse[0];
    assert!(state.shows.is_empty(), "catalog must be cleared on refresh");
    assert_eq!(state.total, 0);
    assert!(state.episodes.is_none());
    assert_eq!(state.episode_selection, None);
    assert!(
        state.loading_pages.contains(&0),
        "page 0 must be marked pending so the catalog request restarts"
    );
    assert!(
        !app.libs[0].nav_stack[0].loading,
        "Audiobookshelf refresh must not reload the Emby library"
    );
    assert_eq!(
        app.player_tab.total_queue_len(),
        0,
        "Audiobookshelf refresh must not touch the queue"
    );
}
