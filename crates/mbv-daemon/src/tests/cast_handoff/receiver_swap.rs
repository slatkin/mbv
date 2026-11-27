// Cast→Cast receiver swaps (cast-owner-output row 7.1 dispatch C,
// design D10) and the Cast→Session switch (keep-old-output-playing-on-switch
// row 2.1).

use super::*;

use super::cast_output::connect_output_client_with_caps;
use super::emby_session_output::host_session_output;
use super::emby_session_output::pump_session_event;
use super::emby_session_output::session_selection;
use super::emby_session_output::wait_for_session_jobs;
use crate::emby_session::fake::FakeSessionApi;
use crate::emby_session::fake::session;

/// Contract: cast-owner-output row 7.1 dispatch C (design D10) — selecting a
/// second receiver while a first receiver plays connects to the new receiver
/// first, stops the old output only after the connect succeeded, and gives
/// the NEW receiver a `load` of the handed-off slot's media at the old
/// receiver's presented position. The old receiver's transport log shows the
/// media stop and then the app teardown, and the new receiver is given no
/// teardown.
#[test]
fn a_cast_handoff_loads_the_new_receiver_at_the_old_receivers_position() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    // The reported duration clamps the presented position's extrapolation,
    // so the capture reads exactly the reported position however long the
    // connect takes.
    let status = CastStatus {
        duration_seconds: Some(10.0),
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let presented = receiver_presenting(&mut t, status);
    let old_worker = presented.worker;
    let old_calls = presented.calls;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let (connector, new_calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, second_cast_selection());
    pump_cast_event(&mut t);
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    // The sentinel job queues behind the select's jobs on the old worker, so
    // its call log is complete once the sentinel answers.
    let (done_tx, done_rx) = mpsc::channel();
    let _ = old_worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the old receiver's queued jobs ran before the sentinel");

    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the new receiver is the output after the hand-off");
    };
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-2".to_string(),
            name: "Kitchen".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        },
        "the selected receiver is the output after the hand-off"
    );
    let loaded = cast.loaded.as_ref().expect("the hand-off loaded the slot");
    assert_eq!(loaded.slot, slot, "the captured slot is the loaded one");
    assert_eq!(
        *new_calls.lock().unwrap(),
        vec![
            "keep_alive".to_string(),
            "load(https://feed/a.mp3, 10)".to_string()
        ],
        "the new receiver is given the handed-off media at the old receiver's position, and no teardown"
    );
    let old_calls = old_calls.lock().unwrap();
    let stop = old_calls
        .iter()
        .position(|call| *call == "stop")
        .expect("the old receiver's media stops");
    let teardown = old_calls
        .iter()
        .position(|call| *call == "teardown")
        .expect("the old receiver's app is torn down");
    assert!(
        stop < teardown,
        "the old receiver's teardown runs after its media stop: {old_calls:?}"
    );
}

/// Contract: cast-owner-output row 7.1 dispatch C (design D10 step 5) — a
/// paused receiver's state carries over to the new receiver: the hand-off
/// load is followed by an explicit pause, so the new receiver holds the item
/// paused at the old receiver's position.
#[test]
fn a_paused_receiver_hands_off_to_a_paused_new_receiver_at_its_position() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let status = CastStatus {
        state: mbv_cast::CastPlaybackState::Paused,
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let _presented = receiver_presenting(&mut t, status);

    let (connector, new_calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, second_cast_selection());
    pump_cast_event(&mut t);
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        *new_calls.lock().unwrap(),
        vec![
            "keep_alive".to_string(),
            "load(https://feed/a.mp3, 10)".to_string(),
            "pause".to_string()
        ],
        "the captured pause applies after the load, in order"
    );
}

/// Contract: cast-owner-output row 7.1 dispatch C (design D10 step 2) — a
/// connect failure to the new receiver keeps the old output fully untouched:
/// it stays connected with its loaded item presenting, its transport logs no
/// stop and no teardown, and the selecting Client is told the reason.
#[test]
fn a_failed_cast_to_cast_connect_leaves_the_old_receiver_untouched() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let status = CastStatus {
        duration_seconds: Some(10.0),
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let old_calls = receiver_presenting(&mut t, status).calls;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    t.event_loop.cast_connector = Some(failing_connector("receiver unreachable"));
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    drain(&rx);
    select_output(&mut t, client_id, &reply_tx, second_cast_selection());
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("a failed connect reports to the selecting Client");
    };
    assert!(reason.contains("receiver unreachable"), "reason: {reason}");
    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the old receiver stays the output");
    };
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        },
        "the old receiver stays the output on a failed connect"
    );
    let loaded = cast.loaded.as_ref().expect("the old output keeps its load");
    assert_eq!(
        loaded.slot, slot,
        "the old output's loaded item is untouched"
    );
    assert!(
        !old_calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| *call == "stop" || *call == "teardown"),
        "the old receiver's media is not stopped and its app is not torn down"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active,
        "the old receiver keeps presenting the loaded item"
    );
}

/// Contract: correction P1 on row 7.1 — a Local selection wins over a
/// Cast→Cast connect that is still in flight (last selection wins): the
/// pending selector is told, the hand-back still runs — the receiver's
/// presented playback continues on a fresh local run at its position while
/// the receiver's media stops and its app tears down — and the late connect
/// outcome matches nothing and is discarded.
#[test]
fn selecting_local_while_a_cast_connect_is_pending_hands_back_to_local() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    // The reported duration clamps the presented position's extrapolation,
    // so the capture reads exactly the reported position.
    let status = CastStatus {
        duration_seconds: Some(10.0),
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let old_calls = receiver_presenting(&mut t, status).calls;
    t.event_loop.cast_connector = Some(fake_connector().0);
    let (selector_id, selector_tx, selector_rx) =
        connect_output_client(&t.event_loop.ctrl_clients, true);
    let (local_id, local_tx, _local_rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    // The Cast→Cast connect goes in flight ...
    select_output(&mut t, selector_id, &selector_tx, second_cast_selection());
    // ... and the newer Local selection must still hand the playback back.
    let (release_tx, old_worker) = hold_cast_worker(&t);
    select_output(&mut t, local_id, &local_tx, OutputSelection::Local);
    let _ = release_tx.send(());
    let (done_tx, done_rx) = mpsc::channel();
    let _ = old_worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the hand-back's queued jobs ran before the sentinel");
    // The late connect outcome matches nothing and is discarded.
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&selector_rx) else {
        panic!("the pending connect's selector is told");
    };
    assert!(reason.contains("chose local playback"), "reason: {reason}");
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the newer Local selection applies"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active && status.position_ticks == 10 * mbv_emby_model::TICKS_PER_SECOND,
        "the receiver's presented playback continues on the local run"
    );
    let old_calls = old_calls.lock().unwrap();
    let stop = old_calls
        .iter()
        .position(|call| *call == "stop")
        .expect("the old receiver's media stops");
    let teardown = old_calls
        .iter()
        .position(|call| *call == "teardown")
        .expect("the old receiver's app is torn down");
    assert!(
        stop < teardown,
        "the teardown runs after the media stop, in worker FIFO order: {old_calls:?}"
    );
}

/// Contract: correction P1 on row 7.1 (design D10 step 3, D12) — every
/// successful swap stops the old output, captured hand-off or not: an idle
/// old receiver's media stop reports nothing (nothing was loaded), but its
/// still-running app tears down, and the new receiver gets no teardown and
/// no load.
#[test]
fn a_swap_without_a_captured_handoff_still_tears_the_old_receiver_down() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    // The old receiver is the output and idle: nothing is loaded and nothing
    // presents, so the selection captures no hand-off.
    let (_client_id, _rx, old_calls) = select_cast_output(&mut t, CastStatus::idle());
    let (release_tx, old_worker) = hold_cast_worker(&t);

    let (connector, new_calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (selector_id, reply_tx, _rx2) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, selector_id, &reply_tx, second_cast_selection());
    pump_cast_event(&mut t);
    let _ = release_tx.send(());
    let (done_tx, done_rx) = mpsc::channel();
    let _ = old_worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the swap's queued jobs ran before the sentinel");

    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-2".to_string(),
            name: "Kitchen".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        },
        "the selected receiver is the output after the swap"
    );
    let old_calls = old_calls.lock().unwrap();
    let stop = old_calls
        .iter()
        .position(|call| *call == "stop")
        .expect("the old receiver's media stop is queued");
    let teardown = old_calls
        .iter()
        .position(|call| *call == "teardown")
        .expect("the old receiver's still-running app is torn down");
    assert!(
        stop < teardown,
        "the teardown runs after the media stop, in worker FIFO order: {old_calls:?}"
    );
    assert!(
        !old_calls.iter().any(|call| call.starts_with("load(")),
        "an idle old receiver is given no load: {old_calls:?}"
    );
    assert!(
        !new_calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| *call == "teardown" || call.starts_with("load(")),
        "the new receiver is neither torn down nor given a load"
    );
}

/// Contract: switch-output-without-hand-off row 2.3(a) and row 2.6 — a
/// `SwitchPlaybackOutput` to a receiver while Local plays captures no hand-off:
/// the connect succeeds, the local run stops, the receiver becomes the output
/// and is given no load, the queue keeps its order, and the observed active
/// slot is recorded and published as none (invariant 25).
#[test]
fn a_switch_to_a_receiver_stops_local_playback_and_loads_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    local_playing(&mut t, slot, 42 * mbv_emby_model::TICKS_PER_SECOND, false);
    let queue_before: Vec<_> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    // The local run's slot is the published observed slot before the switch.
    t.event_loop
        .shared_queue
        .publish_observed(&t.event_loop.owner.core);
    assert_eq!(
        *t.event_loop
            .shared_queue
            .observed_active_slot
            .lock()
            .unwrap(),
        Some(slot),
        "the local run's slot is published before the switch"
    );

    let (connector, calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    switch_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        },
        "the selected receiver is the output after the switch"
    );
    let receiver_calls = calls.lock().unwrap();
    assert!(
        receiver_calls.is_empty(),
        "a switch loads nothing on the receiver: {receiver_calls:?}"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "the local run stops, and the clean receiver presents no playback"
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

/// Contract: switch-output-without-hand-off row 2.3(b) — a failed connect for a
/// switch keeps the old output untouched: local playback keeps running at its
/// position, and the selecting Client is told the reason.
#[test]
fn a_failed_switch_connect_leaves_local_playback_running() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    local_playing(&mut t, slot, 42 * mbv_emby_model::TICKS_PER_SECOND, false);
    t.event_loop.cast_connector = Some(failing_connector("receiver unreachable"));
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    switch_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("a failed connect reports to the selecting Client");
    };
    assert!(reason.contains("receiver unreachable"), "reason: {reason}");
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the old output stays after a failed switch"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active && status.position_ticks == 42 * mbv_emby_model::TICKS_PER_SECOND,
        "local playback keeps running untouched"
    );
}

/// Contract: switch-output-without-hand-off row 2.3(c) — the connect-time guard
/// the switch skips still trips for a `Continue` selection that captured
/// nothing while playback started during the connect (the guard of
/// cast-owner-output, commit 6edcdad7f): the connect is discarded, the old
/// output stays, and the selector is told.
#[test]
fn a_continue_selection_that_captured_nothing_still_trips_the_connect_guard() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    // The local output is idle at the selection, so the capture finds nothing.
    t.event_loop.cast_connector = Some(fake_connector().0);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    // Playback starts on the old output while the connect is in flight.
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    local_playing(&mut t, slot, 0, false);
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("the guard reports to the selecting Client");
    };
    assert!(
        reason.contains("playback started while connecting"),
        "reason: {reason}"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the connect is discarded and the old output stays"
    );
    assert!(
        t.event_loop.player.status.lock().unwrap().active,
        "the playback that started while connecting keeps running"
    );
}

/// Contract: correction P2 on row 7.1 — selecting the receiver that is
/// already the output displaces a connect that is still in flight (last
/// selection wins): the pending selector is told, the same-receiver
/// selection is rejected as no hand-off, the late connect outcome matches
/// nothing and is discarded, and the output stays the receiver it already
/// was with its playback untouched.
#[test]
fn selecting_the_current_receiver_displaces_a_pending_connect() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let status = CastStatus {
        duration_seconds: Some(10.0),
        ..playing_status_at("https://feed/a.mp3", 10.0)
    };
    let old_calls = receiver_presenting(&mut t, status).calls;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    t.event_loop.cast_connector = Some(fake_connector().0);
    let (pending_id, pending_tx, pending_rx) =
        connect_output_client(&t.event_loop.ctrl_clients, true);
    let (current_id, current_tx, current_rx) =
        connect_output_client(&t.event_loop.ctrl_clients, true);

    // The Cast→Cast connect to the second receiver goes in flight ...
    select_output(&mut t, pending_id, &pending_tx, second_cast_selection());
    // ... and the newer selection of the current receiver displaces it.
    select_output(
        &mut t,
        current_id,
        &current_tx,
        super::cast_output::cast_selection(),
    );
    // The late connect outcome matches nothing and is discarded.
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&pending_rx) else {
        panic!("the pending connect's selector is told");
    };
    assert!(reason.contains("superseded"), "reason: {reason}");
    let CtrlEvent::CommandRejected(reason) = recv_event(&current_rx) else {
        panic!("the same-receiver selection is rejected");
    };
    assert!(
        reason.contains("already the playback output"),
        "reason: {reason}"
    );
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        },
        "the output stays the receiver it already was"
    );
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver stays the output");
    };
    let loaded = cast.loaded.as_ref().expect("the load is untouched");
    assert_eq!(loaded.slot, slot, "the loaded item is untouched");
    assert!(
        !old_calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| *call == "stop" || *call == "teardown"),
        "the receiver is not stopped and not torn down"
    );
}

/// Contract: keep-old-output-playing-on-switch row 2.1 (spec "A receiver is
/// released by a switch") — a switch from a playing Cast receiver to an Emby
/// Session releases the receiver: it receives no stop and no teardown, and
/// the Session becomes the output and is given no load. The queue keeps its
/// order and the observed slot is cleared and published as none
/// (invariant 25).
#[test]
fn a_switch_to_a_session_releases_the_old_receiver() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (client_id, _rx, old_calls) =
        select_cast_output(&mut t, playing_status_at("https://feed/a.mp3", 10.0));
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(&mut t, client_id, slot);
    // One tick runs the status poll, so the receiver presents its loaded
    // item and the loaded slot is the observed one before the switch.
    t.event_loop.tick(std::time::Instant::now());
    pump(&mut t);
    let queue_before: Vec<_> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();

    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    host_session_output(&mut t, fake);
    let (switch_id, reply_tx, _rx2) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    switch_output(&mut t, switch_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert!(
        !old_calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call == "stop" || call == "teardown"),
        "the released receiver receives no stop and no teardown: {old_calls:?}"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output after the switch");
    };
    assert_eq!(
        output.device_id, "device-1",
        "the selected Session is the output after the switch"
    );
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        None,
        "a switch loads nothing on the Session"
    );
    assert_eq!(
        queue_slot_ids_of(&t),
        queue_before,
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

/// The queue's slot ids in order.
fn queue_slot_ids_of(t: &TestLoop) -> Vec<mbv_queue::QueueSlotId> {
    t.event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect()
}
