// emby-session-output-followups rows 3.1 and 3.2: the queue-end continuation
// (design D4). When the tracked item ends at the queue's edge and the device
// starts an item of its own, the Owner looks that item up once and adopts it
// as the queue's next slot: appended at the tail, active, and tracked. The
// polls are driven with their own injected times, so no test waits.

use super::*;

use super::super::emby_session_output::pump_session_event;

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
fn adopted_output(t: &TestLoop, slot: mbv_queue::QueueSlotId) {
    let crate::cast::PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output
            .loaded
            .as_ref()
            .map(|loaded| (loaded.slot, loaded.item_id.as_str())),
        Some((slot, "f")),
        "the adopted item is the tracked item"
    );
    assert!(
        output.continuation.is_none(),
        "the adoption spends the pending continuation"
    );
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
    script.set_item(canned_autoplay_item());
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    // The sighting of the tracked item, within 10 seconds of its runtime.
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    // The device starts its own next item straight away: no idle in between.
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    let persisted_before = t.persisted.borrow().len();
    pump_session_event(&mut t);

    assert_eq!(
        get_item_calls(&calls),
        vec!["f".to_string()],
        "the adoption submits one item lookup"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "the adoption sends no device command: the item already plays"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        2,
        "the adopted item is appended at the queue's tail"
    );
    let adopted = t.event_loop.owner.core.queue.slots()[1].slot_id;
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[1].item.id(),
        "f",
        "the appended slot is the item the device started"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(adopted),
        "the adopted slot becomes the active slot"
    );
    assert!(
        slot_played(&t, slots[0]),
        "the ended slot records its completion"
    );
    adopted_output(&t, adopted);
    assert_eq!(
        t.persisted.borrow().len(),
        persisted_before + 1,
        "the adoption is the one queue change the Session event persists"
    );
    assert_eq!(
        t.persisted.borrow()[persisted_before].item_ids(),
        ["a".to_string(), "f".to_string()],
        "the persisted queue holds the adopted slot"
    );
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
    script.set_item(canned_autoplay_item());
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();
    let revision_after_play = t.event_loop.owner.core.queue.revision();

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
    let revision_after_end = t.event_loop.owner.core.queue.revision();
    assert!(
        slot_played(&t, slots[0]),
        "the natural end recorded the completion"
    );
    assert_eq!(
        revision_after_end.raw() - revision_after_play.raw(),
        1,
        "the natural end is the one completion record so far"
    );

    // The device starts its own item within the 30-second window.
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    assert_eq!(
        t.event_loop.owner.core.queue.revision(),
        revision_after_end,
        "the detection records no second completion"
    );
    wait_for_session_jobs(&t.event_loop.owner.output);
    pump_session_event(&mut t);

    assert_eq!(
        get_item_calls(&calls),
        vec!["f".to_string()],
        "the adoption submits one item lookup"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "the adoption sends no device command"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        2,
        "the adopted item is appended at the queue's tail"
    );
    let adopted = t.event_loop.owner.core.queue.slots()[1].slot_id;
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(adopted),
        "the adopted slot becomes the active slot"
    );
    assert!(
        slot_played(&t, slots[0]),
        "the ended slot's completion is still the one the natural end recorded"
    );
    adopted_output(&t, adopted);
    assert_eq!(
        t.event_loop.owner.core.queue.revision().raw() - revision_after_end.raw(),
        2,
        "the adoption's own queue mutations are the append and the active marker: no second completion record"
    );
}
