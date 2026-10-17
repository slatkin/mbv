// emby-session-owner-output rows 6.1 and 6.2: the hand-off between Local and
// an Emby Session (design D11) with the pending pause of design D9, and the
// stop of the old output and of Owner shutdown (design D11). Every test drives
// the real selection and poll paths against the recording `SessionApi` fake,
// with injected poll times, so no test waits or reaches a network.

use std::time::Instant;

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::emby_session_output::{
    host_session_output, inject_lookup_outcome, loop_with_injected_lookups, play_calls, poll_once,
    pump_session_event, session_selection, wait_for_session_jobs,
};
use super::r#loop::{TestLoop, test_loop_with_queue};
use super::*;

use mbv_ctrl::OutputSelection;

use crate::cast::PlaybackOutput;
use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use crate::emby_session::worker::EmbySessionJob;
use mbv_emby::SessionInfo;

/// One second of media in ticks.
const SECOND_TICKS: i64 = mbv_emby_model::TICKS_PER_SECOND;

/// An Emby queue item with a runtime, so the position estimate is capped and
/// the captured hand-off position is predictable.
fn timed(name: &str, runtime_ticks: i64) -> QueueItem {
    let mut emby = item(name, "Video", "Movie");
    emby.runtime_ticks = runtime_ticks;
    QueueItem::Emby(Box::new(emby))
}

/// Stamps the Local output as playing `slot` at `position_ticks`, in the pause
/// state `paused` — the presented state a live local run produces, and the
/// state a Local hand-off captures.
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

/// The Session entry that presents `item_id` at `position_ticks`.
fn session_presenting_at(item_id: &str, position_ticks: i64, runtime_ticks: i64) -> SessionInfo {
    SessionInfo {
        now_playing: Some(item_id.to_string()),
        now_playing_item_id: Some(item_id.to_string()),
        now_playing_item_type: Some("Movie".to_string()),
        position_ticks,
        runtime_ticks,
        ..session("device-1", "session-1")
    }
}

/// Selects the fake's Session as the output over a scripted queue, returning
/// the loop, the fake's call log, and the selecting Client's event receiver.
fn session_output_with_queue(
    fake: FakeSessionApi,
    items: Vec<QueueItem>,
) -> (
    TestLoop,
    Arc<Mutex<Vec<SessionCall>>>,
    mpsc::Receiver<CtrlOutbound>,
) {
    let calls = fake.calls();
    let mut t = test_loop_with_queue(DaemonRole::Local, items, 0);
    host_session_output(&mut t, fake);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    (t, calls, rx)
}

/// Plays `slot` through the queue's own play-slot command and lets the
/// Session's job run.
fn play_slot(t: &mut TestLoop, client_id: u64, slot: mbv_queue::QueueSlotId) {
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);
}

/// The Session worker's sender, kept after an output swap so a test can wait
/// for the jobs the stop queued.
fn session_worker(t: &TestLoop) -> mpsc::Sender<EmbySessionJob> {
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output.worker.clone()
}

/// Waits until every job queued on the Session worker has run, so a test can
/// assert on the fake's call log without a sleep. Takes the sender, because
/// the stop a hand-off queues runs after the output has swapped already.
fn wait_for_worker(worker: &mpsc::Sender<EmbySessionJob>) {
    let (done_tx, done_rx) = mpsc::channel();
    worker
        .send(Box::new(move |_api| {
            let _ = done_tx.send(());
        }))
        .expect("the Session worker accepts the sentinel");
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the queued jobs ran before the sentinel");
}

/// The `Stop` transport commands the fake recorded, in order.
fn stop_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| {
            matches!(
                call,
                SessionCall::Transport { command, .. } if command == "Stop"
            )
        })
        .cloned()
        .collect()
}

/// Contract: row 6.1 / design D11 and spec "An Emby Session is selected while
/// local playback runs" — a Local hand-off gives the Session a `play` of the
/// captured slot's item at the local position, never at the start.
#[test]
fn a_local_handoff_plays_the_session_at_the_local_position() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let mut t = test_loop_with_queue(
        DaemonRole::Local,
        vec![timed("movie", 600 * SECOND_TICKS)],
        0,
    );
    let calls = fake.calls();
    host_session_output(&mut t, fake);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let position = 42 * SECOND_TICKS;
    local_playing(&mut t, slot, position, false);
    let (client_id, reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![SessionCall::Play {
            session_id: "session-1".to_string(),
            item_id: "movie".to_string(),
            start_ticks: position,
        }],
        "the Session is given the handed-off item at the local position"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output after the hand-off");
    };
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        Some(slot),
        "the captured slot is the loaded one"
    );
}

/// Contract: row 6.1 / design D9 — a paused Local hand-off cannot start paused,
/// so the first observation that shows the loaded item sends `Pause` once.
#[test]
fn a_paused_local_handoff_pauses_the_session_after_the_first_sighting() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let mut t = test_loop_with_queue(
        DaemonRole::Local,
        vec![timed("movie", 600 * SECOND_TICKS)],
        0,
    );
    let calls = fake.calls();
    host_session_output(&mut t, fake);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let position = 10 * SECOND_TICKS;
    local_playing(&mut t, slot, position, true);
    let (client_id, reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    wait_for_session_jobs(&t.event_loop.owner.output);

    // The device starts the item playing: a `play` job cannot begin paused.
    script.set_sessions(vec![session_presenting_at(
        "movie",
        position,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        calls.lock().unwrap().last(),
        Some(&SessionCall::Transport {
            session_id: "session-1".to_string(),
            command: "Pause".to_string(),
        }),
        "the first sighting of the loaded item pauses it"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert!(!output.pending_pause, "the captured pause is spent");
}

/// Contract: row 6.1 / design D11 and spec "A device that is not listed keeps
/// the old output" — a Session the lookup cannot find leaves local playback
/// running and reports the reason.
#[test]
fn a_device_that_is_not_listed_leaves_local_playback_running() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-other", "session-1")]);
    let mut t = test_loop_with_queue(
        DaemonRole::Local,
        vec![timed("movie", 600 * SECOND_TICKS)],
        0,
    );
    host_session_output(&mut t, fake);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let position = 42 * SECOND_TICKS;
    local_playing(&mut t, slot, position, false);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);

    assert!(
        matches!(recv_event(&rx), CtrlEvent::CommandRejected(reason) if reason.contains("device-1")),
        "the rejection names the device that is not listed"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the old output stays"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active && status.position_ticks == position,
        "local playback keeps running untouched"
    );
}

/// Contract: row 6.1 / design D11 — a Session→Local hand-off cold-starts
/// locally at the Session's estimated position, so the item continues where
/// the device played it.
#[test]
fn an_emby_session_hand_back_cold_starts_locally_at_the_estimated_position() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, _calls, _rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot);
    let position = 100 * SECOND_TICKS;
    script.set_sessions(vec![session_presenting_at(
        "movie",
        position,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());

    select_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );

    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the Session hands playback back to local"
    );
    let resume = mbv_player::resume_ticks_for_slot(&t.event_loop.owner.core.queue, slot)
        .expect("the handed-off slot has a resume position");
    assert!(
        (position..position + SECOND_TICKS).contains(&resume),
        "the local run resumes at the Session's estimated position: {resume} vs {position}"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(status.active, "the local run continues the handed-off slot");
}

/// Contract: row 6.2 / design D11 and spec "Cast target is selected while a
/// session is watched" — selecting Local while the device plays the loaded
/// item sends exactly one `Stop` to it.
#[test]
fn selecting_local_while_the_loaded_item_plays_sends_one_stop() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls, _rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot);
    script.set_sessions(vec![session_presenting_at(
        "movie",
        100 * SECOND_TICKS,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());
    let worker = session_worker(&t);

    select_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );
    wait_for_worker(&worker);

    assert_eq!(
        stop_calls(&calls),
        vec![SessionCall::Transport {
            session_id: "session-1".to_string(),
            command: "Stop".to_string(),
        }],
        "the stopped old output sends exactly one device stop"
    );
}

/// Contract: row 6.2 / design D11 — stopping the old output leaves content the
/// Owner did not load playing, so it sends no `Stop`.
#[test]
fn selecting_local_while_foreign_content_plays_sends_no_stop() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls, _rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot);
    script.set_sessions(vec![session_presenting_at(
        "other-item",
        10 * SECOND_TICKS,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());
    let worker = session_worker(&t);

    select_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );
    wait_for_worker(&worker);

    assert!(
        stop_calls(&calls).is_empty(),
        "content the Owner did not load is left playing"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the output still swaps to Local"
    );
}

/// Contract: row 6.2 / design D11 (the Cast D12 rule) — Owner shutdown stops
/// the output's worker with no device stop, so the Session keeps playing when
/// the Owner exits.
#[test]
fn the_owner_shutdown_sends_no_stop_to_the_session() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls, rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot);
    script.set_sessions(vec![session_presenting_at(
        "movie",
        100 * SECOND_TICKS,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());
    let worker = session_worker(&t);

    let _ = t.event_loop.handle_event(DaemonEvent::Shutdown);
    wait_for_worker(&worker);

    assert!(
        stop_calls(&calls).is_empty(),
        "the shutdown stops neither the device playback nor its app"
    );
    assert!(
        !drained_events(&rx)
            .into_iter()
            .any(|event| matches!(event, CtrlEvent::CommandRejected(_))),
        "the shutdown reports no failure"
    );
}

/// Contract: review r6 [P2-1] / design D11 — local playback that starts after
/// an Emby Session selection captured no hand-off is not discarded: the
/// resolved selection is rejected and local playback keeps running.
#[test]
fn playback_started_while_the_session_lookup_ran_is_not_discarded() {
    let mut t = loop_with_injected_lookups();
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    // The select-time capture found local playback idle; the user starts it
    // while the device lookup runs.
    let position = 42 * SECOND_TICKS;
    {
        let mut status = t.event_loop.player.status.lock().unwrap();
        status.active = true;
        status.position_ticks = position;
    }

    inject_lookup_outcome(&mut t, "device-1", "session-1");

    assert!(
        matches!(recv_event(&rx), CtrlEvent::CommandRejected(reason) if reason.contains("playback started")),
        "the resolved selection is rejected with the reason"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the old output stays"
    );
    let status = t.event_loop.player.status.lock().unwrap();
    assert!(
        status.active && status.position_ticks == position,
        "the just-started local playback keeps running"
    );
}
