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
            let ids: Vec<_> = items.iter().map(|(_, item)| item.id().to_string()).collect();
            assert_eq!(ids, vec!["b", "c"]);
            // The daemon allocates the canonical slot ids and hands the same
            // ids to the player run.
            let canonical: Vec<_> = queue.slots()[1..].iter().map(|s| s.slot_id).collect();
            assert_eq!(items.iter().map(|(id, _)| *id).collect::<Vec<_>>(), canonical);
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
    match cmd_rx.recv().unwrap() {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert!(items.is_empty());
            assert_eq!(start_idx, 0);
        }
        _ => panic!("expected empty SubmitQueue"),
    }
}
