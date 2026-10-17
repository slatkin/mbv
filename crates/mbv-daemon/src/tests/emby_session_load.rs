// emby-session-owner-output row 4.3: the loaded item's fate on the Session
// output (design D6, D7). An observation presenting the loaded item settles
// its transition, so a step queued behind the load dispatches; a `play` the
// device refuses, or 30 seconds of observations that never present it, report
// the item once and expire the transition without advancing. The tick drives
// the injected clock, so no test waits.

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::emby_session_output::{
    host_session_output, play_calls, poll_once, pump_session_event, session_selection,
    wait_for_session_jobs,
};
use super::r#loop::{TestLoop, test_loop_with_queue};
use super::*;

use crate::cast::PlaybackOutput;
use crate::emby_session::end::EMBY_SESSION_LOAD_TIMEOUT;
use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use crate::emby_session::poll::EMBY_SESSION_POLL_INTERVAL;
use mbv_emby::SessionInfo;

/// A Session entry that presents `item_id` as its now-playing item: the
/// observation by which the Owner learns a load started (design D6).
fn session_playing(device_id: &str, session_id: &str, item_id: &str) -> SessionInfo {
    SessionInfo {
        now_playing: Some(item_id.to_string()),
        now_playing_item_id: Some(item_id.to_string()),
        ..session(device_id, session_id)
    }
}

/// The `play` call the Session receives for `item_id` (design D7): one item,
/// from the start.
fn play(item_id: &str) -> SessionCall {
    SessionCall::Play {
        session_id: "session-1".to_string(),
        item_id: item_id.to_string(),
        start_ticks: 0,
    }
}

/// Selects the fake's Session as the output over a scripted queue, returning
/// the loop and the fake's call log. The fake stays scriptable through a clone
/// the test keeps.
fn session_output_with_queue(
    fake: FakeSessionApi,
    items: Vec<QueueItem>,
) -> (TestLoop, Arc<Mutex<Vec<SessionCall>>>) {
    let calls = fake.calls();
    let mut t = test_loop_with_queue(DaemonRole::Local, items, 0);
    host_session_output(&mut t, fake);
    let (client_id, reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    (t, calls)
}

/// Plays `slot` through the queue's own play-slot command (the transition an
/// explicit play registers) and lets the Session's job run.
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

/// The time the Session's loaded item was sent, so a test can drive the
/// injected tick clock from it instead of waiting.
fn loaded_at(t: &TestLoop) -> std::time::Instant {
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output
        .loaded
        .as_ref()
        .expect("the Session holds a loaded item")
        .loaded_at
}

/// The `CommandRejected` reports a client received, in order.
fn rejections(rx: &mpsc::Receiver<CtrlOutbound>) -> Vec<String> {
    drained_events(rx)
        .into_iter()
        .filter_map(|event| match event {
            CtrlEvent::CommandRejected(reason) => Some(reason),
            _ => None,
        })
        .collect()
}

/// Contract: row 4.3 / design D6 and D7 — an observation that presents the
/// loaded item settles its transition, so a step queued behind the load is
/// dispatched: two quick Next commands end on one final `play` two slots ahead
/// of the queue's first slot, instead of staying stuck behind the first load.
#[test]
fn two_quick_next_steps_settle_into_one_final_play_two_slots_ahead() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls) = session_output_with_queue(
        fake,
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
            emby_qi("d", "Video", "Movie"),
        ],
    );
    let (client_id, _reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot_b = t.event_loop.owner.core.queue.slots()[1].slot_id;
    let slot_c = t.event_loop.owner.core.queue.slots()[2].slot_id;
    play_slot(&mut t, client_id, slot_b);
    assert_eq!(play_calls(&calls), vec![play("b")], "B is loading");

    // Two quick Next commands: the first steps from the in-flight B and queues
    // C, the second coalesces into that step.
    for _ in 0..2 {
        t.event_loop
            .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Step(
                mbv_ctrl::Direction::Next,
            )));
    }
    wait_for_session_jobs(&t.event_loop.owner.output);
    assert_eq!(
        play_calls(&calls),
        vec![play("b")],
        "a step queued behind the unsettled load loads nothing yet"
    );

    // The Session presents B: the load settles and the queued step loads C.
    script.set_sessions(vec![session_playing("device-1", "session-1", "b")]);
    poll_once(&mut t, std::time::Instant::now());
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play("b"), play("c")],
        "the final play is the slot two ahead of the queue's first slot"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        Some(slot_c),
        "the promoted step's load is the Session's loaded item"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slot_c),
        "the queue's active marker follows the final play"
    );
    assert_eq!(
        t.event_loop
            .owner
            .core
            .transitions
            .in_flight()
            .map(|t| t.target),
        Some(slot_c),
        "the promoted step is in flight until the Session shows C"
    );
}

/// Contract: row 4.3 / design D6 and spec "The owner infers a load failure on
/// an Emby Session" — a load no observation presents within 30 seconds is
/// reported once and advances nothing. The tick drives the injected clock, so
/// no test waits.
#[test]
fn a_load_the_session_never_presents_is_reported_once_and_advances_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let (mut t, calls) = session_output_with_queue(
        fake,
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
    );
    let (client_id, _reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot_a = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot_a);
    let _ = drained_events(&rx);
    let revision_before = t.event_loop.owner.core.queue.revision();
    let loaded_at = loaded_at(&t);

    t.event_loop.tick(loaded_at + EMBY_SESSION_LOAD_TIMEOUT);

    let reports = rejections(&rx);
    assert_eq!(reports.len(), 1, "the failure is reported once");
    assert!(
        reports[0].contains('a') && reports[0].contains("did not start on the Emby Session"),
        "the report names the item and the Session: {}",
        reports[0]
    );
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "the timed-out load expires its transition"
    );

    // A later tick reports nothing more: the load is already reported.
    t.event_loop
        .tick(loaded_at + EMBY_SESSION_LOAD_TIMEOUT + EMBY_SESSION_POLL_INTERVAL);

    assert!(
        rejections(&rx).is_empty(),
        "the reported load is not reported again"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.revision(),
        revision_before,
        "the timeout changes no queue revision"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slot_a),
        "the timeout advances nothing"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play("a")],
        "the timeout loads no other slot"
    );
}

/// Contract: row 4.3 / design D6 and D7 — a `play` job the device refuses
/// expires the load's transition at once and reports the item once, so the
/// one-in-flight gate cannot stay wedged behind it.
#[test]
fn a_refused_play_expires_the_load_and_reports_the_item_once() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, _calls) = session_output_with_queue(
        fake,
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
    );
    let (client_id, _reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot_a = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let _ = drained_events(&rx);
    script.fail_with("the device is busy");

    play_slot(&mut t, client_id, slot_a);
    let event = t
        .merged_rx
        .recv()
        .expect("the refused play reports back to the loop");
    t.event_loop.handle_event(event);

    let reports = rejections(&rx);
    assert_eq!(reports.len(), 1, "the refused play is reported once");
    assert!(
        reports[0].contains('a') && reports[0].contains("the device is busy"),
        "the report names the item and the device's error: {}",
        reports[0]
    );
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "the refused play expires its transition"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert!(
        output
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.failure_reported),
        "the load is marked as reported, so no later tick repeats it"
    );
}
