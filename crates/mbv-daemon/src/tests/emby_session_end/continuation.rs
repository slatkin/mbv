// emby-session-output-followups rows 3.1 to 3.4: the queue-end continuation
// (design D4). When the tracked item ends at the queue's edge and the device
// starts an item of its own, the Owner looks that item up once and adopts it
// as the queue's next slot: appended at the tail, active, and tracked. The
// polls are driven with their own injected times, so no test waits.

use super::*;

use super::super::emby_session_output::pump_session_event;
use super::super::emby_session_support::{play_last, play_last_calls};
use crate::emby_session::ContinuationWatch;

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

    assert_adopted(&t, &calls, &["f".to_string()], 1, "f", slots[0]);
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

    assert_adopted(&t, &calls, &["f".to_string()], 1, "f", slots[0]);
    assert_eq!(
        t.event_loop.owner.core.queue.revision().raw() - revision_after_end.raw(),
        2,
        "the adoption's own queue mutations are the append and the active marker: no second completion record"
    );
}

/// Drives the direct adoption the follow-up tests build on: the queue's one
/// slot is playing, and the device starts the canned autoplay item straight
/// after its sighting. The caller scripted the canned item and reads the
/// adopted slot from the queue afterwards.
fn adopt_the_direct_autoplay(
    t: &mut TestLoop,
    script: &FakeSessionApi,
    client_id: u64,
    t0: Instant,
) {
    let slots = slot_ids(t);
    play_slot(t, client_id, slots[0]);
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(t, t0);
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    pump_session_event(t);
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
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let t0 = Instant::now();
    let slots = slot_ids(&t);
    adopt_the_direct_autoplay(&mut t, &script, client_id, t0);
    let first_adopted = assert_adopted(&t, &calls, &["f".to_string()], 1, "f", slots[0]);

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
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let t0 = Instant::now();
    let slots = slot_ids(&t);
    adopt_the_direct_autoplay(&mut t, &script, client_id, t0);
    let adopted = assert_adopted(&t, &calls, &["f".to_string()], 1, "f", slots[0]);

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

/// The foreign item the device starts after the tracked item's end at the
/// queue's edge, in the two ways the adoption fails (row 3.4). Both leave
/// the item presented and the queue unchanged; they differ in the lookup
/// the owner is left with.
#[derive(Clone, Copy)]
enum NoAdoption {
    /// The Session first presents the item after the 30-second window
    /// closed.
    Late,
    /// The lookup runs and fails; the fake answers it with its failure.
    LookupFails,
}

impl NoAdoption {
    /// Prepares the adoption state the foreign item arrives into: the late
    /// case's window is expired in the injected sense — the observation path
    /// reads the real clock, so the test moves the window's close to the
    /// test's start instead of waiting 30 seconds — and the failed lookup
    /// keeps the window the natural end opened.
    fn prepare(self, t: &mut TestLoop, t0: Instant) {
        if let Self::Late = self {
            let PlaybackOutput::EmbySession(output) = &mut t.event_loop.owner.output else {
                panic!("the Session is the output");
            };
            output.continuation = Some(ContinuationWatch::Window { until: t0 });
        }
    }

    /// The item lookups the case leaves behind.
    fn lookups(self) -> Vec<String> {
        match self {
            Self::Late => Vec::new(),
            Self::LookupFails => vec!["f".to_string()],
        }
    }

    /// Lets the lookup's outcome reach the owner: the failed lookup's event
    /// arrives on the merged channel; the late item submitted no lookup, so
    /// nothing arrives.
    fn settle(self, t: &mut TestLoop) {
        if let Self::LookupFails = self {
            wait_for_session_jobs(&t.event_loop.owner.output);
            pump_session_event(t);
        }
    }
}

/// Contract: row 3.4 / followups design D4 and spec scenarios "A late
/// foreign item is not adopted" and "The item lookup fails" — a foreign item
/// the device starts after the tracked item's end at the queue's edge is
/// adopted only while the window is open and the lookup answers: the late
/// item submits no lookup, the failed one submits exactly one, and both
/// leave the item presented and the queue unchanged.
#[rstest]
#[case::a_foreign_item_after_the_window_closed(NoAdoption::Late)]
#[case::a_failed_lookup(NoAdoption::LookupFails)]
fn the_cases_that_adopt_nothing_leave_the_item_presented_and_the_queue_unchanged(
    #[case] case: NoAdoption,
) {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    // The sighting of the tracked item, then the idle observation one second
    // later: the natural end at the queue's edge opens the 30-second window.
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![idle_session()]);
    let t_end = t0 + EMBY_SESSION_POLL_INTERVAL;
    poll_once(&mut t, t_end);

    // The foreign item the case scripts, first presented one poll after the
    // end.
    case.prepare(&mut t, t0);
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t_end + EMBY_SESSION_POLL_INTERVAL);
    case.settle(&mut t);

    assert_eq!(
        get_item_calls(&calls),
        case.lookups(),
        "the case leaves exactly the lookups it scripts"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        1,
        "the queue gains no slot"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "the active slot is unchanged"
    );
    assert_eq!(
        t.event_loop.player.status.lock().unwrap().title,
        "f",
        "the foreign item stays presented"
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
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    adopt_the_direct_autoplay(&mut t, &script, client_id, Instant::now());
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

/// Contract: row 3.4 / followups design D4 and spec scenario "A late foreign
/// item is not adopted", direct path — the freshness bound the direct arm
/// shares with the idle-gap window: the tracked item's estimated end is
/// measured from its last sighting, so an estimate that extrapolated a long
/// absence (the device was missing from `/Sessions`) meets the end margin
/// trivially and must not adopt. The foreign item the device presents after
/// a longer-than-30-second absence stays present-only, submits no lookup,
/// and leaves the queue and the ended slot's completion alone. The case
/// stands alone because its prologue has no idle observation: the direct
/// path is the one the no-adoption table's shared prologue cannot reach.
#[test]
fn a_foreign_item_presented_after_a_long_absence_is_not_adopted() {
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
    // The device then goes missing and the polls fail, so the sighting stays
    // the observation the estimate reads. The observation path reads the real
    // clock, so the test ages the sighting past the 30-second bound instead
    // of waiting — the same injection the no-adoption table's late case makes
    // to its window.
    let PlaybackOutput::EmbySession(output) = &mut t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output.observed_at = Some(
        Instant::now()
            .checked_sub(Duration::from_secs(40))
            .expect("40 seconds before now is representable"),
    );
    // The device reappears playing its own item.
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);

    assert!(
        get_item_calls(&calls).is_empty(),
        "the late foreign item submits no lookup"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        1,
        "the queue gains no slot"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "the active slot is unchanged"
    );
    assert!(
        !slot_played(&t, slots[0]),
        "the ended slot records no completion for content the owner did not adopt"
    );
    assert_eq!(
        t.event_loop.player.status.lock().unwrap().title,
        "f",
        "the foreign item stays presented"
    );
}

/// Contract: spec "A late foreign item is not adopted" (owner-playback-output)
/// and followups design D4 — the queue-end window opens from the estimated
/// end, not from the poll that detects it. The device is idle five minutes
/// after its tracked item ended, so the detecting idle observation must not
/// open a window that runs from its own time: the foreign item presented 20
/// seconds later is more than 30 seconds past the end and submits no lookup.
#[test]
fn a_natural_end_revealed_after_a_long_absence_opens_no_window() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    script.set_item(canned_autoplay_item());
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
    // No polls land for five minutes. The observation path reads the real
    // clock, so the test ages the sighting instead of waiting: the device then
    // reports idle, which reveals the natural end three seconds after the
    // sighting.
    let PlaybackOutput::EmbySession(output) = &mut t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output.observed_at = Some(
        Instant::now()
            .checked_sub(Duration::from_secs(300))
            .expect("300 seconds before now is representable"),
    );
    script.set_sessions(vec![idle_session()]);
    poll_once(&mut t, t0 + Duration::from_secs(300));
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + Duration::from_secs(320));

    assert!(
        get_item_calls(&calls).is_empty(),
        "a foreign item presented after the reappearance is past the 30-second window from the end"
    );
}

/// Contract: followups design D4, review correction — the adoption
/// re-establishes the tracked item, so a lookup result that lands after the
/// output was stopped adopts nothing: the stop cleared the loaded item and a
/// stopped output advances nothing. The pending continuation is spent and
/// the queue and its persistence are untouched.
#[test]
fn a_lookup_result_after_the_output_was_stopped_adopts_nothing() {
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
    // The user stops the output while the lookup runs: the loaded item is
    // gone, and the device keeps the foreign content it started.
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);
    let persisted_before = t.persisted.borrow().len();
    pump_session_event(&mut t);

    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        1,
        "the stopped output adopts nothing"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "the active slot is unchanged"
    );
    let crate::cast::PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert!(
        output.loaded.is_none(),
        "the stopped output stays without a tracked item"
    );
    assert!(
        output.continuation.is_none(),
        "the lookup result spends the pending continuation"
    );
    assert_eq!(
        t.persisted.borrow().len(),
        persisted_before,
        "the rejected lookup persists nothing"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "the rejected lookup sends no device command"
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
