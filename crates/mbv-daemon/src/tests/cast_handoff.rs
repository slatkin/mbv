// cast-owner-output row 7.1 dispatch A: the Local→Cast hand-off (design
// D10). A selection while Local plays captures the active slot, connects
// first, stops the Local run only after the connect succeeded, and loads the
// slot on the receiver at the captured position, paused when it was paused;
// a connect failure leaves local playback running.

use super::*;

use mbv_cast::{CastStatus, ConnectMode};
use mbv_ctrl::{OutputSelection, OutputState};

use super::cast_output::{
    cast_selection, connect_output_client, failing_connector, fake_connector, pump_cast_event,
    select_output,
};
use super::cast_playback::{feed_qi_with_url, playing_status_at, pump, select_cast_output};
use super::cast_resolve::wait_for_worker_jobs;
use super::r#loop::{TestLoop, test_loop_with_queue};
use crate::cast::PlaybackOutput;

/// Stamps the Local output as playing `slot` at `position_ticks`, in the
/// pause state `paused` — the presented state a live local run produces.
fn local_playing(
    t: &mut TestLoop,
    slot: mbv_queue::QueueSlotId,
    position_ticks: i64,
    paused: bool,
) {
    t.event_loop
        .owner
        .core
        .note_observed_active_slot(Some(slot));
    let mut status = t.event_loop.player.status.lock().unwrap();
    status.active = true;
    status.position_ticks = position_ticks;
    status.paused = paused;
}

/// Selects the test receiver and pumps the connect outcome through the
/// hand-off's resolve, then waits for the load jobs to run. Returns the
/// transport's call log.
fn select_receiver_with_local_playing(
    t: &mut TestLoop,
    position_ticks: i64,
    paused: bool,
) -> Arc<Mutex<Vec<String>>> {
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    local_playing(t, slot, position_ticks, paused);
    let (connector, calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(t, client_id, &reply_tx, cast_selection());
    pump_cast_event(t);
    pump(t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    calls
}

/// Contract: cast-owner-output row 7.1 dispatch A (design D10) — selecting a
/// receiver while Local plays at a position connects, stops the local run,
/// and gives the receiver a `load` of the handed-off slot's media at that
/// position, not at the start.
#[test]
fn a_handoff_loads_the_captured_slot_on_the_receiver_at_the_local_position() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let position = 42 * mbv_emby_model::TICKS_PER_SECOND;

    let calls = select_receiver_with_local_playing(&mut t, position, false);

    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output after the hand-off");
    };
    let loaded = cast.loaded.as_ref().expect("the hand-off loaded the slot");
    assert_eq!(loaded.slot, slot, "the captured slot is the loaded one");
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "keep_alive".to_string(),
            "load(https://feed/a.mp3, 42)".to_string()
        ],
        "the receiver is given the handed-off media at the local position"
    );
}

/// Contract: cast-owner-output row 7.1 dispatch A (design D10 step 5) — a
/// paused Local playback carries over: the receiver load is followed by an
/// explicit pause, so the receiver holds the item paused at the position.
#[test]
fn a_paused_handoff_pauses_the_receiver_after_the_load() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let position = 42 * mbv_emby_model::TICKS_PER_SECOND;

    let calls = select_receiver_with_local_playing(&mut t, position, true);

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "keep_alive".to_string(),
            "load(https://feed/a.mp3, 42)".to_string(),
            "pause".to_string()
        ],
        "the captured pause applies after the load, in order"
    );
}

/// Contract: cast-owner-output row 7.1 dispatch A (design D10 step 2) — a
/// connect failure keeps the old output untouched: local playback keeps
/// running at its position, and the selecting Client is told the reason.
#[test]
fn a_failed_connect_leaves_local_playback_running() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    local_playing(&mut t, slot, 42 * mbv_emby_model::TICKS_PER_SECOND, false);
    t.event_loop.cast_connector = Some(failing_connector("receiver unreachable"));
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("a failed connect reports to the selecting Client");
    };
    assert!(reason.contains("receiver unreachable"), "reason: {reason}");
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the old output stays"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active && status.position_ticks == 42 * mbv_emby_model::TICKS_PER_SECOND,
        "local playback keeps running untouched"
    );
}

/// Contract: cast-owner-output row 7.1 dispatch A — the hand-off selection
/// still connects in first-select mode (design D11): a first selection by
/// the user launches the receiver app.
#[test]
fn a_handoff_selection_connects_in_first_select_mode() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    local_playing(&mut t, slot, 0, false);
    let modes = Arc::new(Mutex::new(Vec::new()));
    let mode_log = Arc::clone(&modes);
    t.event_loop.cast_connector =
        Some(Box::new(move |request: crate::cast::CastConnectRequest| {
            mode_log.lock().unwrap().push(request.mode);
            let transport = mbv_cast::FakeCastTransport {
                calls: Arc::new(Mutex::new(Vec::new())),
                status: None,
            };
            let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
            let _ = request
                .reply
                .send(crate::cast::CastConnectOutcome::Connected(worker));
        }));
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        *modes.lock().unwrap(),
        vec![ConnectMode::FirstSelect],
        "the user's first selection launches in first-select mode"
    );
}

/// Loads `slot` on the selected receiver through the ctrl play-slot command
/// and waits for the resolve and the load job to run.
fn load_slot_on_receiver(t: &mut TestLoop, client_id: u64, slot: mbv_queue::QueueSlotId) {
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
}

/// A selection of a second receiver, distinct from the receiver the shared
/// helpers select (`rcv-1`).
fn second_cast_selection() -> OutputSelection {
    OutputSelection::Cast {
        receiver_id: "rcv-2".to_string(),
        name: "Kitchen".to_string(),
        host: "192.0.2.11".to_string(),
        port: 8009,
    }
}

/// Sets the first receiver up as the output presenting the queue's first
/// slot: it is selected, the slot is loaded on it, and one tick runs the
/// status poll whose report presents the loaded item.
fn receiver_presenting(t: &mut TestLoop, status: CastStatus) -> PresentedReceiver {
    let (client_id, _rx, calls) = select_cast_output(t, status);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(t, client_id, slot);
    // One tick runs the status poll, so the output presents the receiver's
    // state for the loaded item before the hand-off.
    t.event_loop.tick(std::time::Instant::now());
    pump(t);
    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    PresentedReceiver {
        worker: cast.worker.clone(),
        calls,
    }
}

/// Drains a Client's event channel, so the events a later step emits are the
/// next ones the channel holds.
fn drain(rx: &mpsc::Receiver<CtrlOutbound>) {
    while rx.try_recv().is_ok() {}
}

/// The handles a receiver set up with `receiver_presenting` gives the test:
/// the receiver's worker sender and its transport call log.
struct PresentedReceiver {
    worker: mpsc::Sender<mbv_cast::CastJob>,
    calls: Arc<Mutex<Vec<String>>>,
}

/// Holds the Cast worker behind a blocker job, so the jobs the Local
/// selection queues (media stop, teardown) run only after the test releases
/// it. Returns the release sender and the worker sender — still valid after
/// the output swapped to Local, so the test can queue a sentinel job behind
/// the select's jobs and learn when they have all run.
fn hold_cast_worker(t: &TestLoop) -> (mpsc::Sender<()>, mpsc::Sender<mbv_cast::CastJob>) {
    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    let worker = cast.worker.clone();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let _ = worker.send(Box::new(move |_| {
        let _ = release_rx.recv();
    }));
    (release_tx, worker)
}

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
            connection: mbv_ctrl::CastConnection::Connected,
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
            connection: mbv_ctrl::CastConnection::Connected,
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

/// Contract: cast-owner-output row 7.1 dispatch C — selecting the receiver
/// that is already the output is rejected as no hand-off: the output is
/// already where the selection asks it to be, and its playback is untouched.
#[test]
fn selecting_the_receiver_that_is_already_the_output_is_rejected() {
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
    // A second Client issues the same-receiver selection; its reply path is
    // its own channel.
    let (selector_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    drain(&rx);

    select_output(
        &mut t,
        selector_id,
        &reply_tx,
        super::cast_output::cast_selection(),
    );

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("a same-receiver selection is rejected");
    };
    assert!(
        reason.contains("already the playback output"),
        "reason: {reason}"
    );
    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver stays the output");
    };
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: mbv_ctrl::CastConnection::Connected,
        },
        "the output is unchanged"
    );
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
            connection: mbv_ctrl::CastConnection::Connected,
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
            connection: mbv_ctrl::CastConnection::Connected,
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
