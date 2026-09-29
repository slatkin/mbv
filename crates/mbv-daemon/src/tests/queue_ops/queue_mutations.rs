// Queue mutations preserve canonical order and mirror changes to the player.

use super::*;

fn connect_op_requester(
    clients: &mut CtrlClients,
) -> (
    u64,
    mpsc::Sender<CtrlOutbound>,
    mpsc::Receiver<CtrlOutbound>,
) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(
        tx.clone(),
        CtrlTransport::Local,
        mbv_ctrl::CtrlAudiobookshelfCapabilities {
            queue: true,
            progress: true,
            book_queue: true,
            book_progress: true,
        },
        true,
    );
    (id, tx, rx)
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
            before: None,
            op: None,
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
        queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c"],
    );
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueAppend { items } => {
            let ids: Vec<_> = items
                .iter()
                .map(|slot| slot.item.id().to_string())
                .collect();
            assert_eq!(ids, vec!["b", "c"]);
            // The daemon allocates the canonical slot ids and hands the same
            // ids to the player run.
            let canonical: Vec<_> = queue.slots()[1..].iter().map(|s| s.slot_id).collect();
            assert_eq!(
                items.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
                canonical
            );
        }
        _ => panic!("expected QueueAppend"),
    }
}

#[test]
fn unified_playback_queue_edits_are_answered_before_the_next_input_append_before_keeps_owner_and_player_order()
 {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, reply_tx, reply_rx) = connect_op_requester(&mut registry.lock().unwrap());
    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("anchor", "Video", "Movie"),
        ],
        0,
    );
    let anchor = owner.core.queue.slots()[1].slot_id;

    run_queue_cmd(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![emby_qi("inserted", "Video", "Movie")],
            before: Some(anchor.raw()),
            op: Some(mbv_ctrl::QueueOpId(44)),
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
            .map(|slot| slot.item.id())
            .collect::<Vec<_>>(),
        vec!["a", "inserted", "anchor"],
    );
    let inserted_slot = owner.core.queue.slots()[1].slot_id;
    match cmd_rx.recv().unwrap() {
        PlayerCommand::QueueAppend { items } => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].item.id(), "inserted");
            assert_eq!(items[0].slot_id, inserted_slot);
        }
        _ => panic!("expected QueueAppend before QueueMove"),
    }
    assert!(matches!(
        cmd_rx.recv().unwrap(),
        PlayerCommand::QueueMove(slot_id, 1) if slot_id == inserted_slot
    ));
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(44),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(_),
        }
    ));
}

#[test]
fn unified_playback_queue_edits_are_answered_before_the_next_input_stale_append_anchor_rejects_with_op()
 {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, reply_tx, reply_rx) = connect_op_requester(&mut registry.lock().unwrap());
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);

    run_queue_cmd(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![emby_qi("inserted", "Video", "Movie")],
            before: Some(u64::MAX),
            op: Some(mbv_ctrl::QueueOpId(45)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 1);
    assert_eq!(cmd_rx.try_recv().unwrap_err(), mpsc::TryRecvError::Empty);
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(45),
            outcome: mbv_ctrl::QueueOpOutcome::Rejected(reason),
        } if reason == "slot not found; append anchor is stale"
    ));
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
            before: None,
            op: None,
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
            slot_id: mbv_ctrl::slot_id_to_u64(c_slot),
            to_index: 0,
            op: None,
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
            op: None,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 1);
    cmd_rx.try_recv().unwrap_err();
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
        CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: 999_999,
            op: None,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    assert_eq!(owner.core.queue.len(), 1);
    cmd_rx.try_recv().unwrap_err();
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
            slot_id: mbv_ctrl::slot_id_to_u64(b_slot),
            op: None,
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
    assert_eq!(
        owner.core.queue.active_slot_id(),
        active,
        "active slot unchanged"
    );
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
                mbv_ctrl::slot_id_to_u64(b_slot),
                mbv_ctrl::slot_id_to_u64(c_slot),
            ],
            op: None,
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
    assert_eq!(
        owner.core.queue.active_slot_id(),
        active,
        "active slot unchanged"
    );
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
            op: None,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );
    assert_eq!(owner.core.queue.len(), 1);
    cmd_rx.try_recv().unwrap_err();
    assert!(
        client_rx.try_recv().is_err(),
        "no snapshot for a no-op batch"
    );

    // An active slot in the batch is removed with the rest.
    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlots {
            slot_ids: vec![mbv_ctrl::slot_id_to_u64(a_slot), 999_999],
            op: None,
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
            slot_id: mbv_ctrl::slot_id_to_u64(a_slot),
            op: None,
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
            slot_id: mbv_ctrl::slot_id_to_u64(a_slot),
            op: None,
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
    let shared_queue = shared_queue_state();
    run_queue_cmd_with_shared(
        CtrlCmd::UnifiedQueueClear,
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    assert!(owner.core.queue.is_empty());
    assert_eq!(owner.core.source, QueueSource::Unknown);
    assert_eq!(
        *shared_queue.lineage.lock().unwrap(),
        mbv_queue::QueueLineage(1)
    );
    match cmd_rx.recv().unwrap() {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert!(items.is_empty());
            assert_eq!(start_idx, 0);
        }
        _ => panic!("expected empty SubmitQueue"),
    }
}

#[test]
fn unified_playback_queue_edits_are_answered_before_the_next_input_sender_gets_result_and_peers_get_broadcast()
 {
    let player = cold_player();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, reply_tx, sender_rx) = connect_op_requester(&mut registry.lock().unwrap());
    let (_other_id, other_rx) = connect_client(&mut registry.lock().unwrap());
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);

    run_queue_cmd(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![emby_qi("b", "Video", "Movie")],
            before: None,
            op: Some(mbv_ctrl::QueueOpId(42)),
        },
        sender_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    match recv_event(&sender_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(42),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } => assert_eq!(state.slots.len(), 2),
        _ => panic!("sender should receive its applied queue result"),
    }
    assert!(
        sender_rx.try_recv().is_err(),
        "sender receives no broadcast"
    );
    match recv_event(&other_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert_eq!(state.slots.len(), 2),
        _ => panic!("other client should receive the queue broadcast"),
    }
}

#[test]
fn unified_playback_queue_edits_are_answered_before_the_next_input_empty_set_removal_answers() {
    let player = cold_player();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, reply_tx, sender_rx) = connect_op_requester(&mut registry.lock().unwrap());
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);

    run_queue_cmd(
        CtrlCmd::UnifiedQueueRemoveSlots {
            slot_ids: Vec::new(),
            op: Some(mbv_ctrl::QueueOpId(43)),
        },
        sender_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    match recv_event(&sender_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(43),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } => assert_eq!(state.slots.len(), 1),
        _ => panic!("empty removal should answer with the unchanged queue"),
    }
    assert!(sender_rx.try_recv().is_err());
}

#[test]
fn unified_playback_queue_edits_are_answered_before_the_next_input_legacy_command_broadcasts_to_everyone()
 {
    let player = cold_player();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (sender_id, sender_rx) = connect_client(&mut registry.lock().unwrap());
    let (_other_id, other_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);

    run_queue_cmd(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![emby_qi("b", "Video", "Movie")],
            before: None,
            op: None,
        },
        sender_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    match recv_event(&sender_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert_eq!(state.slots.len(), 2),
        _ => panic!("legacy command should broadcast to the sender"),
    }
    match recv_event(&other_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert_eq!(state.slots.len(), 2),
        _ => panic!("legacy command should broadcast to the other client"),
    }
}
