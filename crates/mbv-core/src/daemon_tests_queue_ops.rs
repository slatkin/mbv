// Direct `handle_ctrl` coverage for the unified-queue mutation commands
// (Append / MoveSlot / RemoveSlot branches / Clear). These arms mutate the
// daemon's canonical queue and mirror the change to the player; the tests
// assert both the resulting canonical order and the forwarded PlayerCommand.

fn queue_op_client(token: &str) -> Arc<Mutex<crate::api::EmbyClient>> {
    let mut client = crate::api::EmbyClient::new(Config::default());
    client.token = token.to_string();
    Arc::new(Mutex::new(client))
}

fn run_queue_cmd(
    cmd: CtrlCmd,
    client_id: u64,
    reply_tx: &mpsc::Sender<CtrlOutbound>,
    client: &Arc<Mutex<crate::api::EmbyClient>>,
    player: &Player,
    owner: &mut DaemonPlayerOwner,
    registry: &Arc<Mutex<CtrlClients>>,
) {
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    handle_ctrl(
        cmd,
        client_id,
        CtrlRequest { reply_tx },
        client,
        player,
        false,
        owner,
        &shared_queue_state(),
        registry,
        false,
        &merged_tx,
        false,
    );
}

/// Same as [`run_queue_cmd`], but with an explicit shared queue snapshot so a
/// test can seed and then assert on `observed_active_slot`.
fn run_queue_cmd_with_shared(
    cmd: CtrlCmd,
    client_id: u64,
    reply_tx: &mpsc::Sender<CtrlOutbound>,
    client: &Arc<Mutex<crate::api::EmbyClient>>,
    player: &Player,
    owner: &mut DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
    registry: &Arc<Mutex<CtrlClients>>,
) {
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    handle_ctrl(
        cmd,
        client_id,
        CtrlRequest { reply_tx },
        client,
        player,
        false,
        owner,
        shared_queue,
        registry,
        false,
        &merged_tx,
        false,
    );
}

fn owner_with(items: Vec<QueueItem>, active: usize) -> DaemonPlayerOwner {
    DaemonPlayerOwner {
        core: PlayerOwnerState::new(
            PlaybackQueue::from_queue_items(items, Some(active)),
            QueueSource::Unknown,
        ),
        ..Default::default()
    }
}

#[test]
fn unified_queue_append_adds_slots_and_forwards_to_player() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    run_queue_cmd(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![
                emby_qi("b", "Video", "Movie"),
                emby_qi("c", "Video", "Movie"),
            ],
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    let queue = owner.core.queue;
    assert_eq!(
        queue.slots().iter().map(|s| s.item.id()).collect::<Vec<_>>(),
        vec!["a", "b", "c"],
    );
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueAppend { items } => {
            let ids: Vec<_> = items.iter().map(|slot| slot.item.id().to_string()).collect();
            assert_eq!(ids, vec!["b", "c"]);
            // The daemon allocates the canonical slot ids and hands the same
            // ids to the player run.
            let canonical: Vec<_> = queue.slots()[1..].iter().map(|s| s.slot_id).collect();
            assert_eq!(items.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(), canonical);
        }
        _ => panic!("expected QueueAppend"),
    }
}

#[test]
fn unified_queue_append_rejects_when_nothing_is_admissible() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    // No Emby token => the daemon cannot admit an Emby item.
    let client = queue_op_client("");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    run_queue_cmd(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![emby_qi("b", "Video", "Movie")],
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 1, "canonical queue is untouched");
    assert!(cmd_rx.try_recv().is_err(), "no player command on rejection");
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "Playback owner rejected the queue append");
        }
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn unified_queue_move_slot_reorders_canonically_and_forwards_to_player() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
        ],
        0,
    );
    let c_slot = owner.core.queue.slots()[2].slot_id;
    run_queue_cmd(
        CtrlCmd::UnifiedQueueMoveSlot {
            slot_id: crate::ctrl::slot_id_to_u64(c_slot),
            to_index: 0,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["c", "a", "b"],
    );
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueMove(sid, to) => {
            assert_eq!(sid, c_slot);
            assert_eq!(to, 0);
        }
        _ => panic!("expected QueueMove"),
    }
}

#[test]
fn unified_queue_move_unknown_slot_is_rejected_without_mutation() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    run_queue_cmd(
        CtrlCmd::UnifiedQueueMoveSlot {
            slot_id: 999_999,
            to_index: 0,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 1);
    assert!(cmd_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "slot not found; move skipped");
        }
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn unified_queue_remove_unknown_slot_is_rejected() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id: 999_999 },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 1);
    assert!(cmd_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "slot not found; remove skipped");
        }
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn unified_queue_remove_non_active_slot_keeps_active_and_forwards_removal() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        0,
    );
    let active = owner.core.queue.active_slot_id();
    let b_slot = owner.core.queue.slots()[1].slot_id;
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: crate::ctrl::slot_id_to_u64(b_slot),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["a"],
    );
    assert_eq!(owner.core.queue.active_slot_id(), active, "active slot unchanged");
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueRemove(sid) => assert_eq!(sid, b_slot),
        _ => panic!("expected QueueRemove"),
    }
}

#[test]
fn unified_queue_remove_slots_applies_the_range_and_publishes_one_snapshot() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
        ],
        0,
    );
    let active = owner.core.queue.active_slot_id();
    let b_slot = owner.core.queue.slots()[1].slot_id;
    let c_slot = owner.core.queue.slots()[2].slot_id;
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlots {
            slot_ids: vec![
                crate::ctrl::slot_id_to_u64(b_slot),
                crate::ctrl::slot_id_to_u64(c_slot),
            ],
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["a"],
    );
    assert_eq!(owner.core.queue.active_slot_id(), active, "active slot unchanged");
    // The whole range is one published snapshot, not one per removed slot.
    match recv_event(&client_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert_eq!(state.slots.len(), 1),
        _ => panic!("expected UnifiedQueueState"),
    }
    assert!(client_rx.try_recv().is_err(), "no second snapshot");
    // The player run still receives one command per removed slot.
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueRemove(sid) => assert_eq!(sid, b_slot),
        _ => panic!("expected QueueRemove"),
    }
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueRemove(sid) => assert_eq!(sid, c_slot),
        _ => panic!("expected QueueRemove"),
    }
}

#[test]
fn unified_queue_remove_slots_skips_unknown_ids_and_no_ops_when_empty() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    let a_slot = owner.core.queue.slots()[0].slot_id;
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlots {
            slot_ids: vec![999_999],
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );
    assert_eq!(owner.core.queue.len(), 1);
    assert!(cmd_rx.try_recv().is_err());
    assert!(client_rx.try_recv().is_err(), "no snapshot for a no-op batch");

    // An active slot in the batch is removed with the rest.
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlots {
            slot_ids: vec![
                crate::ctrl::slot_id_to_u64(a_slot),
                999_999,
            ],
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );
    assert!(owner.core.queue.is_empty());
    match recv_event(&client_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert!(state.slots.is_empty()),
        _ => panic!("expected UnifiedQueueState"),
    }
    match cmd_rx.recv().unwrap() {
        PlayerCommand::SubmitQueue { items, .. } => assert!(items.is_empty()),
        _ => panic!("expected SubmitQueue for the emptied queue"),
    }
}

#[test]
fn unified_queue_remove_active_slot_keeps_queue_when_others_remain() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        0,
    );
    let a_slot = owner.core.queue.slots()[0].slot_id;
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: crate::ctrl::slot_id_to_u64(a_slot),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["b"],
    );
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueRemove(sid) => assert_eq!(sid, a_slot),
        _ => panic!("expected QueueRemove"),
    }
}

#[test]
fn unified_queue_remove_last_slot_clears_the_player_queue() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    let a_slot = owner.core.queue.slots()[0].slot_id;
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: crate::ctrl::slot_id_to_u64(a_slot),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert!(owner.core.queue.is_empty());
    match cmd_rx.recv().unwrap() {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert!(items.is_empty());
            assert_eq!(start_idx, 0);
        }
        _ => panic!("expected empty SubmitQueue"),
    }
}

#[test]
fn unified_queue_clear_empties_canonical_queue_and_clears_the_player() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        0,
    );
    run_queue_cmd(
        CtrlCmd::UnifiedQueueClear,
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert!(owner.core.queue.is_empty());
    assert_eq!(owner.core.source, QueueSource::Unknown);
    match cmd_rx.recv().unwrap() {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert!(items.is_empty());
            assert_eq!(start_idx, 0);
        }
        _ => panic!("expected empty SubmitQueue"),
    }
}

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
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &merged_tx,
        true,
        crate::daemon::DaemonRole::Packaged,
    );

    assert_eq!(owner.core.queue.slots()[0].slot_id, original_slot);
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert!(matches!(commands.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 19,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("only by the Stay-alive owner")));

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: QueueSource::Album,
            lineage: crate::ctrl::QueueLineage(3),
        },
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &merged_tx,
        true,
        crate::daemon::DaemonRole::Packaged,
    );
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::CommandRejected(reason)
        if reason.contains("only by the Stay-alive owner")));
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
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &merged_tx,
        true,
        crate::daemon::DaemonRole::Local,
    );

    assert_eq!(owner.core.queue.slots()[0].slot_id, original_slot);
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert!(player.status.lock().unwrap().active, "rejection leaves the old run active");
    assert!(matches!(commands.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 21,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("did not negotiate")));
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

    handle_ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 51,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue,
        &registry,
        false,
        &merged_tx,
        true,
    );

    assert!(owner.core.queue.is_empty());
    assert_eq!(owner.core.source, QueueSource::Album);
    assert_eq!(player.status.lock().unwrap().sequence_generation, original_generation + 1);
    assert!(matches!(commands.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(matches!(recv_event(&client_rx), CtrlEvent::UnifiedQueueState(state)
        if state.slots.is_empty() && state.active_slot.is_none() && !state.status.active));
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 51,
        result: crate::ctrl::QueueLoadResult::Accepted,
    }));
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
    let old_run = (0, player.status.lock().unwrap().sequence_generation);

    handle_ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 52,
            slots: vec![crate::ctrl::UnifiedQueueSlot {
                slot_id: crate::ctrl::slot_id_to_u64(old_slot),
                item: emby_qi("new", "Video", "Movie"),
            }],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue,
        &registry,
        false,
        &merged_tx,
        true,
    );

    assert_eq!(owner.core.queue.slots()[0].slot_id, old_slot);
    assert!(owner.pending_idle_load.is_some());
    assert!(matches!(reply_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(matches!(commands.try_recv(), Err(mpsc::TryRecvError::Empty)));

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
    assert!(!crate::daemon::playback_run_identity_is_current(old_run, &player));
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
    assert_eq!(owner.core.queue.slots()[0].slot_id, old_slot, "replacement reuses the old slot id");
    assert_eq!(owner.core.queue.slots()[0].item.playback_position_ticks(), 0);
    assert!(!owner.core.queue.slots()[0].item.played());
    assert!(matches!(recv_event(&client_rx), CtrlEvent::UnifiedQueueState(state)
        if state.slots.len() == 1 && state.slots[0].item.id() == "new"
            && state.active_slot.is_none() && !state.status.active));
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
    assert!(client_rx.try_recv().is_err(), "committing stop is not rebroadcast raw");
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 52,
        result: crate::ctrl::QueueLoadResult::Accepted,
    }));
    assert!(matches!(commands.try_recv(), Err(mpsc::TryRecvError::Empty)));
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
    let slots = |id: &str| vec![crate::ctrl::UnifiedQueueSlot {
        slot_id: 901,
        item: emby_qi(id, "Video", "Movie"),
    }];

    for (request_id, id) in [(53, "first"), (54, "second")] {
        handle_ctrl(
            CtrlCmd::UnifiedQueueLoadIdle {
                request_id,
                slots: slots(id),
                cursor: 0,
                source: QueueSource::Album,
            },
            client_id,
            CtrlRequest { reply_tx: &reply_tx },
            &client,
            &player,
            false,
            &mut owner,
            &shared_queue,
            &registry,
            false,
            &merged_tx,
            true,
        );
    }

    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert_eq!(owner.pending_idle_load.as_ref().unwrap().slots[0].1.id(), "first");
    handle_ctrl(
        CtrlCmd::UnifiedQueueClear,
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue,
        &registry,
        false,
        &merged_tx,
        true,
    );
    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 54,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("another idle queue load is pending")));
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::CommandRejected(reason)
        if reason.contains("finalizing an idle queue load")));
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

    handle_ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 56,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &merged_tx,
        true,
    );
    let deadline = owner.pending_idle_load.as_ref().unwrap().started_at;
    assert!(crate::daemon::expire_pending_idle_queue_load(
        &mut owner,
        deadline + Duration::from_secs(31),
    ));
    assert!(owner.pending_idle_load.is_none());
    assert_eq!(owner.core.queue.slots()[0].item.id(), "old");
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 56,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("timed out")));
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

    handle_ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 57,
            slots: vec![],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &merged_tx,
        true,
    );
    player.status.lock().unwrap().sequence_generation += 1;
    handle_ctrl(
        CtrlCmd::RequestShutdown,
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &merged_tx,
        true,
    );
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 57,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason.contains("run changed")));
    assert!(owner.pending_idle_load.is_none());
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::ShutdownRejected { .. }));
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

    handle_ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 55,
            slots: vec![crate::ctrl::UnifiedQueueSlot { slot_id: 902, item: emby_qi("new", "Video", "Movie") }],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        CtrlRequest { reply_tx: &reply_tx },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue,
        &registry,
        false,
        &merged_tx,
        true,
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
    assert!(matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
        request_id: 55,
        result: crate::ctrl::QueueLoadResult::Rejected { reason },
    } if reason == "stop finalization failed"));
}

#[test]
fn unified_queue_replace_publishes_the_start_slot_as_active() {
    let player = cold_player();
    let _cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);

    run_queue_cmd_with_shared(
        CtrlCmd::UnifiedQueueReplace {
            items: vec![],
            slots: vec![
                crate::ctrl::UnifiedQueueSlot {
                    slot_id: 11,
                    item: emby_qi("a", "Video", "Movie"),
                },
                crate::ctrl::UnifiedQueueSlot {
                    slot_id: 22,
                    item: emby_qi("b", "Video", "Movie"),
                },
            ],
            start_idx: Some(1),
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
    assert_eq!(owner.core.source, QueueSource::Album);

    // The publish the client receives must already name the new queue's start
    // slot: publishing before the submit handed a cold daemon's client a queue
    // with no active slot at all, and its now-playing projection then fell back
    // to a stale index — the queue's first row — until the next broadcast.
    match recv_event(&client_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(state.active_slot, Some(22));
            assert_eq!(state.status.current_idx, 1);
        }
        _ => panic!("expected a queue-state publish"),
    }
}

#[test]
fn unified_queue_replace_clears_observed_active_slot() {
    let player = cold_player();
    let _cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        0,
    );
    // A genuine playback observation advances the observed active slot on the
    // owner and is mirrored into the shared snapshot (daemon_run precedent).
    let old_slot = owner.core.queue.slots()[1].slot_id;
    assert_eq!(
        owner.core.observe_track_change(old_slot),
        Some((1, old_slot))
    );
    *shared_queue.observed_active_slot.lock().unwrap() = owner.core.observed_active_slot();
    assert_eq!(owner.core.observed_active_slot(), Some(old_slot));

    run_queue_cmd_with_shared(
        CtrlCmd::UnifiedQueueReplace {
            items: vec![],
            slots: vec![crate::ctrl::UnifiedQueueSlot {
                slot_id: 77,
                item: emby_qi("c", "Video", "Movie"),
            }],
            start_idx: Some(0),
            source: QueueSource::Unknown,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    // The replacement queue is in place and the stale observation is gone
    // from both the owner core and the shared snapshot (design D2): a
    // momentary None is correct; navigation falls back to the new queue's
    // active slot until the first TrackChanged arrives.
    assert_eq!(owner.core.queue.len(), 1);
    assert_eq!(owner.core.queue.slots()[0].slot_id.raw(), 77);
    assert_eq!(owner.core.queue.active_slot_id().map(|s| s.raw()), Some(77));
    assert_eq!(owner.core.observed_active_slot(), None);
    assert_eq!(*shared_queue.observed_active_slot.lock().unwrap(), None);
    // The spy receiver stays attached so the submit path's cold-start thread
    // targets the test player, mirroring `replace_queue_succeeds_unconditionally`.
}
