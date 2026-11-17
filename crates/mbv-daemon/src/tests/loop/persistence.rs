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
fn queue_replace_command_persists_once_through_injected_store() {
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
    let (reply_tx, _reply_rx) = mpsc::channel();

    let flow = t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueReplace {
            queue: None,
            items: vec![],
            slots: vec![mbv_ctrl::UnifiedQueueSlot {
                slot_id: 1,
                item: emby_qi("new", "Video", "Movie"),
            }],
            start_idx: Some(0),
            source: QueueSource::Album,
            start_position_secs: None,
            op: None,
        },
        client_id,
        reply_tx,
    ));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Album);
    // A queue-mutating command records exactly one snapshot through the
    // loop-pass store.
    assert_eq!(t.persisted.borrow().len(), 1);
    assert_eq!(t.persisted.borrow()[0].item_ids, vec!["new".to_string()]);
    assert!(
        !mbv_config::stay_alive_queue_state_path().exists(),
        "no command path writes the production store directly"
    );
}

/// Contract (gui-named-queues review fix): a Named-queue persistence request
/// carries no snapshot — the loop writes the live state at handling time, so
/// a request queued behind a structural change can never overwrite the newer
/// state with an older one, and a request whose live state already matches
/// the capture writes nothing.
#[test]
fn a_named_queues_request_never_writes_a_state_older_than_the_last_capture() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("first", "Video", "Movie")],
        0,
    );

    // The request writes the live state and records the capture.
    t.event_loop.handle_event(DaemonEvent::NamedQueuesDirty);
    assert_eq!(t.persisted_named.borrow().len(), 1);

    // A structural change lands behind the request; its dirty event writes
    // the newer state and updates the capture.
    t.event_loop
        .owner
        .core
        .queue
        .append(emby_qi("added", "Video", "Movie"));
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    t.event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(slot),
            run_identity: current_run(&t.event_loop),
            position_ticks: 0,
            played: false,
            consume: false,
            error: None,
        }));
    assert_eq!(t.persisted_named.borrow().len(), 2);
    let newest = t.persisted_named.borrow().last().unwrap().to_string();
    assert!(
        newest.contains("added"),
        "the file holds the newer state: {newest}"
    );

    // The queued request now finds the live state already captured: the
    // newer state stays in the file and no older snapshot overwrites it.
    t.event_loop.handle_event(DaemonEvent::NamedQueuesDirty);
    assert_eq!(t.persisted_named.borrow().len(), 2);
}

/// Contract (gui-named-queues review fix): a failed Named-queue save does not
/// record the capture — a store error, a gone worker, or a worker write
/// failure leaves the capture unset, so the next dirty event retries the
/// write instead of trusting a file older than the capture.
#[test]
fn a_failed_named_queues_save_leaves_the_capture_unset_so_the_next_request_retries() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("first", "Video", "Movie")],
        0,
    );
    let fail_first = Rc::new(std::cell::Cell::new(true));
    let fail_flag = Rc::clone(&fail_first);
    let recorded_named = Rc::clone(&t.persisted_named);
    t.event_loop.named_store = Box::new(move |state| {
        if fail_flag.get() {
            fail_flag.set(false);
            return Err(crate::DaemonLibError::owner_context("disk full"));
        }
        recorded_named
            .borrow_mut()
            .push(serde_json::to_value(&state).unwrap_or(serde_json::Value::Null));
        Ok(())
    });

    // The first save fails, so no capture is recorded.
    t.event_loop.handle_event(DaemonEvent::NamedQueuesDirty);
    assert!(t.event_loop.named_queues_saved.is_none());
    assert!(t.persisted_named.borrow().is_empty());

    // Because the capture stayed unset, the next request retries and records
    // the capture only after the write succeeds.
    t.event_loop.handle_event(DaemonEvent::NamedQueuesDirty);
    assert!(t.event_loop.named_queues_saved.is_some());
    assert_eq!(t.persisted_named.borrow().len(), 1);

    // A worker write failure reported back unsets the capture again.
    t.event_loop
        .handle_event(DaemonEvent::NamedQueuesPersistFailed("disk full".into()));
    assert!(t.event_loop.named_queues_saved.is_none());
}
