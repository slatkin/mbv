// `UnifiedQueuePlaySlot` must start the target slot even when no Playback run
// is alive: a bare `JumpTo` is dropped by a cold owner, which wedged every
// later jump behind the never-settling transition (Enter on an Audiobookshelf
// item after an owner restart played nothing and queued every retry).

use super::*;

#[test]
fn play_slot_on_a_cold_owner_starts_the_queue_at_the_target_slot() {
    let player = cold_player();
    // A dropped receiver is the dead-run condition: `send_command` fails
    // exactly as it does when no player thread is alive.
    drop(player.spy_on_commands());
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
    let target = owner.core.queue.slots()[1].slot_id;

    run_queue_cmd(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(target),
            op: None,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    // The cold submission seeds the run at the target slot.
    let status = player.status.lock().unwrap();
    assert!(status.active, "the cold start activated playback");
    assert_eq!(status.current_idx, 1, "playback starts at the target slot");
    assert_eq!(
        status.queue_len, 2,
        "the canonical queue is submitted whole"
    );
    drop(status);
    // No transition lingers in flight: a cold run's TrackChanged carries the
    // run identity, never this request identity, so an in-flight entry could
    // never settle and would hold every later jump behind it until expiry.
    assert_eq!(owner.core.in_flight_transition_slot(), None);
}

#[test]
fn play_slot_with_a_live_run_still_jumps_instead_of_resubmitting() {
    let player = cold_player();
    // A live spy receiver stands in for the live run's command channel:
    // `send_command` delivers, so the jump must stay a JumpTo.
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
    let target = owner.core.queue.slots()[1].slot_id;

    run_queue_cmd(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(target),
            op: None,
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    match cmd_rx.recv().unwrap() {
        PlayerCommand::JumpTo { .. } => {}
        other => panic!("a live run receives the JumpTo, got {other:?}"),
    }
    // The accepted transition stays in flight awaiting the run's own
    // request-identity observation.
    assert_eq!(owner.core.in_flight_transition_slot(), Some(target));
}
