use super::*;
use crate::app::tests::*;

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
