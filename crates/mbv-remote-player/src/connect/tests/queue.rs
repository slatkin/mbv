use super::*;

#[test]
fn unsupported_idle_queue_load_is_rejected_without_staging_local_queue() {
    let (remote, _events, commands) =
        RemotePlayer::stub_with_command_rx(vec![make_media_item("confirmed")], 0);
    let result = remote.load_queue_idle(9, vec![], 0, QueueSource::Album);
    assert!(result.is_err());
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(remote.status.lock().unwrap().queue_len, 1);
    let queue = remote
        .unified_queue_state()
        .expect("confirmed queue projection");
    assert!(matches!(
        &queue.slots[0].item,
        QueueItem::Emby(item) if item.id == "confirmed"
    ));
}

#[test]
fn supported_idle_queue_load_sends_correlated_request_without_staging_queue() {
    let (mut remote, _events, commands) =
        RemotePlayer::stub_with_command_rx(vec![make_media_item("confirmed")], 0);
    remote.ctrl_compatibility.supports_owner_queue_load = true;
    remote
        .load_queue_idle(31, vec![], 0, QueueSource::Album)
        .unwrap();
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedQueueLoadIdle { request_id: 31, .. })
    ));
    assert_eq!(remote.status.lock().unwrap().queue_len, 1);
    let queue = remote
        .unified_queue_state()
        .expect("confirmed queue projection");
    assert!(matches!(
        &queue.slots[0].item,
        QueueItem::Emby(item) if item.id == "confirmed"
    ));
}

#[test]
fn answered_queue_op_is_correlated_and_sent_on_the_wire() {
    let (mut remote, _events, commands) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.ctrl_compatibility.supports_answered_queue_ops = true;
    let op = remote
        .send_queue_op(QueueOp::RemoveSlot { slot_id: 17 })
        .unwrap();
    let Some(expected) = op else {
        panic!("current peer advertises answered queue operations");
    };
    assert_eq!(expected.0, 1);
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: 17,
            op: Some(id)
        }) if id == expected
    ));
}

#[test]
fn legacy_queue_op_sends_without_an_operation_id() {
    let (mut remote, _events, commands) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.ctrl_compatibility.supports_answered_queue_ops = false;
    assert_eq!(
        remote
            .send_queue_op(QueueOp::RemoveSlot { slot_id: 17 })
            .unwrap(),
        None
    );
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: 17,
            op: None
        })
    ));
}

// Regression for 2b22f7f42: Applied is the client's only post-op queue snapshot.
#[test]
fn inbound_queue_op_result_follows_prior_state_event() {
    use mbv_ctrl::UnifiedQueueSlot;

    let (remote, events, mut daemon) = connected_pair_for_disconnect_test();
    let slots = vec![
        UnifiedQueueSlot {
            slot_id: 10,
            item: QueueItem::Emby(Box::new(make_media_item("e0"))),
        },
        UnifiedQueueSlot {
            slot_id: 20,
            item: QueueItem::Emby(Box::new(make_media_item("e1"))),
        },
        UnifiedQueueSlot {
            slot_id: 30,
            item: QueueItem::Emby(Box::new(make_media_item("e2"))),
        },
    ];
    let mut state = UnifiedQueueStateData {
        status: status_with_idx_and_len(2, 3),
        slots,
        active_slot: Some(30),
        revision: 3,
        source: QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    let mut earlier_state = state.clone();
    earlier_state.revision = 2;
    writeln!(
        daemon,
        "{}",
        serde_json::to_string(&CtrlEvent::UnifiedQueueState(earlier_state)).unwrap()
    )
    .unwrap();
    state.slots.remove(0);
    let mut stale_status = state.status.clone();
    stale_status.current_idx = 2;
    stale_status.queue_len = 3;
    state.status = stale_status;
    writeln!(
        daemon,
        "{}",
        serde_json::to_string(&CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(7),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(Box::new(state)),
        })
        .unwrap()
    )
    .unwrap();

    assert!(matches!(
        events.recv().unwrap(),
        PlayerEvent::UnifiedQueueUpdated(_)
    ));
    assert!(matches!(
        events.recv().unwrap(),
        PlayerEvent::UnifiedQueueUpdated(_)
    ));
    assert!(matches!(
        events.recv().unwrap(),
        PlayerEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(7),
            ..
        }
    ));
    let status = remote.status.lock().unwrap();
    assert_eq!(status.queue_len, 2);
    assert_eq!(status.current_idx, 1);
    let queue = remote.unified_queue_state().unwrap();
    assert_eq!(queue.revision, 3);
    assert_eq!(queue.slots.len(), 2);
    assert_eq!(queue.active_slot, Some(30));
}

#[test]
fn failed_ctrl_write_marks_remote_disconnected_and_rejects_later_commands() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Read).unwrap();
    assert!(!remote.is_disconnected());
    assert!(remote.send_ctrl_cmd(CtrlCmd::Stop));
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(!remote.send_ctrl_cmd(CtrlCmd::Stop));
}

#[test]
fn failed_writer_write_emits_connection_lost_once() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Read).unwrap();
    assert!(remote.send_ctrl_cmd(CtrlCmd::Stop));

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[test]
fn reader_eof_emits_connection_lost_instead_of_stopped() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Write).unwrap();

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[test]
fn writer_and_reader_loss_emit_only_one_disconnect_event() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Read).unwrap();
    assert!(remote.send_ctrl_cmd(CtrlCmd::Stop));
    daemon.shutdown(Shutdown::Write).unwrap();

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn announced_shutdown_emits_only_its_dedicated_event() {
    use std::io::Write;
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, mut daemon) = connected_pair_for_disconnect_test();
    let shutdown = CtrlEvent::Disconnected {
        reason: DisconnectReason::DaemonShutdown,
    };
    writeln!(daemon, "{}", serde_json::to_string(&shutdown).unwrap()).unwrap();
    daemon.shutdown(Shutdown::Write).unwrap();

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::DaemonShutdownAnnounced
    ));
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(remote.is_shutdown_announced());
}

#[test]
fn status_only_preserves_event_confirmed_current_index() {
    let status = Arc::new(Mutex::new(status_with_idx(3)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx(5)),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
}

#[test]
fn status_only_preserves_current_idx_and_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(3, 7)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx_and_len(5, 2)),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 3);
    assert_eq!(s.queue_len, 7);
}

#[test]
fn track_changed_leaves_status_mirror_for_app_to_rederive() {
    // `TrackChanged` now carries a `QueueSlotId`; the RemotePlayer read loop
    // has no queue to resolve it against and no longer mutates the status
    // mirror. `App::handle_player_event` re-derives `current_idx` from its
    // canonical queue instead (client-side mirror removal is Section 4).
    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 5)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::Player(PlayerEvent::TrackChanged {
            slot_id: mbv_queue::QueueSlotId::from_raw(2),
            transition: None,
        }),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 0);
    assert_eq!(s.queue_len, 5);
}

#[test]
fn command_rejected_forwards_reason_as_player_event() {
    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::CommandRejected("daemon is audio-only".to_string()),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    match rx.recv().unwrap() {
        PlayerEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon is audio-only");
        }
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn reconnect_replaces_queue_and_status_from_one_playback_snapshot() {
    use mbv_ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 0)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel();
    let pending_playback = Arc::new(Mutex::new(std::collections::HashMap::new()));

    let reconnect_snapshot = UnifiedQueueStateData {
        status: status_with_idx_and_len(0, 2),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 11,
                item: QueueItem::Emby(Box::new(make_media_item("a"))),
            },
            UnifiedQueueSlot {
                slot_id: 22,
                item: QueueItem::Emby(Box::new(make_media_item("b"))),
            },
        ],
        active_slot: Some(22),
        revision: 9,
        source: QueueSource::Remote,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(reconnect_snapshot),
        &status,
        &unified_queue,
        &tx,
        &pending_playback,
        true,
    );

    let stored = unified_queue.lock().unwrap().clone().unwrap();
    let status = status.lock().unwrap().clone();
    let event = rx.recv().unwrap();
    let PlayerEvent::UnifiedQueueUpdated(event_snapshot) = event else {
        panic!("expected unified queue snapshot");
    };

    assert_eq!(stored.revision, 9);
    assert_eq!(
        stored
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        vec![11, 22]
    );
    assert_eq!(stored.active_slot, Some(22));
    assert_eq!(stored.status.current_idx, 1);
    assert_eq!(stored.status.queue_len, 2);
    assert_eq!(status.current_idx, stored.status.current_idx);
    assert_eq!(status.queue_len, stored.status.queue_len);
    assert_eq!(status.active, stored.status.active);
    assert_eq!(event_snapshot.revision, stored.revision);
    assert_eq!(
        event_snapshot
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        vec![11, 22]
    );
    assert_eq!(event_snapshot.active_slot, stored.active_slot);
    assert_eq!(event_snapshot.status.current_idx, status.current_idx);
}

#[test]
fn unified_queue_state_preserves_canonical_coordinates_and_source() {
    use mbv_ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel();

    // Mixed queue: [Emby(e0), Feed(f1), Emby(e2), Feed(f3)]. The active
    // Feed slot retains its canonical index in the unified snapshot.
    let e0 = make_media_item("e0");
    let e2 = make_media_item("e2");
    let f1 = make_feed_entry("f1");
    let f3 = make_feed_entry("f3");

    let unified = UnifiedQueueStateData {
        status: status_with_idx_and_len(1, 4),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 10,
                item: QueueItem::Emby(Box::new(e0)),
            },
            UnifiedQueueSlot {
                slot_id: 20,
                item: QueueItem::Feed(f1),
            },
            UnifiedQueueSlot {
                slot_id: 30,
                item: QueueItem::Emby(Box::new(e2)),
            },
            UnifiedQueueSlot {
                slot_id: 40,
                item: QueueItem::Feed(f3),
            },
        ],
        active_slot: Some(20), // canonical slot_id for f1
        revision: 1,
        source: QueueSource::Playlist {
            id: Some("pl-1".into()),
            name: "My Playlist".into(),
        },
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(unified),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    // queue_len from canonical slots
    assert_eq!(status.lock().unwrap().queue_len, 4);

    assert_eq!(status.lock().unwrap().current_idx, 1);

    let canonical = unified_queue.lock().unwrap().clone().unwrap();
    assert_eq!(canonical.active_slot, Some(20));
    assert_eq!(canonical.slots[1].slot_id, 20);

    // UnifiedQueueUpdated event emitted with full canonical data
    match rx.recv().unwrap() {
        PlayerEvent::UnifiedQueueUpdated(state) => {
            assert_eq!(state.slots.len(), 4);
            assert_eq!(state.active_slot, Some(20));
            assert_eq!(
                state.source,
                QueueSource::Playlist {
                    id: Some("pl-1".into()),
                    name: "My Playlist".into(),
                }
            );
            // Slot IDs preserved
            assert_eq!(state.slots[0].slot_id, 10);
            assert_eq!(state.slots[1].slot_id, 20);
            assert_eq!(state.slots[2].slot_id, 30);
            assert_eq!(state.slots[3].slot_id, 40);
        }
        _ => panic!("expected UnifiedQueueUpdated"),
    }
}
