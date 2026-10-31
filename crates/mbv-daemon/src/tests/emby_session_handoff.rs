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
use super::emby_session_support::{
    play, play_at, play_last, play_last_calls, play_slot, remove_slot, session_output_with_queue,
    session_presenting_at, timed,
};
use super::r#loop::{TestLoop, test_loop_with_queue};
use super::*;

use mbv_ctrl::OutputSelection;

use crate::cast::PlaybackOutput;
use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use crate::emby_session::poll::EMBY_SESSION_POLL_INTERVAL;
use crate::emby_session::worker::EmbySessionJob;
use mbv_emby::SessionInfo;

/// One second of media in ticks.
const SECOND_TICKS: i64 = mbv_emby_model::TICKS_PER_SECOND;

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
            item_ids: vec!["movie".to_string()],
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

/// Contract: spec "play, pause, next, previous, and stop from either Client
/// act on that receiver" — an explicit Stop stops the device whatever it
/// plays, the same rule the Cast arm applies. Only the output-change stop
/// leaves content the Owner did not load playing (design D11).
#[test]
fn a_user_stop_while_foreign_content_plays_stops_the_device() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls, _rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    script.set_sessions(vec![session_presenting_at(
        "other-item",
        10 * SECOND_TICKS,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        stop_calls(&calls),
        vec![SessionCall::Transport {
            session_id: "session-1".to_string(),
            command: "Stop".to_string(),
        }],
        "the user's stop reaches the device that plays the foreign content"
    );
}

/// The Session entry that presents `item_id` paused at `position_ticks`, so the
/// exit hand-over's position estimate is the observed position itself — a
/// paused Session adds no elapsed time (design D6).
fn session_paused_at(item_id: &str, position_ticks: i64, runtime_ticks: i64) -> SessionInfo {
    SessionInfo {
        is_paused: true,
        ..session_presenting_at(item_id, position_ticks, runtime_ticks)
    }
}

/// The queue's slot ids, in order.
fn queue_slot_ids(t: &TestLoop) -> Vec<mbv_queue::QueueSlotId> {
    t.event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect()
}

/// Contract: row 6.2 / design D11 (the Cast D12 rule) and row 9.5 / design
/// D15 — Owner shutdown stops the output's worker with no device stop, so the
/// Session keeps playing when the Owner exits, and the exit hands the rest of
/// the queue to the device with one `play_last` that interrupts nothing.
#[test]
fn the_owner_shutdown_sends_no_stop_to_the_session() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls, rx) = session_output_with_queue(
        fake,
        vec![
            timed("a", 600 * SECOND_TICKS),
            timed("b", 600 * SECOND_TICKS),
            timed("c", 600 * SECOND_TICKS),
            timed("d", 600 * SECOND_TICKS),
        ],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = queue_slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    script.set_sessions(vec![session_presenting_at(
        "a",
        100 * SECOND_TICKS,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, Instant::now());
    let worker = session_worker(&t);

    let _ = t.event_loop.handle_event(DaemonEvent::Shutdown);
    wait_for_worker(&worker);

    assert_eq!(
        play_last_calls(&calls),
        vec![play_last(&["c", "d"])],
        "the exit appends every playable slot after the queued next item"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a", "b"])],
        "the exit interrupts nothing: the active item is not reloaded"
    );
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

/// Contract: row 9.5 / design D15 and spec "The owner hands the rest of the
/// queue to the Emby Session at exit" — the exit hands the rest of the queue
/// over only while the device plays the loaded item: a queued next item that a
/// queue edit made stale (design D14) reloads the active item at its estimated
/// position with the queue's correct window, and content the Owner did not load
/// takes nothing.
#[rstest]
#[case::a_stale_queued_item_reloads_the_active_item("a", Some(&["a", "c"][..]))]
#[case::foreign_content_takes_nothing("other-item", None)]
fn the_shutdown_hands_over_only_what_the_device_plays(
    #[case] presented: &str,
    #[case] exit_play: Option<&[&str]>,
) {
    let position = 100 * SECOND_TICKS;
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        vec![
            timed("a", 600 * SECOND_TICKS),
            timed("b", 600 * SECOND_TICKS),
            timed("c", 600 * SECOND_TICKS),
        ],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = queue_slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();
    script.set_sessions(vec![session_paused_at("a", position, 600 * SECOND_TICKS)]);
    poll_once(&mut t, t0);
    // The edit removes the slot the device holds next, so the next window check
    // marks the queued item stale (design D14).
    remove_slot(&mut t, client_id, slots[1]);
    script.set_sessions(vec![session_paused_at(
        presented,
        position,
        600 * SECOND_TICKS,
    )]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    let worker = session_worker(&t);

    let _ = t.event_loop.handle_event(DaemonEvent::Shutdown);
    wait_for_worker(&worker);

    let exit_calls: Vec<SessionCall> = play_calls(&calls).into_iter().skip(1).collect();
    assert_eq!(
        exit_calls,
        exit_play.map_or_else(Vec::new, |item_ids| vec![play_at(item_ids, position)]),
        "the exit reloads the active item with the queue's window, or sends nothing"
    );
    assert!(
        play_last_calls(&calls).is_empty(),
        "a stale queued item is never appended after"
    );
    assert!(stop_calls(&calls).is_empty(), "the shutdown sends no stop");
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
    let mut status = t.event_loop.player.status.lock().unwrap();
    status.active = true;
    status.position_ticks = position;
    drop(status);

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

/// Contract: review r6 [P2-2] / design D9 — a pause a hand-off still owed
/// belongs to the item that load replaced: when A is never presented and the
/// user plays B, B starts playing instead of inheriting the pause.
#[test]
fn a_replacing_load_does_not_inherit_a_pending_handoff_pause() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let mut t = test_loop_with_queue(
        DaemonRole::Local,
        vec![
            timed("a", 600 * SECOND_TICKS),
            timed("b", 600 * SECOND_TICKS),
        ],
        0,
    );
    let calls = fake.calls();
    host_session_output(&mut t, fake);
    let slots: Vec<mbv_queue::QueueSlotId> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    local_playing(&mut t, slots[0], 10 * SECOND_TICKS, true);
    let (client_id, reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    wait_for_session_jobs(&t.event_loop.owner.output);
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output after the hand-off");
    };
    assert!(output.pending_pause, "the paused hand-off owes a pause");

    // A is never presented; the user plays B, which replaces the loaded item.
    play_slot(&mut t, client_id, slots[1]);

    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is still the output");
    };
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        Some(slots[1]),
        "the new load replaced the hand-off's item"
    );
    assert!(
        !output.pending_pause,
        "the replaced item's pause does not leak into the new load"
    );

    script.set_sessions(vec![session_presenting_at("b", 0, 600 * SECOND_TICKS)]);
    poll_once(&mut t, Instant::now());
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert!(
        !calls.lock().unwrap().iter().any(|call| matches!(
            call,
            SessionCall::Transport { command, .. } if command == "Pause"
        )),
        "the first sighting of B sends no inherited pause"
    );
}
