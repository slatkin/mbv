use super::*;
use crate::app::tests::*;

pub(super) fn audiobookshelf_app() -> App {
    let mut app = make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state =
        super::types_audiobookshelf_browse::AudiobookshelfBrowseState::new(library.clone());
    state.append_page(
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
    state.episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

/// A populated Emby movie library, so a key or action that leaks across the
/// Service seam would have Emby state to mutate.
fn add_emby_movie_library(app: &mut App) {
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
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
}

/// PR #514's keyboard guard: while the Audiobookshelf tab is selected and the
/// library panel has focus, Emby-only keys (search, watched, shuffle,
/// enqueue, context menu) must be consumed without touching Emby, queue,
/// playback, or help state.
#[test]
fn audiobookshelf_tab_keys_cannot_enter_emby_action_paths() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);
    let nav_len = app.libs[0].nav_stack.len();

    let slash = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('/'),
        crossterm::event::KeyModifiers::NONE,
    );
    let ctrl_w = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('w'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let ctrl_s = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('s'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let ctrl_a = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('a'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let ctrl_r = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('r'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let dot = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('.'),
        crossterm::event::KeyModifiers::NONE,
    );

    for key in [slash, ctrl_w, ctrl_s, ctrl_a, ctrl_r, dot] {
        assert_eq!(
            app.handle_key_view_dispatch(key),
            Some(false),
            "Audiobookshelf tab must consume {key:?}"
        );
        assert!(
            app.libs[0].search.is_none(),
            "{key:?} must not open an Emby search"
        );
        assert_eq!(
            app.libs[0].nav_stack.len(),
            nav_len,
            "{key:?} must not navigate the Emby library"
        );
        assert!(!app.libs[0].nav_stack[0].items[0].played);
        assert_eq!(
            app.player_tab.total_queue_len(),
            0,
            "{key:?} must not enqueue anything"
        );
        assert!(app.context_menu.is_none(), "{key:?} must not open a menu");
        assert!(
            app.confirm_modal.is_none(),
            "{key:?} must not open a rescan confirmation"
        );
        assert!(
            app.status.is_empty(),
            "{key:?} must not flash, got {:?}",
            app.status
        );
    }
    assert!(matches!(app.tab, TabSelection::AudiobookshelfLibrary(_)));
}

/// Unsupported-owner play remains inert while enqueue still edits the
/// Composed queue without involving the owner.
#[test]
fn audiobookshelf_episode_space_and_enqueue_are_inert_without_owner() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);
    app.enter_audiobookshelf_episode_selection();
    let nav_len = app.libs[0].nav_stack.len();

    let space = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(' '),
        crossterm::event::KeyModifiers::NONE,
    );
    assert_eq!(app.handle_key_view_dispatch(space), Some(false));
    let ctrl_a = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('a'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    assert_eq!(app.handle_key_view_dispatch(ctrl_a), Some(false));

    assert_eq!(
        app.audiobookshelf_browse[0].episode_selection,
        Some(0),
        "selected episode must be preserved"
    );
    assert_eq!(app.player_tab.total_queue_len(), 1);
    assert_eq!(
        app.libs[0].nav_stack.len(),
        nav_len,
        "inert activation must not navigate the Emby library"
    );
    assert!(app
        .status
        .contains("Audiobookshelf playback owner is unavailable"));
}

/// The Audiobookshelf destination never opens an Emby context menu, even when
/// a populated Emby library would produce one if selected.
#[test]
fn audiobookshelf_tab_never_opens_an_emby_context_menu() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);

    app.open_context_menu();
    assert!(
        app.context_menu.is_none(),
        "Audiobookshelf must not open an Emby context menu"
    );

    // Control: selecting the Emby library with the same state does produce a
    // menu, so the absence above is the destination guard, not an empty setup.
    app.tab = TabSelection::EmbyLibrary(0);
    app.open_context_menu();
    assert!(app.context_menu.is_some());
}

#[test]
fn feeds_destination_never_opens_an_emby_context_menu() {
    let mut app = make_app_stub();
    add_emby_movie_library(&mut app);
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::Feeds;

    app.open_context_menu();
    assert!(
        app.context_menu.is_none(),
        "Feeds must not open an Emby context menu"
    );

    // Control: selecting the Emby library with the same state does produce a
    // menu, so the absence above is the destination guard, not an empty setup.
    app.tab = TabSelection::EmbyLibrary(0);
    app.open_context_menu();
    assert!(app.context_menu.is_some());
}

/// An Emby item in the queue panel still opens the queue panel menu (Remove
/// from Queue / Go to Library), independent of the selected browse destination.
#[test]
fn emby_queue_item_still_opens_queue_panel_menu() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.player_tab
        .set_items(vec![make_item("Queue Movie", "Movie")], 0);

    app.open_context_menu();
    let menu = app
        .context_menu
        .as_ref()
        .expect("queue panel must open a menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert!(
        labels.contains(&"Remove from Queue"),
        "queue item menu must include Remove from Queue, got {labels:?}"
    );
    assert!(
        labels.contains(&"Go to Library"),
        "queue item menu must include Go to Library, got {labels:?}"
    );
}

#[test]
fn audiobookshelf_activation_enters_selection_then_remains_inert_without_owner() {
    let mut app = audiobookshelf_app();
    let enter = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    );

    assert_eq!(
        app.handle_key_view_dispatch(enter),
        Some(false),
        "Enter must be consumed"
    );
    assert_eq!(app.audiobookshelf_browse[0].episode_selection, Some(0));
    assert_eq!(app.player_tab.total_queue_len(), 0);

    assert_eq!(
        app.handle_key_view_dispatch(enter),
        Some(false),
        "second Enter stays inert"
    );
    assert_eq!(app.audiobookshelf_browse[0].episode_selection, Some(0));
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

/// The provider-specific resolver seams remain read-only; task 4.3 consumes
/// their QueueItem result through ordinary actions.
#[test]
fn audiobookshelf_episode_activation_seams_do_not_mutate_queue() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);
    app.enter_audiobookshelf_episode_selection();
    let before_selection = app.audiobookshelf_browse[0].episode_selection;
    let before_queue = app.player_tab.total_queue_len();
    let before_nav = app.libs[0].nav_stack.len();
    let before_active = app.player.status.lock().unwrap().active;

    app.activate_audiobookshelf_episode(0);
    app.enqueue_audiobookshelf_episode(0);

    assert_eq!(
        app.audiobookshelf_browse[0].episode_selection, before_selection,
        "activation seams must preserve the selected episode"
    );
    assert_eq!(
        app.player_tab.total_queue_len(),
        before_queue,
        "activation seams must not mutate the queue"
    );
    assert_eq!(
        app.libs[0].nav_stack.len(),
        before_nav,
        "activation seams must not navigate the Emby library"
    );
    assert_eq!(
        app.player.status.lock().unwrap().active,
        before_active,
        "activation seams must not change playback state"
    );
    assert!(matches!(app.tab, TabSelection::AudiobookshelfLibrary(0)));
    assert!(!app.audiobookshelf_browse[0].shows.is_empty());
}

#[test]
fn audiobookshelf_episode_handlers_build_native_item_from_read_only_snapshot() {
    let mut app = audiobookshelf_app();
    let state = &mut app.audiobookshelf_browse[0];
    state.episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: Some("2024-01-02T00:00:00Z".into()),
            duration_seconds: Some(1234.5),
        },
    ]);
    state.progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 42.5,
            is_finished: false,
        },
    );
    state.enter_episode_selection();

    let item = app
        .activate_audiobookshelf_episode(0)
        .expect("selected downloaded episode");
    let queued = item.as_audiobookshelf().expect("Audiobookshelf QueueItem");
    assert_eq!(queued.library_item_id, "show-a");
    assert_eq!(queued.episode_id, "episode-a");
    assert_eq!(queued.title, "Episode A");
    assert_eq!(queued.show_title.as_deref(), Some("Show A"));
    assert_eq!(
        queued.duration_ticks,
        Some((1234.5 * mbv_core::api::TICKS_PER_SECOND as f64).round() as u64)
    );
    assert_eq!(
        queued.position_ticks,
        (42.5 * mbv_core::api::TICKS_PER_SECOND as f64).round() as i64
    );
    assert_eq!(queued.pub_date_secs, Some(1_704_153_600));
    assert!(!queued.is_finished);

    let serialized = serde_json::to_string(&item).unwrap();
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("sessionId"));
    assert!(!serialized.contains("Authorization"));
    assert_eq!(app.player_tab.total_queue_len(), 0);

    let enqueued = app
        .enqueue_audiobookshelf_episode(0)
        .expect("selected downloaded episode");
    assert_eq!(
        enqueued.as_audiobookshelf().unwrap().content_id(),
        queued.content_id()
    );
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

#[test]
fn audiobookshelf_episode_handlers_leave_unselected_rows_without_queue_items() {
    let mut app = audiobookshelf_app();
    assert!(app.activate_audiobookshelf_episode(0).is_none());
    assert!(app.enqueue_audiobookshelf_episode(0).is_none());

    app.audiobookshelf_browse[0].enter_episode_selection();
    app.audiobookshelf_browse[0].episode_selection = Some(99);
    assert!(app.activate_audiobookshelf_episode(0).is_none());
    assert!(app.enqueue_audiobookshelf_episode(0).is_none());
    assert_eq!(app.player_tab.total_queue_len(), 0);

    app.audiobookshelf_browse[0].episodes = Some(Vec::new());
    app.audiobookshelf_browse[0].episode_selection = Some(0);
    assert!(app.activate_audiobookshelf_episode(0).is_none());
    assert!(app.enqueue_audiobookshelf_episode(0).is_none());
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

/// An absent or stale Audiobookshelf index is a silent no-op for both seams.
#[test]
fn audiobookshelf_episode_seams_noop_on_absent_index() {
    let mut app = audiobookshelf_app();
    app.activate_audiobookshelf_episode(1);
    app.enqueue_audiobookshelf_episode(1);
    assert_eq!(app.player_tab.total_queue_len(), 0);
    assert_eq!(app.audiobookshelf_browse.len(), 1);
    assert!(matches!(app.tab, TabSelection::AudiobookshelfLibrary(0)));
}

#[test]
fn audiobookshelf_escape_returns_to_show_selection() {
    let mut app = audiobookshelf_app();
    let escape = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    );
    assert_eq!(
        app.handle_key_view_dispatch(escape),
        Some(false),
        "Escape must be consumed"
    );
    assert_eq!(app.audiobookshelf_browse[0].episode_selection, None);
    assert_eq!(
        app.audiobookshelf_browse[0].selected_id.as_deref(),
        Some("show-a")
    );
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
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: None,

        album_track_focus: None,
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
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: None,

        album_track_focus: None,
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
        search: None,
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
        feed_home_video: None,

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.tab = TabSelection::EmbyLibrary(0);

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
        search: None,
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
        feed_home_video: None,

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.tab = TabSelection::EmbyLibrary(0);

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("context menu");
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
        library,
        search: None,
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

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

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
        library,
        search: None,
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

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

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
