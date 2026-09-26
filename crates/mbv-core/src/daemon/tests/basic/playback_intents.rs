use super::*;

// ── relative transport steps advance from the desired slot ──────────────

/// A Next intent while a jump to B is in flight steps from B (queues C
/// behind it), never from the published `current_idx` mirror — which still
/// names A until the run confirms and made a rapid second press re-target B
/// (the erratic Next report). Both presses must also act: the owner, not a
/// client's status mirror, bounds-checks the step.
#[test]
fn next_intent_while_a_jump_is_in_flight_steps_from_the_desired_slot() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let queue = queue_from_items(
        &[
            item("a", "Video", "Movie"),
            item("b", "Video", "Movie"),
            item("c", "Video", "Movie"),
        ],
        0,
    );
    let slot_b = queue.slots()[1].slot_id;
    let slot_c = queue.slots()[2].slot_id;
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let shared = shared_queue_state();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let next_intent = |request_id: u64| {
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id,
            generation: request_id,
            action: PlaybackIntentAction::Next,
        })
    };

    // First press: nothing in flight, observed slot A -> jump to B.
    handle_ctrl_for_role(
        next_intent(1),
        CtrlContext {
            reply_tx: &(mpsc::channel().0),
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    assert!(
        matches!(
            cmd_rx.recv().unwrap(),
            PlayerCommand::JumpTo { slot_id, .. } if slot_id == slot_b
        ),
        "the first Next dispatches a slot jump to B"
    );

    // Second rapid press, B still in flight: steps from B and queues C
    // behind it.
    handle_ctrl_for_role(
        next_intent(2),
        CtrlContext {
            reply_tx: &(mpsc::channel().0),
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    // The second press must not dispatch past the in-flight jump (one
    // in-flight at a time, design D4): C is held queued, not sent to the run.
    assert!(
        cmd_rx.try_recv().is_err(),
        "no second dispatch while B is in flight"
    );
    // The owner snapshot names the true desired pair: B in flight, C queued.
    // (Stepping from the mirror would have queued B again.) Drain the
    // already-queued events without ever blocking on recv.
    let mut state = None;
    while let Ok(outbound) = client_rx.try_recv() {
        if let CtrlOutbound::Event(json) = outbound {
            if let CtrlEvent::UnifiedQueueState(s) = serde_json::from_str(&json).unwrap() {
                state = Some(s);
            }
        }
    }
    let state = state.expect("a UnifiedQueueState snapshot was broadcast");
    assert_eq!(
        state.in_flight_transition.map(|t| t.target_slot),
        Some(slot_b.raw()),
        "B remains in flight"
    );
    assert_eq!(
        state.queued_latest_transition.map(|t| t.target_slot),
        Some(slot_c.raw()),
        "the second press queues C behind the in-flight B"
    );
}

// ── active-file JumpTo confirms via TrackChanged (stay-alive D1) ─────────

/// Active-file mode has no mpv playlist move to observe, so the daemon's
/// queue tracking depends entirely on the run's `TrackChanged` response to a
/// `JumpTo`. The run side needs mpv and is exercised by design review; this
/// test pins the daemon half of the loop: dispatch a slot jump, mock the
/// Playback run's `TrackChanged` response exactly as the active-file `JumpTo`
/// handler now emits it (target slot + the jump's request identity), and
/// assert the observed active slot advances to the target and the in-flight
/// transition settles — the closed loop Next/Previous resolves from.
#[test]
fn active_file_jump_to_observed_slot_advances_when_the_run_confirms_via_track_changed() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _client_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let queue = queue_from_items(
        &[
            item("a", "Video", "Movie"),
            item("b", "Video", "Movie"),
            item("c", "Video", "Movie"),
        ],
        0,
    );
    let slot_b = queue.slots()[1].slot_id;
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let shared = shared_queue_state();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    // Nothing observed yet: the pre-D1 broken state this change fixes.
    assert_eq!(owner.core.observed_active_slot(), None);

    // Next press dispatches the slot jump the active-file run will execute.
    handle_ctrl_for_role(
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 1,
            generation: 1,
            action: PlaybackIntentAction::Next,
        }),
        CtrlContext {
            reply_tx: &(mpsc::channel().0),
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared,
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    let (jump_request_id, jump_generation) = match cmd_rx.recv().unwrap() {
        PlayerCommand::JumpTo {
            slot_id,
            request_id,
            generation,
            ..
        } => {
            assert_eq!(slot_id, slot_b, "the jump targets the next slot");
            (request_id, generation)
        }
        other => panic!("expected a JumpTo dispatch, got {other:?}"),
    };

    // Mock the active-file run's response (design D1): TrackChanged naming
    // the target slot and carrying the jump's request identity, then apply
    // the daemon loop's TrackChanged arm (observe → settle → publish).
    let run_response = crate::player::PlayerEvent::TrackChanged {
        slot_id: slot_b,
        transition: Some((jump_request_id, jump_generation)),
    };
    let observed = match run_response {
        crate::player::PlayerEvent::TrackChanged {
            slot_id,
            transition: Some((request_id, _generation)),
        } if slot_id == slot_b => {
            let (_, resolved) = owner
                .core
                .observe_track_change(slot_id)
                .expect("the run reports a slot the canonical queue still holds");
            super::settle_and_redispatch(&mut owner, &player, request_id, resolved);
            *shared.observed_active_slot.lock().unwrap() = owner.core.observed_active_slot();
            Some((1, resolved))
        }
        _ => panic!("expected a TrackChanged confirmation for slot B"),
    };
    assert_eq!(observed, Some((1, slot_b)));
    assert_eq!(
        *shared.observed_active_slot.lock().unwrap(),
        Some(slot_b),
        "the observed active slot advances to the jump target"
    );
    assert_eq!(
        owner.core.in_flight_transition_slot(),
        None,
        "the jump's transition settles on the run's confirmation"
    );
}
