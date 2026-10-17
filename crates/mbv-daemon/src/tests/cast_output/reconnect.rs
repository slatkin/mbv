// Status polling, watchdog recovery, and reconnect behavior for a Cast
// output (cast-owner-output rows 3.3 and 7.4, design D8/D11).

use super::*;

/// A Cast connector whose connect always succeeds, backed by a fake
/// transport whose status poll answers `status`; records each connect's
/// `ConnectMode` so a test can tell a first select from a reconnect.
type RecordedConnector = (
    CastConnector,
    Arc<Mutex<Vec<String>>>,
    Arc<Mutex<Vec<ConnectMode>>>,
);

fn recording_connector(status: Option<mbv_cast::CastStatus>) -> RecordedConnector {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let modes = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let mode_log = Arc::clone(&modes);
    let connector: CastConnector = Box::new(move |request: CastConnectRequest| {
        mode_log.lock().unwrap().push(request.mode);
        let transport = mbv_cast::FakeCastTransport {
            calls: Arc::clone(&log),
            status: status.clone(),
            refuse_commands: false,
        };
        let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
        let _ = request.reply.send(CastConnectOutcome::Connected(worker));
    });
    (connector, calls, modes)
}

/// One sequenced connect outcome: a connect that succeeds with a transport
/// answering `status` on every poll, or one that fails with `reason`.
#[derive(Clone)]
enum SequencedConnect {
    Succeeds(Option<CastStatus>),
    Fails(String),
}

/// A Cast connector that plays `steps` in connect order — first select
/// first, reconnects after — and repeats the last step for further connects.
/// Records each connect's `ConnectMode` and shares one call log across the
/// succeeded connects, so a test can tell what each connection answered.
fn sequenced_connector(steps: Vec<SequencedConnect>) -> RecordedConnector {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let modes = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let mode_log = Arc::clone(&modes);
    let next = Arc::new(Mutex::new(0usize));
    let connector: CastConnector = Box::new(move |request: CastConnectRequest| {
        let step = {
            let mut next = next.lock().unwrap();
            let step = steps
                .get(*next)
                .cloned()
                .unwrap_or_else(|| steps.last().expect("at least one step").clone());
            *next += 1;
            step
        };
        mode_log.lock().unwrap().push(request.mode);
        match step {
            SequencedConnect::Succeeds(status) => {
                let transport = mbv_cast::FakeCastTransport {
                    calls: Arc::clone(&log),
                    status,
                    refuse_commands: false,
                };
                let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
                let _ = request.reply.send(CastConnectOutcome::Connected(worker));
            }
            SequencedConnect::Fails(reason) => {
                let _ = request.reply.send(CastConnectOutcome::Failed(reason));
            }
        }
    });
    (connector, calls, modes)
}

/// Contract: cast-owner-output row 3.3 — a status poll that fails marks the
/// output disconnected, broadcasts the change, and reconnects in reconnect
/// mode (design D8). The queue and its active slot are untouched: the
/// connection loss says nothing about the owner's playback intent.
#[test]
fn a_failed_status_poll_disconnects_broadcasts_and_reconnects() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let (connector, _calls, modes) = recording_connector(None);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: OutputConnection::Connected,
                ..
            })
        ),
        "the select is broadcast before the tick"
    );
    let revision_before = t.event_loop.owner.core.queue.revision();
    let active_before = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let start = std::time::Instant::now();
    t.event_loop.tick(start);
    let event = t.merged_rx.recv().expect("the poll reports its result");
    t.event_loop.handle_event(event);
    let event = t
        .merged_rx
        .recv()
        .expect("the reconnect reports its outcome");
    t.event_loop.handle_event(event);

    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect, ConnectMode::Reconnect],
        "the loss reconnects in reconnect mode"
    );
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: OutputConnection::Connected,
        },
        "a successful reconnect restores the connection"
    );
    assert_eq!(t.event_loop.owner.core.queue.revision(), revision_before);
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[0].slot_id,
        active_before
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: OutputConnection::Disconnected,
                ..
            })
        ),
        "the loss is broadcast"
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: OutputConnection::Connected,
                ..
            })
        ),
        "the restored connection is broadcast"
    );
}

/// Contract: cast-owner-output row 3.3 — a poll whose worker event never
/// arrives within the answer bound is a wedged worker (`rust_cast` has no
/// socket timeout): the tick marks the output disconnected and reconnects.
/// The status event the wedged poll eventually produced sits unpumped, as it
/// would on a wedged loop, and must not disturb the recovery.
#[test]
fn a_poll_with_no_worker_event_within_the_answer_bound_disconnects() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, _calls, modes) = recording_connector(Some(mbv_cast::CastStatus::idle()));
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: OutputConnection::Connected,
                ..
            })
        ),
        "the select is broadcast before the tick"
    );

    let start = std::time::Instant::now();
    // The poll is submitted; no worker event is pumped back.
    t.event_loop.tick(start);
    t.event_loop
        .tick(start + crate::cast::CAST_JOB_ANSWER_BOUND + std::time::Duration::from_secs(1));

    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: OutputConnection::Disconnected,
        },
        "the wedged worker's poll is treated as a lost connection"
    );
    // The late status of the wedged poll is ignored, then the reconnect
    // outcome restores the connection.
    let event = t.merged_rx.recv().unwrap();
    t.event_loop.handle_event(event);
    let event = t.merged_rx.recv().unwrap();
    t.event_loop.handle_event(event);
    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect, ConnectMode::Reconnect]
    );
    assert!(
        matches!(
            crate::cast::output_state(&t.event_loop.owner.output),
            OutputState::Cast {
                connection: OutputConnection::Connected,
                ..
            }
        ),
        "the reconnect restores the connection"
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: OutputConnection::Disconnected,
                ..
            })
        ),
        "the loss is broadcast"
    );
}

/// Contract: cast-owner-output row 3.3 — a returned status disarms the
/// watchdog clock and re-arms the poll cadence: a later tick past the poll
/// interval polls again, and no reconnect fires.
#[test]
fn a_returned_status_rearms_the_poll_and_keeps_the_connection() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, _calls, modes) = recording_connector(Some(mbv_cast::CastStatus::idle()));
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let start = std::time::Instant::now();
    t.event_loop.tick(start);
    let event = t
        .merged_rx
        .recv()
        .expect("the first poll reports its status");
    t.event_loop.handle_event(event);

    t.event_loop
        .tick(start + crate::cast::CAST_STATUS_POLL_INTERVAL + std::time::Duration::from_secs(1));
    let event = t
        .merged_rx
        .recv()
        .expect("the poll cadence re-arms and polls again");
    t.event_loop.handle_event(event);

    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect],
        "a responsive poll never reconnects"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Cast(_)),
        "the connection stays"
    );
}

/// The owner's `OutputState` for the test receiver in `connection`.
fn cast_output_state(connection: OutputConnection) -> OutputState {
    OutputState::Cast {
        receiver_id: "rcv-1".to_string(),
        name: "Living room".to_string(),
        connection,
    }
}

/// A hand-stamped late result of the old worker: the same shape the worker
/// sends, carrying a stale poll id.
fn stale_status(poll_id: u64) -> DaemonEvent {
    DaemonEvent::Cast(crate::cast::CastOwnerEvent::Status {
        receiver_id: "rcv-1".to_string(),
        poll_id,
        result: Err("late worker lost the connection".to_string()),
    })
}

/// Contract: review P2 on row 3.3 — a late result from the previous worker
/// (the watchdog reconnect race) carries a stale poll id and is dropped: it
/// causes no extra disconnect/reconnect flap and leaves the new poll's
/// watchdog armed.
#[test]
fn a_late_result_from_the_old_worker_leaves_the_new_poll_alone() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, _calls, modes) = recording_connector(None);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(OutputConnection::Connected)
    ));

    let start = std::time::Instant::now();
    // Poll P1 goes to worker W1; its result sits unpumped.
    t.event_loop.tick(start);
    // W1's blocking status exceeds the answer bound: the watchdog disconnects
    // and reconnects.
    t.event_loop
        .tick(start + crate::cast::CAST_JOB_ANSWER_BOUND + std::time::Duration::from_secs(1));
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(OutputConnection::Disconnected)
    ));

    // W1's late result arrives while no poll is in flight: dropped silently.
    let event = t.merged_rx.recv().unwrap();
    let DaemonEvent::Cast(crate::cast::CastOwnerEvent::Status {
        poll_id: stale_poll_id,
        ..
    }) = event
    else {
        panic!("expected the wedged poll's late status result");
    };
    t.event_loop.handle_event(stale_status(stale_poll_id));
    assert!(rx.try_recv().is_err(), "the stale late result is silent");
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        cast_output_state(OutputConnection::Disconnected),
        "the stale late result causes no flap"
    );

    // The reconnect restores worker W2.
    let event = t
        .merged_rx
        .recv()
        .expect("the reconnect reports its outcome");
    t.event_loop.handle_event(event);
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(OutputConnection::Connected)
    ));

    // The next tick polls W2 (P2).
    t.event_loop
        .tick(start + std::time::Duration::from_secs(40));

    // A late result of the old worker arrives while P2 is in flight: dropped
    // by its stale id — no spurious disconnect.
    t.event_loop.handle_event(stale_status(stale_poll_id));
    assert!(rx.try_recv().is_err(), "the stale late result is silent");
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        cast_output_state(OutputConnection::Connected),
        "the stale late result does not disconnect the restored worker"
    );

    // P2's watchdog was not disarmed by the stale result: a tick past the
    // answer bound still fires it.
    t.event_loop
        .tick(start + std::time::Duration::from_secs(40) + crate::cast::CAST_JOB_ANSWER_BOUND);
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(OutputConnection::Disconnected)
    ));
    // The reconnect the fired watchdog spawns is the only one after the
    // recovery: the stale late results spawned none.
    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![
            ConnectMode::FirstSelect,
            ConnectMode::Reconnect,
            ConnectMode::Reconnect
        ],
    );
}

/// A receiver status that reports the loaded content's media session went
/// idle for `Finished` (design D7): idle, no position, the content session
/// still named.
fn finished_status(content_id: &str) -> CastStatus {
    CastStatus {
        state: mbv_cast::CastPlaybackState::Idle,
        position_seconds: None,
        duration_seconds: None,
        playback_rate: 1.0,
        playing_content_id: Some(content_id.to_string()),
        idle_reason: Some(mbv_cast::CastIdleReason::Finished),
    }
}

/// Loads the queue's first slot on the selected receiver and runs the settle
/// tick, so the output presents the loaded item before the outage under test.
fn load_and_settle_first_slot(t: &mut TestLoop, client_id: u64) -> mbv_queue::QueueSlotId {
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    pump(t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(t);
    slot
}

/// Drives the connection into the `Disconnected` reconnect through the real
/// watchdog path: one tick submits a poll at the poll cadence, a later tick
/// past the answer bound marks the worker wedged, and the pump drains the
/// wedged poll's late result (dropped) and the reconnect outcome.
fn disconnect_through_the_watchdog(t: &mut TestLoop, t0: std::time::Instant) {
    let outage = t0 + crate::cast::CAST_STATUS_POLL_INTERVAL + std::time::Duration::from_secs(1);
    t.event_loop.tick(outage);
    t.event_loop
        .tick(outage + crate::cast::CAST_JOB_ANSWER_BOUND + std::time::Duration::from_secs(1));
    let event = t
        .merged_rx
        .recv()
        .expect("the wedged poll reported its result");
    t.event_loop.handle_event(event);
    let event = t
        .merged_rx
        .recv()
        .expect("the reconnect reported its outcome");
    t.event_loop.handle_event(event);
}

/// Contract: cast-owner-output row 7.4 (design D11, D7) — after the
/// connection dropped, a reconnect whose first status reports the loaded
/// item's `Finished` end picks the missed end up and advances the queue
/// exactly once: one load for the next slot, no second step.
#[test]
fn a_reconnect_that_sees_the_loaded_item_finished_advances_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    let (connector, calls, modes) = sequenced_connector(vec![
        SequencedConnect::Succeeds(Some(playing_status_at("https://feed/a.mp3", 10.0))),
        SequencedConnect::Succeeds(Some(finished_status("https://feed/a.mp3"))),
    ]);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    load_and_settle_first_slot(&mut t, client_id);
    let t0 = std::time::Instant::now();

    disconnect_through_the_watchdog(&mut t, t0);
    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect, ConnectMode::Reconnect],
        "the loss reconnects in reconnect mode"
    );

    // The first poll of the restored connection reports the loaded item's
    // Finished end: the end the outage missed advances the queue once.
    t.event_loop.tick(t0 + std::time::Duration::from_secs(600));
    let event = t
        .merged_rx
        .recv()
        .expect("the restored connection's poll reports its result");
    t.event_loop.handle_event(event);
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        load_calls(&calls),
        vec![
            "load(https://feed/a.mp3, 0)".to_string(),
            "load(https://feed/b.mp3, 0)".to_string()
        ],
        "the reconnect's Finished report advances exactly one slot"
    );
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: OutputConnection::Connected,
        },
        "the restored connection stays the output"
    );
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    assert_eq!(
        cast.loaded.as_ref().expect("the advance loaded").slot,
        t.event_loop.owner.core.queue.slots()[1].slot_id,
        "the advance's load is the next slot, not the ended one"
    );
}

/// Contract: cast-owner-output row 7.4 (design D11) — a reconnect refused
/// because another application runs on the receiver leaves the output Cast
/// and `Disconnected`: it never launches over the other sender's app, never
/// swaps to Local, and the loaded item is kept for the user's next decision.
#[test]
fn a_reconnect_refused_because_another_app_runs_stays_disconnected() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (connector, calls, modes) = sequenced_connector(vec![
        SequencedConnect::Succeeds(Some(playing_status_at("https://feed/a.mp3", 10.0))),
        SequencedConnect::Fails("another application is running on the receiver".to_string()),
    ]);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    let slot = load_and_settle_first_slot(&mut t, client_id);
    let t0 = std::time::Instant::now();

    disconnect_through_the_watchdog(&mut t, t0);

    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect, ConnectMode::Reconnect],
        "the refusal is not retried and never launches over the other app"
    );
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: OutputConnection::Disconnected,
        },
        "the refused reconnect leaves the output disconnected"
    );
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver stays the output");
    };
    assert_eq!(
        cast.loaded.as_ref().expect("the load is kept").slot,
        slot,
        "the loaded item is kept for the user's next decision"
    );
    assert_eq!(
        load_calls(&calls),
        vec!["load(https://feed/a.mp3, 0)".to_string()],
        "the refused reconnect loads nothing"
    );
    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| *call == "stop" || *call == "teardown"),
        "the refused reconnect stops and tears nothing down"
    );
    let events = drained_events(&rx);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CtrlEvent::PlaybackOutput(state) if *state == OutputState::Cast {
                receiver_id: "rcv-1".to_string(),
                name: "Living room".to_string(),
                connection: OutputConnection::Disconnected,
            })),
        "the disconnect was broadcast"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, CtrlEvent::PlaybackOutput(OutputState::Local))),
        "the output never swaps to Local"
    );
}
