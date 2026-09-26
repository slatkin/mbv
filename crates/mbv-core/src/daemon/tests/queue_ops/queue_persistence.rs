// Queue state persists and is taken over across owner restarts.

use super::*;

#[test]
fn packaged_owner_keeps_per_user_queue_persistence_on_shutdown() {
    let _scratch = crate::config::TestTempDir::new().as_xdg_home();
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
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );

    let saved = crate::config::load_queue_state().unwrap();
    assert_eq!(saved.items[0].id(), "packaged");
    assert_eq!(saved.source, QueueSource::Album);
    assert!(!crate::config::stay_alive_queue_state_path().exists());
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::ShutdownAccepted));
    assert!(matches!(merged_rx.try_recv(), Ok(DaemonEvent::Shutdown)));
}

#[test]
fn stay_alive_owner_queue_state_round_trips_queue_source_and_lineage() {
    let temp = crate::config::TestTempDir::new();
    let path = temp.join("stay_alive_queue_state.json");
    let state = crate::config::StayAliveQueueState {
        queue: crate::config::QueueState {
            // Duplicate content still occupies two distinct queue slots.
            items: vec![
                emby_qi("persisted", "Video", "Movie"),
                emby_qi("persisted", "Video", "Movie"),
            ],
            cursor: 1,
            source: QueueSource::Album,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: std::collections::HashMap::default(),
        },
        lineage: crate::ctrl::QueueLineage(42),
    };
    crate::config::save_stay_alive_queue_state_at(&path, &state).unwrap();

    // A daemon restart loads a fresh owner state from the state file.
    let restored = crate::config::load_stay_alive_queue_state_at(&path).unwrap();
    let queue = PlaybackQueue::from_queue_items(restored.queue.items, Some(restored.queue.cursor));
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.slots()[0].item.id(), "persisted");
    assert_eq!(queue.slots()[1].item.id(), "persisted");
    assert_eq!(queue.active_index(), Some(1));
    assert_eq!(restored.queue.source, QueueSource::Album);
    assert_eq!(restored.lineage, crate::ctrl::QueueLineage(42));
}

#[test]
fn stay_alive_owner_takes_over_legacy_snapshot_only_once() {
    let path =
        std::env::temp_dir().join(format!("mbv-takeover-owner-{}.json", uuid::Uuid::new_v4()));
    let legacy = crate::config::QueueState {
        items: vec![emby_qi("legacy", "Video", "Movie")],
        cursor: 0,
        source: QueueSource::Series,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    };
    let takeover = crate::config::legacy_queue_for_owner_if_absent(&path, Some(legacy)).unwrap();
    assert_eq!(takeover.lineage, crate::ctrl::QueueLineage::default());
    crate::config::save_stay_alive_queue_state_at(&path, &takeover).unwrap();
    assert!(crate::config::legacy_queue_for_owner_if_absent(
        &path,
        Some(crate::config::QueueState {
            items: vec![emby_qi("stale", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: std::collections::HashMap::default(),
        }),
    )
    .is_none());
    assert_eq!(
        crate::config::load_stay_alive_queue_state_at(&path)
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
    let temp = crate::config::TestTempDir::new();
    let path = temp.join("stay_alive_queue_state.json");
    let state = crate::config::StayAliveQueueState {
        queue: crate::config::QueueState {
            items: vec![],
            cursor: 0,
            source: QueueSource::Unknown,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: std::collections::HashMap::default(),
        },
        lineage: crate::ctrl::QueueLineage(7),
    };
    crate::config::save_stay_alive_queue_state_at(&path, &state).unwrap();
    let legacy = crate::config::QueueState {
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
    assert!(crate::config::legacy_queue_for_owner_if_absent(&path, Some(legacy)).is_none());

    // Restart still loads the explicitly saved empty queue, not the stale
    // per-user snapshot; repeated startup cannot turn empty into populated.
    for _restart in 0..2 {
        let restored = crate::config::load_stay_alive_queue_state_at(&path).unwrap();
        assert!(restored.queue.items.is_empty());
        assert_eq!(restored.lineage, crate::ctrl::QueueLineage(7));
        assert!(crate::config::legacy_queue_for_owner_if_absent(&path, None).is_none());
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
