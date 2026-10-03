// Queue state persists and is taken over across owner restarts.

use super::*;

#[test]
fn packaged_owner_keeps_per_user_queue_persistence_on_shutdown() {
    let _scratch = mbv_config::TestTempDir::new().as_xdg_home();
    let player = cold_player();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (merged_tx, merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("packaged", "Video", "Movie")], 0);
    owner.core.source = QueueSource::Album;

    handle_ctrl_for_role(
        CtrlCmd::RequestShutdown,
        CtrlContext {
            reply_tx: &reply_tx,
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            audiobookshelf: None,
            merged_tx: &merged_tx,
            owner_settings: crate::owner_settings::fixed_reader(false),
            role: crate::DaemonRole::Packaged,
            op: std::cell::Cell::new(None),
        },
    );

    let saved = mbv_config::load_queue_state().unwrap();
    assert_eq!(saved.items[0].id(), "packaged");
    assert_eq!(saved.source, QueueSource::Album);
    assert!(!mbv_config::stay_alive_queue_state_path().exists());
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::ShutdownAccepted));
    assert!(matches!(merged_rx.try_recv(), Ok(DaemonEvent::Shutdown)));
}

#[test]
fn stay_alive_owner_queue_state_round_trips_queue_source_and_lineage() {
    let temp = mbv_config::TestTempDir::new();
    let path = temp.join("stay_alive_queue_state.json");
    let mut emby_with_position = emby_qi("persisted", "Video", "Movie");
    if let QueueItem::Emby(item) = &mut emby_with_position {
        item.playback_position_ticks = 20 * mbv_emby_model::TICKS_PER_SECOND;
    }
    let mut abs_with_position = abs_qi("library-a", "episode-1");
    if let QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(episode)) =
        &mut abs_with_position
    {
        episode.position_ticks = 5 * mbv_emby_model::TICKS_PER_SECOND;
    }
    let mut feed_with_position = video_feed_qi("feed-1");
    if let QueueItem::Feed(entry) = &mut feed_with_position {
        entry.position_ticks = 42;
    }
    let state = mbv_config::StayAliveQueueState {
        queue: mbv_queue::QueueState {
            // Duplicate content still occupies two distinct queue slots.
            items: vec![
                emby_with_position.clone(),
                emby_with_position,
                abs_with_position,
                feed_with_position,
            ],
            cursor: 1,
            source: QueueSource::Album,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: std::collections::HashMap::default(),
        },
        lineage: mbv_queue::QueueLineage(42),
    };
    mbv_config::save_stay_alive_queue_state_at(&path, &state).unwrap();

    // A daemon restart loads a fresh owner state from the state file. Service
    // positions are cleared on the way out; a Feed entry keeps its local one.
    let restored = mbv_config::load_stay_alive_queue_state_at(&path).unwrap();
    assert_eq!(restored.queue.items[0].playback_position_ticks(), 0);
    assert_eq!(restored.queue.items[1].playback_position_ticks(), 0);
    assert_eq!(restored.queue.items[2].playback_position_ticks(), 0);
    assert_eq!(restored.queue.items[3].playback_position_ticks(), 42);
    let queue = PlaybackQueue::from_queue_items(
        restored.queue.items,
        Some(restored.queue.cursor),
        crate::tests::revision_mint(),
    );
    assert_eq!(queue.len(), 4);
    assert_eq!(queue.slots()[0].item.id(), "persisted");
    assert_eq!(queue.slots()[1].item.id(), "persisted");
    assert_eq!(queue.active_index(), Some(1));
    assert_eq!(restored.queue.source, QueueSource::Album);
    assert_eq!(restored.lineage, mbv_queue::QueueLineage(42));

    // A state file written before this change carries Service positions; the
    // load path clears those too rather than only relying on the save path.
    std::fs::write(&path, serde_json::to_string(&state).unwrap()).unwrap();
    let earlier = mbv_config::load_stay_alive_queue_state_at(&path).unwrap();
    assert_eq!(earlier.queue.items[0].playback_position_ticks(), 0);
    assert_eq!(earlier.queue.items[2].playback_position_ticks(), 0);
    assert_eq!(earlier.queue.items[3].playback_position_ticks(), 42);
}

#[test]
fn stay_alive_owner_takes_over_legacy_snapshot_only_once() {
    let path =
        std::env::temp_dir().join(format!("mbv-takeover-owner-{}.json", uuid::Uuid::new_v4()));
    let legacy = mbv_queue::QueueState {
        items: vec![emby_qi("legacy", "Video", "Movie")],
        cursor: 0,
        source: QueueSource::Series,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    };
    let takeover = mbv_config::legacy_queue_for_owner_if_absent(&path, Some(legacy)).unwrap();
    assert_eq!(takeover.lineage, mbv_queue::QueueLineage::default());
    mbv_config::save_stay_alive_queue_state_at(&path, &takeover).unwrap();
    assert!(
        mbv_config::legacy_queue_for_owner_if_absent(
            &path,
            Some(mbv_queue::QueueState {
                items: vec![emby_qi("stale", "Video", "Movie")],
                cursor: 0,
                source: QueueSource::Unknown,
                last_played_content_id: None,
                last_played_item_id: None,
                last_played_completed: false,
                positions: std::collections::HashMap::default(),
            }),
        )
        .is_none()
    );
    assert_eq!(
        mbv_config::load_stay_alive_queue_state_at(&path)
            .unwrap()
            .queue
            .items[0]
            .id(),
        "legacy"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn stay_alive_empty_owner_state_never_takes_over_legacy_snapshot() {
    let temp = mbv_config::TestTempDir::new();
    let path = temp.join("stay_alive_queue_state.json");
    let state = mbv_config::StayAliveQueueState {
        queue: mbv_queue::QueueState {
            items: vec![],
            cursor: 0,
            source: QueueSource::Unknown,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: std::collections::HashMap::default(),
        },
        lineage: mbv_queue::QueueLineage(7),
    };
    mbv_config::save_stay_alive_queue_state_at(&path, &state).unwrap();
    let legacy = mbv_queue::QueueState {
        items: vec![emby_qi("stale", "Video", "Movie")],
        cursor: 0,
        source: QueueSource::Playlist {
            id: None,
            name: "old".to_string(),
        },
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    };
    assert!(mbv_config::legacy_queue_for_owner_if_absent(&path, Some(legacy)).is_none());

    // Restart still loads the explicitly saved empty queue, not the stale
    // per-user snapshot; repeated startup cannot turn empty into populated.
    for _restart in 0..2 {
        let restored = mbv_config::load_stay_alive_queue_state_at(&path).unwrap();
        assert!(restored.queue.items.is_empty());
        assert_eq!(restored.lineage, mbv_queue::QueueLineage(7));
        assert!(mbv_config::legacy_queue_for_owner_if_absent(&path, None).is_none());
    }
}

#[test]
fn stay_alive_owner_refuses_client_queue_adoption() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("owner", "Video", "Movie")], 0);
    let shared_queue = shared_queue_state();
    let lineage = *shared_queue.lineage.lock().unwrap();
    run_queue_cmd_with_shared(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("client", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );
    assert_eq!(owner.core.queue.slots()[0].item.id(), "owner");
    assert_eq!(*shared_queue.lineage.lock().unwrap(), lineage);
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::CommandRejected(reason)
        if reason.contains("cannot be adopted"))
    );
    commands.try_recv().unwrap_err();
}
