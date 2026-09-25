use super::*;

pub(crate) fn audiobookshelf_app() -> App {
    let mut app = make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(
        library.clone(),
    );
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
    state.detail_cache.insert(
        "show-a".into(),
        vec![mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            description: None,
            published_at: None,
            duration_seconds: None,
        }],
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

/// A populated Emby movie library, so a key or action that leaks across the
/// Service seam would have Emby state to mutate.
pub(super) fn add_emby_movie_library(app: &mut App) {
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
}
/// The Audiobookshelf destination never opens an Emby context menu, even when
#[test]
fn audiobookshelf_tab_never_opens_an_emby_context_menu() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);

    app.open_context_menu(false, None);
    assert!(
        !matches!(
            app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(_))
        ),
        "Audiobookshelf must not open an Emby context menu"
    );

    // Control: selecting the Emby library with the same state does produce a
    // menu, so the absence above is the destination guard, not an empty setup.
    app.tab = TabSelection::EmbyLibrary(0);
    app.open_context_menu(false, None);
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(_))
    ));
}

#[test]
fn feeds_destination_never_opens_an_emby_context_menu() {
    let mut app = make_app_stub();
    add_emby_movie_library(&mut app);
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::Feeds;

    app.open_context_menu(false, None);
    assert!(
        !matches!(
            app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(_))
        ),
        "Feeds must not open an Emby context menu"
    );

    // Control: selecting the Emby library with the same state does produce a
    // menu, so the absence above is the destination guard, not an empty setup.
    app.tab = TabSelection::EmbyLibrary(0);
    app.open_context_menu(false, None);
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(_))
    ));
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

    app.open_context_menu(false, None);
    let menu = match app.pending_overlay.as_ref() {
        Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("queue panel must open a menu"),
    };
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

/// The provider-specific resolver seams remain read-only; task 4.3 consumes
/// their QueueItem result through ordinary actions.
#[test]
fn audiobookshelf_episode_activation_seams_do_not_mutate_queue() {
    let mut app = audiobookshelf_app();
    add_emby_movie_library(&mut app);
    let before_queue = app.player_tab.total_queue_len();
    let before_nav = app.libs[0].nav_stack.len();
    let before_active = app.player.status.lock().unwrap().active;

    app.activate_audiobookshelf_episode(0, 0);
    app.enqueue_audiobookshelf_episode(0, 0);

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
    state.detail_cache.insert(
        "show-a".into(),
        vec![mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            description: None,
            published_at: Some(1_704_153_600),
            duration_seconds: Some(1234.5),
        }],
    );
    state.progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 42.5,
            is_finished: false,
        },
    );
    let item = app
        .activate_audiobookshelf_episode(0, 0)
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
        .enqueue_audiobookshelf_episode(0, 0)
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
    // Out-of-range index against the loaded list.
    assert!(app.activate_audiobookshelf_episode(0, 99).is_none());
    assert!(app.enqueue_audiobookshelf_episode(0, 99).is_none());
    assert_eq!(app.player_tab.total_queue_len(), 0);

    // Empty visible list.
    app.audiobookshelf_browse[0]
        .detail_cache
        .insert("show-a".into(), Vec::new());
    assert!(app.activate_audiobookshelf_episode(0, 0).is_none());
    assert!(app.enqueue_audiobookshelf_episode(0, 0).is_none());
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

/// An absent or stale Audiobookshelf index is a silent no-op for both seams.
#[test]
fn audiobookshelf_episode_seams_noop_on_absent_index() {
    let mut app = audiobookshelf_app();
    app.activate_audiobookshelf_episode(1, 0);
    app.enqueue_audiobookshelf_episode(1, 0);
    assert_eq!(app.player_tab.total_queue_len(), 0);
    assert_eq!(app.audiobookshelf_browse.len(), 1);
    assert!(matches!(app.tab, TabSelection::AudiobookshelfLibrary(0)));
}

/// The saved-position path no longer treats a show id as the tab's
/// selection (task 2.3): saving writes no focused item even with a selected
/// show, and restoring a legacy show-id position does not adopt it.
#[test]
fn podcast_saved_positions_do_not_record_or_restore_a_show_id() {
    let mut app = audiobookshelf_app();
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("https://podcasts.example"),
    );

    // A selected show is still not recorded as the position's focused item.
    assert_eq!(
        app.audiobookshelf_browse[0].selected_id.as_deref(),
        Some("show-a")
    );
    app.save_audiobookshelf_position(0);
    let saved = &app.library_position_state.libraries
        ["audiobookshelf:https://podcasts.example:abs-podcasts"];
    assert_eq!(saved.levels[0].focused_item_id, None);

    // A legacy saved position names a show id; restore ignores it instead of
    // treating it as the tab's selection.
    let mut app = audiobookshelf_app();
    app.library_position_state.libraries.insert(
        "audiobookshelf:https://podcasts.example:abs-podcasts".into(),
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                focused_item_id: Some("show-old".into()),
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    // Tab off the library so restore performs no episode fetch.
    app.tab = TabSelection::Home;
    app.activate_audiobookshelf_position(0);
    assert_eq!(
        app.audiobookshelf_browse[0].selected_id.as_deref(),
        Some("show-a"),
        "restore starts on the first show, never the saved show id"
    );
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
    assert!(state.detail_cache.is_empty());
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

mod loading;
mod playback;
