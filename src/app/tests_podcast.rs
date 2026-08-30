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
pub(super) fn add_emby_movie_library(app: &mut App) {
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![make_item("Item 0", "Movie")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
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
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "Audiobookshelf must not open an Emby context menu"
    );

    // Control: selecting the Emby library with the same state does produce a
    // menu, so the absence above is the destination guard, not an empty setup.
    app.tab = TabSelection::EmbyLibrary(0);
    app.open_context_menu(false, None);
    assert!(matches!(
        app.pending_overlay,
        Some(super::types_overlay::OverlayRequest::ContextMenu(_))
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
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "Feeds must not open an Emby context menu"
    );

    // Control: selecting the Emby library with the same state does produce a
    // menu, so the absence above is the destination guard, not an empty setup.
    app.tab = TabSelection::EmbyLibrary(0);
    app.open_context_menu(false, None);
    assert!(matches!(
        app.pending_overlay,
        Some(super::types_overlay::OverlayRequest::ContextMenu(_))
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
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
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
    app.audiobookshelf_browse[0].episodes = Some(Vec::new());
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

