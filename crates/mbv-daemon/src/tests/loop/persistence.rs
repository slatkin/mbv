// Tests for what the per-pass store records: which loop events commit a
// snapshot and which persist nothing. Split from loop.rs after the shuffle
// Next-Up card test pushed it past the 800-line cap.

use super::*;

#[test]
fn track_completed_stale_run_leaves_queue_and_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation = 5;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let original_position = t.event_loop.owner.core.queue.slots()[0]
        .item
        .playback_position_ticks();

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::TrackCompleted {
            slot_id: slot,
            run_identity: 4,
            position_ticks: 900,
            played: true,
            consume: true,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots().len(), 1);
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[0]
            .item
            .playback_position_ticks(),
        original_position
    );
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn stopped_current_run_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(slot),
            run_identity: current_run(&t.event_loop),
            position_ticks: 900,
            played: false,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(
        t.event_loop
            .owner
            .core
            .queue
            .slot(slot)
            .unwrap()
            .item
            .playback_position_ticks(),
        900
    );
    assert_eq!(t.persisted.borrow().len(), 1);
}

#[test]
fn stopped_stale_run_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation = 5;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(slot),
            run_identity: 4,
            position_ticks: 900,
            played: true,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(
        !t.event_loop
            .owner
            .core
            .queue
            .slot(slot)
            .unwrap()
            .item
            .played()
    );
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn stopped_matching_pending_idle_load_commits_and_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    let old_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let run = current_run(&t.event_loop);
    let (reply_tx, reply_rx) = mpsc::channel();
    t.event_loop.owner.pending_idle_load = Some(PendingIdleQueueLoad {
        client_id: 0,
        request_id: 7,
        slots: vec![(old_slot, emby_qi("new", "Video", "Movie"))],
        cursor: 0,
        source: QueueSource::Album,
        reply_tx,
        stopped_run: run,
        started_at: Instant::now(),
    });

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(old_slot),
            run_identity: run,
            position_ticks: 900,
            played: false,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.event_loop.owner.pending_idle_load.is_none());
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Album);
    assert_eq!(t.persisted.borrow().len(), 1);
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 7,
            result: mbv_ctrl::QueueLoadResult::Accepted,
        }
    ));
}

#[test]
fn stopped_different_run_cancels_pending_idle_load_and_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation = 5;
    let old_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let (reply_tx, reply_rx) = mpsc::channel();
    t.event_loop.owner.pending_idle_load = Some(PendingIdleQueueLoad {
        client_id: 0,
        request_id: 8,
        slots: vec![(old_slot, emby_qi("new", "Video", "Movie"))],
        cursor: 0,
        source: QueueSource::Album,
        reply_tx,
        stopped_run: 5,
        started_at: Instant::now(),
    });

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(old_slot),
            run_identity: 4,
            position_ticks: 900,
            played: false,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.event_loop.owner.pending_idle_load.is_none());
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "old");
    assert!(t.persisted.borrow().is_empty());
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 8,
            result: mbv_ctrl::QueueLoadResult::Rejected { .. },
        }
    ));
}

#[test]
fn ws_matching_generation_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let runtime = EmbyOwnerContext::from_client(EmbyClient::new(Config::default()), 1);
    let generation = runtime.generation;
    t.event_loop.emby_runtime = Some(runtime);

    let flow = t.event_loop.handle_event(DaemonEvent::Ws {
        generation,
        event: WsEvent::Stop,
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.persisted.borrow().len(), 1);
}

#[test]
fn ws_mismatched_generation_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop.emby_runtime = Some(EmbyOwnerContext::from_client(
        EmbyClient::new(Config::default()),
        1,
    ));

    let flow = t.event_loop.handle_event(DaemonEvent::Ws {
        generation: mbv_core::service_runtime::SetupGeneration::new(99),
        event: WsEvent::Stop,
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn role_gate_non_local_dirty_event_persists_nothing() {
    let mut t = test_loop_with_role(crate::DaemonRole::Packaged);
    t.event_loop.client.lock().unwrap().config.consume_audio = true;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::TrackCompleted {
            slot_id: mbv_queue::QueueSlotId::from_raw(1),
            run_identity: current_run(&t.event_loop),
            position_ticks: 0,
            played: true,
            consume: true,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn idle_queue_load_install_persists_once_through_injected_store() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    // A real state dir would receive the install path's removed production
    // write; point it at a tempdir and assert that file never appears.
    let _guard = mbv_config::TestStateDirGuard::new();
    t.event_loop.client.lock().unwrap().token = "test-token".to_string();
    let (client_id, _rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let flow = t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 5,
            slots: vec![mbv_ctrl::UnifiedQueueSlot {
                slot_id: 1,
                item: emby_qi("new", "Video", "Movie"),
            }],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        reply_tx,
    ));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Album);
    // The install path has no inline production write: the loop-pass store is
    // the single writer, so a committed load records exactly one snapshot.
    assert_eq!(t.persisted.borrow().len(), 1);
    assert_eq!(t.persisted.borrow()[0].item_ids, vec!["new".to_string()]);
    assert!(
        !mbv_config::stay_alive_queue_state_path().exists(),
        "install path must not write the production store directly"
    );
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 5,
            result: mbv_ctrl::QueueLoadResult::Accepted,
        }
    ));
}
