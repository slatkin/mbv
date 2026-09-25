// Idle queue loads validate peers and commit only after playback stops.

use super::*;

#[test]
fn packaged_role_rejects_idle_queue_load_without_staging_it() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("kept", "Video", "Movie")], 0);
    let original_slot = owner.core.queue.slots()[0].slot_id;

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 19,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
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
            stay_alive: true,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );

    assert_eq!(owner.core.queue.slots()[0].slot_id, original_slot);
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 19,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("only by the Stay-alive owner"))
    );

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: QueueSource::Album,
            lineage: crate::ctrl::QueueLineage(3),
        },
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
            stay_alive: true,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::CommandRejected(reason)
        if reason.contains("only by the Stay-alive owner"))
    );
}

#[test]
fn idle_queue_load_from_unsupported_peer_is_rejected_without_mutation() {
    let player = cold_player();
    player.status.lock().unwrap().active = true;
    let commands = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (reply_tx, reply_rx) = mpsc::channel();
    let client_id = registry.lock().unwrap().connect(
        reply_tx.clone(),
        CtrlTransport::Local,
        true,
        true,
        true,
        true,
        false,
    );
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("kept", "Video", "Movie")], 0);
    let original_slot = owner.core.queue.slots()[0].slot_id;

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 21,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
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
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );

    assert_eq!(owner.core.queue.slots()[0].slot_id, original_slot);
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert!(
        player.status.lock().unwrap().active,
        "rejection leaves the old run active"
    );
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 21,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("did not negotiate"))
    );
}

#[test]
fn idle_queue_load_without_active_run_publishes_one_stopped_snapshot_and_accepts_empty() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);
    let original_generation = player.status.lock().unwrap().sequence_generation;

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 51,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );

    assert!(owner.core.queue.is_empty());
    assert_eq!(owner.core.source, QueueSource::Album);
    assert_eq!(
        player.status.lock().unwrap().sequence_generation,
        original_generation + 1
    );
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(
        matches!(recv_event(&client_rx), CtrlEvent::UnifiedQueueState(state)
        if state.slots.is_empty() && state.active_slot.is_none() && !state.status.active)
    );
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 51,
            result: crate::ctrl::QueueLoadResult::Accepted,
        }
    ));
}

#[test]
fn pending_idle_load_keeps_old_queue_until_stop_then_commits_once_and_invalidates_old_run() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    player.status.lock().unwrap().active = true;
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);
    let old_slot = owner.core.queue.slots()[0].slot_id;
    let old_run = player.status.lock().unwrap().sequence_generation;

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 52,
            slots: vec![crate::ctrl::UnifiedQueueSlot {
                slot_id: crate::ctrl::slot_id_to_u64(old_slot),
                item: emby_qi("new", "Video", "Movie"),
            }],
            cursor: 0,
            source: QueueSource::Album,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );

    assert_eq!(owner.core.queue.slots()[0].slot_id, old_slot);
    assert!(owner.pending_idle_load.is_some());
    assert!(matches!(
        reply_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    assert!(crate::daemon::complete_pending_idle_queue_load(
        old_run,
        None,
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    ));
    assert_eq!(owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(owner.core.source, QueueSource::Album);
    assert!(owner.core.observed_active_slot().is_none());
    assert!(!player.status.lock().unwrap().active);
    assert!(!crate::daemon::playback_run_identity_is_current(
        old_run, &player
    ));
    assert_eq!(
        crate::daemon::apply_stopped_observation(
            &mut owner,
            &player,
            old_run,
            Some(old_slot),
            99_000_000,
            false,
        ),
        None,
        "late old-run observations are ignored after replacement",
    );
    assert_eq!(owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(
        owner.core.queue.slots()[0].slot_id,
        old_slot,
        "replacement reuses the old slot id"
    );
    assert_eq!(
        owner.core.queue.slots()[0].item.playback_position_ticks(),
        0
    );
    assert!(!owner.core.queue.slots()[0].item.played());
    assert!(
        matches!(recv_event(&client_rx), CtrlEvent::UnifiedQueueState(state)
        if state.slots.len() == 1 && state.slots[0].item.id() == "new"
            && state.active_slot.is_none() && !state.status.active)
    );
    crate::daemon::broadcast_player_event_if_not_replaced(
        &registry,
        PlayerEvent::Stopped {
            slot_id: Some(old_slot),
            run_identity: old_run,
            position_ticks: 99_000_000,
            played: true,
            consume: true,
            progress_report_accepted: false,
            error: None,
        },
        true,
    );
    assert!(
        client_rx.try_recv().is_err(),
        "committing stop is not rebroadcast raw"
    );
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 52,
            result: crate::ctrl::QueueLoadResult::Accepted,
        }
    ));
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn second_idle_load_is_rejected_busy_without_replacing_pending_or_old_queue() {
    let player = cold_player();
    player.status.lock().unwrap().active = true;
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);
    let slots = |id: &str| {
        vec![crate::ctrl::UnifiedQueueSlot {
            slot_id: 901,
            item: emby_qi(id, "Video", "Movie"),
        }]
    };

    for (request_id, id) in [(53, "first"), (54, "second")] {
        handle_ctrl_for_role(
            CtrlCmd::UnifiedQueueLoadIdle {
                request_id,
                slots: slots(id),
                cursor: 0,
                source: QueueSource::Album,
            },
            CtrlContext {
                reply_tx: &reply_tx,
                client_id,
                client: &client,
                player: &player,
                audio_only: false,
                owner: &mut owner,
                shared_queue: &shared_queue,
                ctrl_clients: &registry,
                has_audiobookshelf: false,
                merged_tx: &merged_tx,
                stay_alive: true,
                role: crate::daemon::DaemonRole::Local,
            },
        );
    }

    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert_eq!(
        owner.pending_idle_load.as_ref().unwrap().slots[0].1.id(),
        "first"
    );
    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueClear,
        CtrlContext {
            reply_tx: &reply_tx,
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 54,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("another idle queue load is pending"))
    );
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::CommandRejected(reason)
        if reason.contains("finalizing an idle queue load"))
    );
}

#[test]
fn pending_idle_load_times_out_and_rejects_without_replacing_old_queue() {
    let player = cold_player();
    player.status.lock().unwrap().active = true;
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 56,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
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
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    let deadline = owner.pending_idle_load.as_ref().unwrap().started_at;
    assert!(crate::daemon::expire_pending_idle_queue_load(
        &mut owner,
        deadline + Duration::from_secs(31),
    ));
    assert!(owner.pending_idle_load.is_none());
    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 56,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("timed out"))
    );
}

#[test]
fn pending_idle_load_does_not_block_request_shutdown() {
    let player = cold_player();
    player.status.lock().unwrap().active = true;
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 57,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
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
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    player.status.lock().unwrap().sequence_generation += 1;
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
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 57,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("run changed"))
    );
    assert!(owner.pending_idle_load.is_none());
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::ShutdownRejected { .. }
    ));
}

#[test]
fn failed_stop_finalization_rejects_load_and_keeps_old_queue_and_source() {
    let player = cold_player();
    player.status.lock().unwrap().active = true;
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);
    let old_slot = owner.core.queue.slots()[0].slot_id;

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 55,
            slots: vec![crate::ctrl::UnifiedQueueSlot {
                slot_id: 902,
                item: emby_qi("new", "Video", "Movie"),
            }],
            cursor: 0,
            source: QueueSource::Album,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: true,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    let old_run = owner.pending_idle_load.as_ref().unwrap().stopped_run;

    assert!(crate::daemon::complete_pending_idle_queue_load(
        old_run,
        Some("stop finalization failed".to_string()),
        &mut owner,
        &player,
        &shared_queue,
        &registry,
    ));
    assert_eq!(owner.core.queue.slots()[0].slot_id, old_slot);
    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 55,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason == "stop finalization failed")
    );
}
