// Queue replacements and source updates publish lineage-consistent snapshots.

use super::*;

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

#[test]
fn matching_source_update_publishes_without_replacing_queue_or_playback() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 71,
            slots: vec![crate::ctrl::UnifiedQueueSlot {
                slot_id: 81,
                item: emby_qi("loaded", "Video", "Movie"),
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
    assert!(matches!(
        recv_event(&client_rx),
        CtrlEvent::UnifiedQueueState(_)
    ));
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 71,
            result: crate::ctrl::QueueLoadResult::Accepted,
        }
    ));
    let lineage = *shared_queue.lineage.lock().unwrap();
    let slots_before: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    player.status.lock().unwrap().active = true;
    let status_before = {
        let status = player.status.lock().unwrap();
        (
            status.active,
            status.sequence_generation,
            status.current_idx,
            status.queue_len,
        )
    };

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: QueueSource::Playlist {
                id: Some("playlist-1".to_string()),
                name: "Saved playlist".to_string(),
            },
            lineage,
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

    let CtrlEvent::UnifiedQueueState(snapshot) = recv_event(&client_rx) else {
        panic!("source-only update must publish the owner snapshot");
    };
    assert_eq!(*shared_queue.lineage.lock().unwrap(), lineage);
    assert_eq!(snapshot.lineage, lineage);
    assert_eq!(snapshot.source, owner.core.source);
    assert_eq!(
        snapshot.source,
        QueueSource::Playlist {
            id: Some("playlist-1".to_string()),
            name: "Saved playlist".to_string(),
        }
    );
    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|slot| (slot.slot_id, slot.item.id().to_string()))
            .collect::<Vec<_>>(),
        slots_before,
    );
    let status = player.status.lock().unwrap();
    assert_eq!(
        (
            status.active,
            status.sequence_generation,
            status.current_idx,
            status.queue_len
        ),
        status_before,
    );
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn delayed_source_update_is_rejected_after_another_client_replaces_queue() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_a, rx_a) = connect_client(&mut registry.lock().unwrap());
    let (client_b, rx_b) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let mut owner = owner_with(vec![emby_qi("old", "Video", "Movie")], 0);

    for (request_id, client_id, item_id, playlist_name) in [
        (72, client_a, "first", "First"),
        (73, client_b, "second", "Second"),
    ] {
        handle_ctrl_for_role(
            CtrlCmd::UnifiedQueueLoadIdle {
                request_id,
                slots: vec![crate::ctrl::UnifiedQueueSlot {
                    slot_id: request_id,
                    item: emby_qi(item_id, "Video", "Movie"),
                }],
                cursor: 0,
                source: QueueSource::Playlist {
                    id: Some(request_id.to_string()),
                    name: playlist_name.to_string(),
                },
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
        assert!(
            matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueLoadResult {
            request_id: got,
            result: crate::ctrl::QueueLoadResult::Accepted,
        } if got == request_id)
        );
    }
    let stale_lineage = crate::ctrl::QueueLineage(1);
    assert_eq!(
        *shared_queue.lineage.lock().unwrap(),
        crate::ctrl::QueueLineage(2)
    );
    // Drain both replacement broadcasts before asserting the rejection result.
    for rx in [&rx_a, &rx_b] {
        let _ = recv_event(rx);
        let _ = recv_event(rx);
    }
    let slots_before: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    let source_before = owner.core.source.clone();

    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: QueueSource::Playlist {
                id: Some("stale".to_string()),
                name: "Stale save".to_string(),
            },
            lineage: stale_lineage,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: client_a,
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

    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::CommandRejected(reason)
        if reason.contains("lineage changed"))
    );
    assert!(
        matches!(recv_event(&reply_rx), CtrlEvent::UnifiedQueueState(snapshot)
        if snapshot.lineage == crate::ctrl::QueueLineage(2) && snapshot.source == source_before)
    );
    assert_eq!(owner.core.source, source_before);
    assert_eq!(
        *shared_queue.lineage.lock().unwrap(),
        crate::ctrl::QueueLineage(2)
    );
    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|slot| (slot.slot_id, slot.item.id().to_string()))
            .collect::<Vec<_>>(),
        slots_before,
    );
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}
