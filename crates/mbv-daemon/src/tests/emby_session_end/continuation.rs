// emby-session-output-followups rows 3.1 to 3.4: the queue-end continuation
// (design D4). When the tracked item ends at the queue's edge and the device
// starts an item of its own, the Owner looks that item up once and adopts it
// as the queue's next slot: appended at the tail, active, and tracked. The
// polls are driven with their own injected times, so no test waits. The
// cases the adoption leaves alone live in `no_adoption.rs`.

use super::*;

use super::super::emby_session_output::pump_session_event;
use super::super::emby_session_support::{play_last, play_last_calls};

mod no_adoption;

/// The canned item the fake's `get_item` returns for the device's own
/// autoplay, with the runtime the tracked item's slot carries.
fn canned_autoplay_item() -> mbv_emby_model::EmbyItem {
    let mut canned = item("f", "Video", "Movie");
    canned.runtime_ticks = RUNTIME_TICKS;
    canned
}

/// The item ids the fake's `get_item` was called with, in order.
fn get_item_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<String> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter_map(|call| match call {
            SessionCall::GetItem { item_id } => Some(item_id.clone()),
            _ => None,
        })
        .collect()
}

/// Whether the slot's Emby item was recorded as played (design D6, D14).
fn slot_played(t: &TestLoop, slot: mbv_queue::QueueSlotId) -> bool {
    t.event_loop
        .owner
        .core
        .queue
        .slot(slot)
        .and_then(|slot| slot.item.as_emby())
        .map(|item| item.played)
        .expect("the slot is still queued")
}

/// The adopted slot's state on the output: the loaded item and the cleared
/// continuation.
fn adopted_output(t: &TestLoop, slot: mbv_queue::QueueSlotId, item_id: &str) {
    let crate::cast::PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output
            .loaded
            .as_ref()
            .map(|loaded| (loaded.slot, loaded.item_id.as_str())),
        Some((slot, item_id)),
        "the adopted item is the tracked item"
    );
    assert!(
        output.continuation.is_none(),
        "the adoption spends the pending continuation"
    );
}

/// The adoption contract every path shares (design D4): `lookups` ran in
/// order, no device command went out beyond the initial load — the item
/// already plays — the item's slot was appended at the queue's tail and made
/// the active and the tracked slot, and `ended` recorded its completion.
/// Returns the adopted slot.
fn assert_adopted(
    t: &TestLoop,
    calls: &Arc<Mutex<Vec<SessionCall>>>,
    lookups: &[String],
    tail: usize,
    item_id: &str,
    ended: mbv_queue::QueueSlotId,
) -> mbv_queue::QueueSlotId {
    assert_eq!(
        get_item_calls(calls),
        lookups,
        "each autoplay submits one item lookup"
    );
    assert_eq!(
        play_calls(calls),
        vec![play(&["a"])],
        "the adoption sends no device command: the item already plays"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        tail + 1,
        "the adopted item is appended at the queue's tail"
    );
    let adopted = t.event_loop.owner.core.queue.slots()[tail].slot_id;
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[tail].item.id(),
        item_id,
        "the appended slot is the item the device started"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(adopted),
        "the adopted slot becomes the active slot"
    );
    assert!(
        slot_played(t, ended),
        "the ended slot records its completion"
    );
    adopted_output(t, adopted, item_id);
    adopted
}

/// Contract: row 3.2 / followups design D4, direct path, and spec scenario
/// "The device autoplays its next episode at the queue's end" — the
/// observation after the tracked item's sighting presents the device's own
/// next item: one lookup adopts it as the queue's tail slot, active and
/// tracked, the ended slot records its completion, and the adoption persists
/// the owner queue.
#[test]
fn the_queue_end_autoplay_is_adopted() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    let (_client_id, ended) = adopt_the_direct_autoplay(&mut t, &script, Instant::now());

    let persisted_before = t.persisted.borrow().len();
    pump_session_event(&mut t);

    assert_adopted(&t, &calls, &["f".to_string()], 1, "f", ended);
    assert_persisted_adoption(&t, persisted_before, &["a".to_string(), "f".to_string()]);
}

/// Contract: row 3.2 / followups design D4, idle-gap path, and spec scenario
/// "The device autoplays after an idle gap" — the idle observation ends the
/// tracked item at the queue's edge and records its completion once; the
/// first foreign item within 30 seconds is adopted as in the direct case,
/// and neither the detection nor the adoption records a second completion.
#[test]
fn the_autoplay_after_an_idle_gap_is_adopted_with_one_completion() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    let (_client_id, ended) = playing_queue_with_canned_autoplay(&mut t, &script);
    let t0 = Instant::now();

    // The sighting of the tracked item, then the idle observation one second
    // later: the natural end records the completion and opens the window.
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    assert!(
        slot_played(&t, ended),
        "the natural end recorded the completion"
    );

    // The device starts its own item within the 30-second window.
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    let persisted_before = t.persisted.borrow().len();
    pump_session_event(&mut t);

    assert_adopted(&t, &calls, &["f".to_string()], 1, "f", ended);
    assert_persisted_adoption(&t, persisted_before, &["a".to_string(), "f".to_string()]);
}

/// The prologue the queue-end adoption tests share: the canned autoplay item
/// is scripted, a client is connected, and the queue's one slot is playing.
/// Returns the client id and the tracked slot the device ends.
fn playing_queue_with_canned_autoplay(
    t: &mut TestLoop,
    script: &FakeSessionApi,
) -> (u64, mbv_queue::QueueSlotId) {
    script.set_item(canned_autoplay_item());
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(t);
    play_slot(t, client_id, slots[0]);
    (client_id, slots[0])
}

/// Drives the direct adoption the follow-up tests build on: the queue's one
/// slot is playing (the shared prologue), and the device starts the canned
/// autoplay item straight after its sighting — no idle in between. The
/// lookup's job has run when this returns; the caller pumps the result event
/// itself, so a test can snapshot persistence first.
fn adopt_the_direct_autoplay(
    t: &mut TestLoop,
    script: &FakeSessionApi,
    t0: Instant,
) -> (u64, mbv_queue::QueueSlotId) {
    let (client_id, ended) = playing_queue_with_canned_autoplay(t, script);
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(t, t0);
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    (client_id, ended)
}

/// The adoption's persistence contract (design D4): the Session event that
/// lands the lookup result is the one queue change it persists, and the
/// persisted queue holds the adopted tail slot.
fn assert_persisted_adoption(t: &TestLoop, persisted_before: usize, item_ids: &[String]) {
    assert_eq!(
        t.persisted.borrow().len(),
        persisted_before + 1,
        "the adoption is the one queue change the Session event persists"
    );
    assert_eq!(
        t.persisted.borrow()[persisted_before].item_ids(),
        item_ids,
        "the persisted queue holds the adopted slot"
    );
}

/// The transport commands the Session received, in order.
fn transport_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| matches!(call, SessionCall::Transport { .. }))
        .cloned()
        .collect()
}

/// Contract: row 3.3 / followups design D4 and spec scenario "The adopted
/// item is tracked like a loaded item" — the adopted slot's item is the
/// tracked item the Session's tables read: its presented position is the
/// Session's own report, its end is inferred from the same margin, so the
/// device's next autoplay is adopted again ("Adoption repeats") and the
/// ended adopted slot records its completion.
#[test]
fn an_adopted_item_is_tracked_like_a_loaded_item_and_adoption_repeats() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    script.set_item(canned_autoplay_item());
    let t0 = Instant::now();
    let (_client_id, ended) = adopt_the_direct_autoplay(&mut t, &script, t0);
    pump_session_event(&mut t);
    let first_adopted = assert_adopted(&t, &calls, &["f".to_string()], 1, "f", ended);

    // The adopted item plays like a loaded item: the Session's own position
    // is the presented one, and its sighting near the runtime arms the end
    // rule for the device's next autoplay.
    let mut tracked = item("g", "Video", "Movie");
    tracked.runtime_ticks = RUNTIME_TICKS;
    script.set_item(tracked);
    script.set_sessions(vec![session_presenting_at(
        "f",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    assert_eq!(
        t.event_loop.player.status.lock().unwrap().position_ticks,
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        "the adopted item's presented position is the Session's own report"
    );

    // The device starts its own next item at the adopted item's end: the
    // adoption repeats.
    script.set_sessions(vec![session_presenting_at("g", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + 3 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    pump_session_event(&mut t);

    assert_adopted(
        &t,
        &calls,
        &["f".to_string(), "g".to_string()],
        2,
        "g",
        first_adopted,
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[1].item.id(),
        "f",
        "the first adopted slot keeps the item the device started"
    );
}

/// Contract: row 3.3 / followups design D4 and spec scenario "The adopted
/// item is tracked like a loaded item" — an adopted item's natural end is
/// inferred from the same sighting margin a loaded item's is: the device
/// going idle records the completion, loads nothing, and leaves the output
/// idle.
#[test]
fn an_adopted_items_natural_end_records_its_completion_and_loads_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    script.set_item(canned_autoplay_item());
    let t0 = Instant::now();
    let (_client_id, ended) = adopt_the_direct_autoplay(&mut t, &script, t0);
    pump_session_event(&mut t);
    let adopted = assert_adopted(&t, &calls, &["f".to_string()], 1, "f", ended);

    // The adopted item's sighting near the runtime is what the idle
    // observation reads the end from.
    script.set_sessions(vec![session_presenting_at(
        "f",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);

    // The device goes idle at the adopted item's end: the completion is
    // recorded, nothing loads, and the output stays idle.
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + 3 * EMBY_SESSION_POLL_INTERVAL);

    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "the idle observation loads nothing"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        2,
        "the idle observation appends nothing"
    );
    assert!(
        slot_played(&t, adopted),
        "the adopted item's completion is recorded like a loaded slot's"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "the output stays idle"
    );
}

/// Contract: row 3.4 / followups design D4 and spec scenario "Next on an
/// adopted item follows the queue" — next on the adopted slot at the queue's
/// end resolves over the queue like any active slot's step, and the queue
/// ends there: nothing loads and the device receives no command.
#[test]
fn next_on_the_adopted_queue_end_slot_loads_nothing_and_sends_no_device_command() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    script.set_item(canned_autoplay_item());
    let (client_id, _ended) = adopt_the_direct_autoplay(&mut t, &script, Instant::now());
    pump_session_event(&mut t);
    let adopted = slot_ids(&t)[1];

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 1,
            generation: 1,
            action: PlaybackIntentAction::Next,
        }),
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "next at the queue's end loads nothing"
    );
    assert!(
        transport_calls(&calls).is_empty(),
        "next sends no device command"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        2,
        "the queue is unchanged"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(adopted),
        "the adopted slot stays active"
    );
}

/// Contract: followups design D4, review correction — a queue edit that
/// lands while the lookup runs re-establishes the queued next item with one
/// `play_last`, and the device holds it: the adoption keeps that record
/// instead of dropping it, so the device's move to it at the adopted item's
/// end is a known item, not a second adoption of the same content.
#[test]
fn an_append_during_the_lookup_flight_keeps_the_queued_next_item() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    script.set_item(canned_autoplay_item());
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    // The sighting of the tracked item, then the device's own autoplay: the
    // lookup is in flight.
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    // The queue edit lands while the lookup runs; the tick that reconciles
    // the device's window with the queue queues the appended slot with one
    // `play_last`.
    t.event_loop.client.lock().unwrap().token = "test-token".to_string();
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![timed("b", RUNTIME_TICKS)],
            before: None,
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    t.event_loop.tick(t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    pump_session_event(&mut t);

    assert_eq!(
        play_last_calls(&calls),
        vec![play_last(&["b"])],
        "the appended slot was queued on the device during the flight"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        3,
        "the adoption appends the autoplay item once: a, the append, the adoption"
    );
    let adopted = t.event_loop.owner.core.queue.slots()[2].slot_id;
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[2].item.id(),
        "f",
        "the adopted slot is the item the device started"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(adopted),
        "the adopted slot becomes the active slot"
    );
    let crate::cast::PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output
            .queued_next
            .as_ref()
            .map(|queued| (queued.slot, queued.item_id.as_str())),
        Some((slot_ids(&t)[1], "b")),
        "the adoption keeps the queued next item the device was given during the flight"
    );
}
