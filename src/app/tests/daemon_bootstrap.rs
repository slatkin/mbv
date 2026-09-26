use super::*;

#[test]
fn remote_app_starts_on_local_queue_when_remote_queue_is_empty() {
    let app = make_remote_app_stub(make_items(2), Vec::new());

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
}

#[test]
fn remote_app_starts_on_remote_queue_when_remote_queue_has_items() {
    let app = make_remote_app_stub(make_items(2), make_items(1));

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
}

// `item_text_and_style` and its dedicated tests above were deleted
// (#361): its only production caller was the deleted Standard
// `render/library/table/context.rs`.

#[test]
fn attaching_to_empty_local_daemon_does_not_restore_or_persist_saved_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let saved = crate::config::QueueState::from_emby_items(
        make_items(5),
        0,
        crate::config::QueueSource::Playlist {
            id: Some("saved".into()),
            name: "Saved snapshot".into(),
        },
    );
    crate::config::save_queue_state(&saved).expect("save queue state");

    let (remote, player_rx, command_rx) =
        mbv_core::remote_player::RemotePlayer::stub_owner_queue_load_with_command_rx(Vec::new(), 0);
    *remote.unified_queue.lock().unwrap() = Some(emby_unified_state(&[], 0));
    let config = crate::config::Config::default();
    let mut app = App::new_remote_with_config(
        mbv_core::api::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_core::remote_player::DaemonEndpoint::Local,
        config,
    );
    assert!(app.player_tab.emby_items().is_empty());
    assert_eq!(app.queue_source, crate::config::QueueSource::Remote);
    app.restore_queue_state();
    assert!(app.player_tab.emby_items().is_empty());

    app.player_tab.set_items(make_items(2), 0);
    app.save_queue_state();
    app.save_queue_state_no_clear();
    let commands: Vec<_> = command_rx.try_iter().collect();
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, mbv_core::ctrl::CtrlCmd::UnifiedAdoptQueue { .. })),
        "Client must not send queue adoption"
    );
    let restored = crate::config::load_queue_state().expect("saved snapshot remains");
    assert_eq!(restored.items.len(), saved.items.len());
    assert_eq!(restored.items[0].content_id(), saved.items[0].content_id());
    assert_eq!(restored.source, saved.source);
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

// Task 4.1: A later capable client adopts the daemon's live Audiobookshelf
// queue (active slot, position) over a stale saved local/shared disk snapshot,
// and reconciles browse state on adoption via the daemon progress event path.
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
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut browse =
        crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(
            library.clone(),
        );
    browse.detail_cache.insert(
        "show-a".into(),
        vec![mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
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
    let acknowledged_position_ticks = (30.0 * mbv_core::api::TICKS_PER_SECOND as f64) as i64;
    let abs_item = mbv_core::playback_queue::QueueItem::Audiobookshelf(
        mbv_core::playback_queue::AudiobookshelfQueueItem {
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
    );
    app.player_tab.set_queue_items(vec![abs_item], 0);
    assert_eq!(app.player_tab.total_queue_len(), 1);

    // Save a stale disk snapshot (5 Emby items) — what a previous session left.
    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: make_items(5)
            .into_iter()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .expect("save stale queue state");

    // The local-daemon guard must prevent the stale snapshot from clobbering
    // the live adopted ABS queue.
    app.restore_queue_state();

    assert_eq!(
        app.player_tab.total_queue_len(),
        1,
        "live ABS queue must survive restore_queue_state"
    );
    let ep = app.player_tab.queue.slots()[0]
        .item
        .as_audiobookshelf()
        .expect("surviving slot must be an Audiobookshelf item");
    assert_eq!(ep.library_item_id, "show-a");
    assert_eq!(
        ep.position_ticks, acknowledged_position_ticks,
        "last-acknowledged position must not be clobbered by the stale snapshot"
    );

    // Browse reconcile: simulate the progress event the daemon sends when a
    // client attaches (Decision-2 apply path).
    let generation = app.audiobookshelf_runtime.generation();
    app.handle_player_event(mbv_core::player::PlayerEvent::AudiobookshelfProgress(
        mbv_core::ctrl::AudiobookshelfProgressEvent {
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

#[test]
fn queue_restore_uses_saved_cursor_when_last_played_is_missing() {
    let items: Vec<mbv_core::playback_queue::QueueItem> = make_items(3)
        .into_iter()
        .map(|i| mbv_core::playback_queue::QueueItem::Emby(Box::new(i)))
        .collect();
    let cursor = crate::app::dispatch::actions::queue_restore_cursor(&items, 2, None, None, false);
    assert_eq!(cursor, 2);
}
