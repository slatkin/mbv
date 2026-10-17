// emby-session-owner-output row 5.2: the end table of design D6 on the Emby
// Session output. Emby reports no end reason, so the Owner infers the end from
// an idle Session after a sighting of the loaded item: near the runtime it
// records the slot's completion and steps the shared `OutputAdvance`, and
// earlier it presents nothing and advances nothing. A step target the Session
// cannot play is reported and stepped past. The polls are driven with their own
// injected times, so no test waits.

use std::time::Instant;

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::emby_session_output::{
    host_session_output, play_calls, poll_once, pump_session_event, session_selection,
    wait_for_session_jobs,
};
use super::r#loop::{TestLoop, test_loop_with_queue};
use super::*;

use crate::cast::PlaybackOutput;
use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use crate::emby_session::poll::EMBY_SESSION_POLL_INTERVAL;
use mbv_emby::SessionInfo;

/// One second of media in ticks.
const SECOND_TICKS: i64 = mbv_emby_model::TICKS_PER_SECOND;

/// The 600 seconds of media every scripted item carries.
const RUNTIME_TICKS: i64 = 600 * SECOND_TICKS;

/// An Emby queue item with a runtime, so the end rule has one to compare the
/// position estimate with.
fn timed(name: &str, runtime_ticks: i64) -> QueueItem {
    let mut emby = item(name, "Video", "Movie");
    emby.runtime_ticks = runtime_ticks;
    QueueItem::Emby(Box::new(emby))
}

/// The Session entry that presents the loaded item at `position_ticks` of
/// `runtime_ticks` — the sighting that makes an idle Session an end.
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

/// The Session entry of a device that presents nothing: the idle observation
/// the end table reads.
fn idle_session() -> SessionInfo {
    session("device-1", "session-1")
}

/// The one `play` call the Session receives for `item_id`, from the start.
fn play(item_id: &str) -> SessionCall {
    SessionCall::Play {
        session_id: "session-1".to_string(),
        item_ids: vec![item_id.to_string()],
        start_ticks: 0,
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

/// The queue's slot ids, in order.
fn slot_ids(t: &TestLoop) -> Vec<mbv_queue::QueueSlotId> {
    t.event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect()
}

/// Whether the loaded item's end was already acted on (design D6).
fn end_handled(t: &TestLoop) -> bool {
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output
        .loaded
        .as_ref()
        .is_some_and(|loaded| loaded.end_handled)
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

/// Contract: row 5.2 / design D6 and spec "The owner infers a natural end on
/// an Emby Session" — an idle Session 3 seconds before the runtime records the
/// ended slot's completion and loads the next slot, and the same idle seen in
/// a later poll advances nothing.
#[test]
fn the_same_idle_near_the_end_in_two_polls_plays_the_next_slot_once() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        vec![timed("a", RUNTIME_TICKS), timed("b", RUNTIME_TICKS)],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play("a"), play("b")],
        "the inferred end loads the next slot of the play order"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[1]),
        "the step moves the queue's active marker to the next slot"
    );

    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play("a"), play("b")],
        "the same idle in a later poll advances nothing"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        2,
        "the recorded completion consumes nothing by default"
    );
}

/// Contract: row 5.2 / design D6 and spec "User stops an Emby Session early" —
/// an idle Session 300 seconds before the runtime is a stop on the device: it
/// presents no active playback and advances nothing.
#[test]
fn an_idle_thirty_seconds_before_the_end_advances_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        vec![timed("a", RUNTIME_TICKS), timed("b", RUNTIME_TICKS)],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS / 2,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);

    assert_eq!(
        play_calls(&calls),
        vec![play("a")],
        "an early stop loads nothing"
    );
    assert!(end_handled(&t), "the early stop still records the end once");
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "an early stop leaves the active slot where it was"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "an idle Session presents no active playback"
    );
}

/// Contract: row 5.2 review r5 / design D6's last row — a Session that presents
/// an item the Owner did not load ends nothing. The idle that follows such an
/// item is that item's own stop: it records no completion for the loaded slot
/// and advances nothing, so the loaded item's resume position survives.
#[test]
fn an_idle_after_foreign_content_completes_and_advances_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        vec![timed("a", RUNTIME_TICKS), timed("b", RUNTIME_TICKS)],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let revision_before = t.event_loop.owner.core.queue.revision();
    let t0 = Instant::now();

    // The loaded item is seen playing, well before its end.
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS / 2,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    // Someone else plays another item on the device to its own end.
    script.set_sessions(vec![session_presenting_at(
        "f",
        RUNTIME_TICKS - SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);

    assert_eq!(
        play_calls(&calls),
        vec![play("a")],
        "another item's end loads nothing"
    );
    assert!(
        !end_handled(&t),
        "the loaded item has no end from another item's stop"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "another item's end moves no active slot"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.revision(),
        revision_before,
        "no completion is recorded for the loaded slot"
    );
    let played = t
        .event_loop
        .owner
        .core
        .queue
        .slot(slots[0])
        .and_then(|slot| slot.item.as_emby())
        .map(|item| item.played)
        .expect("the loaded slot is still queued");
    assert!(!played, "the loaded slot's resume state is untouched");
}

/// Contract: row 5.2 / design D6 and spec "Idle before the item starts" — an
/// idle Session before the loaded item was ever presented is not an end, so it
/// records no end and advances nothing.
#[test]
fn an_idle_before_the_first_sighting_advances_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        vec![timed("a", RUNTIME_TICKS), timed("b", RUNTIME_TICKS)],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0);

    assert_eq!(
        play_calls(&calls),
        vec![play("a")],
        "an idle Session before the first sighting loads nothing"
    );
    assert!(
        !end_handled(&t),
        "a load the Session never showed has no end to record"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "the wait leaves the active slot where it was"
    );
}

/// Contract: row 5.2 / design D6, D7 and spec "Feed slot is next" — a step
/// target the Session cannot play is reported by name and reason, and the next
/// target is picked in the same advance, so the Emby slot after it is loaded.
#[test]
fn a_feed_next_slot_is_reported_and_the_emby_slot_after_it_is_loaded() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, rx) = session_output_with_queue(
        fake,
        vec![
            timed("a", RUNTIME_TICKS),
            video_feed_qi("episode-1"),
            timed("b", RUNTIME_TICKS),
        ],
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let _ = drained_events(&rx);
    let t0 = Instant::now();

    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    let reports = rejections(&rx);
    assert_eq!(reports.len(), 1, "the stepped-over slot is reported once");
    assert!(
        reports[0].contains("episode-1") && reports[0].contains("plays only Emby items"),
        "the report names the Feed entry and the reason: {}",
        reports[0]
    );
    assert_eq!(
        play_calls(&calls),
        vec![play("a"), play("b")],
        "the advance steps past the Feed entry and loads the Emby slot after it"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        Some(slots[2]),
        "the loaded item is the step's second target"
    );
}

/// Contract: row 5.2 / design D6 and spec "Session reports no runtime" — a
/// queue item with no runtime takes the runtime the Session reports, so an
/// idle Session near that runtime is still the item's natural end.
#[test]
fn a_queue_item_with_no_runtime_uses_the_session_runtime() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) =
        session_output_with_queue(fake, vec![timed("a", 0), timed("b", RUNTIME_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play("a"), play("b")],
        "the Session's runtime stands in for the queue item's absent one"
    );
}
