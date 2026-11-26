// Cast→Local hand-back and receiver teardown (cast-owner-output row 7.1
// dispatch B and row 7.2/7.3, design D10/D12).

use super::*;

/// Contract: cast-owner-output row 7.2 (design D10 step 5, dispatch B) — a
/// Cast→Local hand-off from a paused receiver gives a paused local run at
/// the receiver's position: the fresh local run is submitted at the
/// handed-off slot with the receiver's position as its resume position, and
/// the captured pause reaches the local player as an explicit pause command
/// after the submission (a local cold start cannot begin paused).
#[test]
fn a_paused_receiver_hands_back_to_a_paused_local_run_at_the_receiver_position() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let position_ticks = 10 * mbv_emby_model::TICKS_PER_SECOND;
    let receiver = CastStatus {
        state: mbv_cast::CastPlaybackState::Paused,
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let (client_id, rx, _calls) = select_cast_output(&mut t, receiver);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(&mut t, client_id, slot);
    // One tick runs the status poll, so the output presents the receiver's
    // paused state at its reported position before the hand-off.
    let now = std::time::Instant::now();
    t.event_loop.tick(now);
    pump(&mut t);
    // Drain the events the load and the status produced, so the events the
    // hand-off emits are the next ones the channel holds.
    while rx.try_recv().is_ok() {}

    let commands = t.event_loop.player.spy_on_commands();
    select_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );

    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the receiver hands playback back to local"
    );
    // The old output's stop report precedes the new output's state (design
    // D9): the Feed `Stopped` goes out before the Local broadcast.
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::Player(PlayerEvent::Stopped { .. })
        ),
        "the handed-off slot's stop report goes out first"
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Local)
        ),
        "the output change is broadcast after the stop report"
    );
    assert_eq!(
        commands
            .try_recv()
            .map(|command| matches!(command, PlayerCommand::TogglePause)),
        Ok(true),
        "the captured pause reaches the local player as a pause command"
    );
    assert!(
        commands.try_recv().is_err(),
        "the hand-off touches the local player only to pause it"
    );
    assert_eq!(
        mbv_player::resume_ticks_for_slot(&t.event_loop.owner.core.queue, slot),
        Some(position_ticks),
        "the receiver's position is the fresh run's resume position"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active && status.position_ticks == position_ticks,
        "the local run presents the handed-off slot at the receiver's position"
    );
}

/// Contract: cast-owner-output row 7.1 dispatch B / design D12 — selecting
/// Local tears the receiver app down after its media stops: the deliberate
/// output change queues the teardown job behind the media-stop job. (A user
/// Stop records no teardown — owned by the row 5.3 stop test.)
#[test]
fn selecting_local_tears_the_receiver_app_down_after_the_media_stop() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (client_id, _rx, calls) = select_cast_output(&mut t, CastStatus::idle());
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(&mut t, client_id, slot);

    let (release_tx, worker) = hold_cast_worker(&t);
    select_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the output swaps to Local"
    );
    let _ = release_tx.send(());
    let (done_tx, done_rx) = mpsc::channel();
    let _ = worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the select's queued jobs ran before the sentinel");

    let calls = calls.lock().unwrap();
    let stop = calls
        .iter()
        .position(|call| *call == "stop")
        .expect("the media stop lands on the receiver");
    let teardown = calls
        .iter()
        .position(|call| *call == "teardown")
        .expect("the deliberate output change tears the receiver app down");
    assert!(
        stop < teardown,
        "the teardown runs after the media stop, in worker FIFO order: {calls:?}"
    );
}

/// Contract: cast-owner-output row 7.3 (design D12) — the Owner's shutdown
/// drops the receiver connection without tearing the receiver app down: the
/// shutdown event queues neither a media stop nor an app teardown on the
/// receiver's worker, so the receiver keeps playing when the Owner exits.
/// (A deliberate select of `Local` does tear down — owned by
/// `selecting_local_tears_the_receiver_app_down_after_the_media_stop`.)
#[test]
fn the_owner_shutdown_queues_no_stop_and_no_teardown_on_the_receiver() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (client_id, _rx, calls) =
        select_cast_output(&mut t, playing_status_at("https://feed/a.mp3", 10.0));
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(&mut t, client_id, slot);
    // One tick runs the status poll, so the receiver presents its loaded
    // item when the Owner shuts down.
    t.event_loop.tick(std::time::Instant::now());
    pump(&mut t);

    // The worker is held behind a blocker, so any job the shutdown queued —
    // if it queued one — runs before the sentinel the test queues last.
    let (release_tx, worker) = hold_cast_worker(&t);
    t.event_loop.handle_event(DaemonEvent::Shutdown);
    let _ = release_tx.send(());
    let (done_tx, done_rx) = mpsc::channel();
    let _ = worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the shutdown's queued jobs, if any, ran before the sentinel");

    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| *call == "stop" || *call == "teardown"),
        "the shutdown stops neither the receiver media nor its app: {calls:?}"
    );
}

/// Contract: switch-output-without-hand-off row 2.5 and row 2.6 — a
/// `SwitchPlaybackOutput` to Local while a Cast receiver plays captures no
/// hand-off: the receiver's media stops and its app tears down, Local becomes
/// the output, no local run is submitted, the queue keeps its order, and the
/// observed active slot is recorded and published as none (invariant 25).
#[test]
fn a_switch_to_local_stops_the_receiver_and_submits_no_local_run() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let status = CastStatus {
        duration_seconds: Some(10.0),
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let (client_id, _rx, calls) = select_cast_output(&mut t, status);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(&mut t, client_id, slot);
    // One tick runs the status poll, so the receiver presents the loaded item
    // and the loaded slot becomes the observed one.
    t.event_loop.tick(std::time::Instant::now());
    pump(&mut t);
    assert_eq!(
        t.event_loop.owner.core.observed_active_slot(),
        Some(slot),
        "the receiver's loaded slot is the observed slot before the switch"
    );
    t.event_loop
        .shared_queue
        .publish_observed(&t.event_loop.owner.core);
    let queue_before: Vec<_> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    // A submission that reuses a live local run would reach this spy; one
    // that cold-starts instead seeds the local status as active below.
    let commands = t.event_loop.player.spy_on_commands();
    let (release_tx, worker) = hold_cast_worker(&t);

    switch_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );
    let _ = release_tx.send(());
    let (done_tx, done_rx) = mpsc::channel();
    let _ = worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the switch's queued jobs ran before the sentinel");

    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the switch makes Local the output"
    );
    let calls = calls.lock().unwrap();
    let stop = calls
        .iter()
        .position(|call| *call == "stop")
        .expect("the receiver's media stops");
    let teardown = calls
        .iter()
        .position(|call| *call == "teardown")
        .expect("the receiver's app is torn down");
    assert!(
        stop < teardown,
        "the teardown runs after the media stop, in worker FIFO order: {calls:?}"
    );
    assert!(
        commands.try_recv().is_err(),
        "a switch submits no local run and pauses nothing"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "no local run starts, so the local output presents no playback"
    );
    let queue_after: Vec<_> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    assert_eq!(
        queue_after, queue_before,
        "the queue and its order are unchanged"
    );
    assert_eq!(
        t.event_loop.owner.core.observed_active_slot(),
        None,
        "the switch records no observed active slot"
    );
    assert_eq!(
        *t.event_loop
            .shared_queue
            .observed_active_slot
            .lock()
            .unwrap(),
        None,
        "the switch publishes the observed slot as none (invariant 25)"
    );
}
