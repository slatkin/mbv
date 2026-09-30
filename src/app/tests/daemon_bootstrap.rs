use super::*;

#[test]
fn remote_app_starts_on_remote_queue_when_remote_queue_has_items() {
    let app = make_remote_app_stub(make_items(2), make_items(1));

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
}

// `item_text_and_style` and its dedicated tests above were deleted
// (#361): its only production caller was the deleted Standard
// `render/library/table/context.rs`.

// Task 4.1: A later capable client adopts the daemon's live Audiobookshelf
// queue (active slot, position), and reconciles browse state on adoption via
// the daemon progress event path.
#[test]
fn local_daemon_app_keeps_live_abs_queue_and_reconciles_browse_on_adoption() {
    // Isolated state dir: without this the empty-remote bootstrap reads
    // ambient queue_state (real home in isolation, another test's leftovers
    // in-suite), arms a phantom adoption that fails against the stub's
    // dropped command channel, and pays ~1s of disconnect-failure handling.
    let _guard = crate::config::TestStateDirGuard::new();
    // Create a local daemon app with no Emby remote items so the live queue
    // starts empty — we inject an ABS slot directly below.
    let mut app = make_local_daemon_app_stub(Vec::new());

    // Set up ABS browse state (mirrors audiobookshelf_app() setup).
    let library = mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut browse =
        mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState::new(library.clone());
    browse.detail_cache.insert(
        "show-a".into(),
        vec![mbv_audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            description: None,
            published_at: None,
            duration_seconds: Some(300.0),
        }],
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(browse);

    // Inject the live ABS queue slot (simulates the daemon broadcasting its
    // queue to the newly attached client via PlayerEvent::UnifiedQueueUpdated).
    let acknowledged_position_ticks = 30 * mbv_emby_model::TICKS_PER_SECOND;
    let abs_item = mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: acknowledged_position_ticks,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        },
    ));
    app.local_view.adopt_queue_items(vec![abs_item], 0);
    assert_eq!(app.local_view.total_queue_len(), 1);

    let ep = app.local_view.playback_queue().slots()[0]
        .item
        .as_audiobookshelf()
        .expect("surviving slot must be an Audiobookshelf item");
    assert_eq!(ep.library_item_id, "show-a");
    assert_eq!(
        ep.position_ticks, acknowledged_position_ticks,
        "adopted slot must carry the last-acknowledged position"
    );

    // Browse reconcile: simulate the progress event the daemon sends when a
    // client attaches (Decision-2 apply path).
    let generation = app.audiobookshelf_runtime.generation();
    app.handle_player_event(mbv_ctrl::player::PlayerEvent::AudiobookshelfProgress(
        mbv_ctrl::AudiobookshelfProgressEvent {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            position_ticks: acknowledged_position_ticks,
            is_finished: false,
            setup_generation: generation.value(),
        },
    ));

    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(
        (progress.current_time_seconds - 30.0).abs() < f64::EPSILON,
        "browse must reflect the adopted acknowledged position"
    );
    assert!(!progress.is_finished);
}
