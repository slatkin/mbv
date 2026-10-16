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
