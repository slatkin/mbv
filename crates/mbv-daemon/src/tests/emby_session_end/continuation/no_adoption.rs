// emby-session-output-followups row 3.4 and the PR 966 review corrections:
// the cases the queue-end continuation adopts nothing. A late foreign item,
// a failed lookup, a stopped output, and an item the owner or Session
// cannot admit all leave the item presented and the queue unchanged. The
// adoption itself and the shared assertions live in `continuation.rs`.

use super::*;

use crate::emby_session::ContinuationWatch;

/// Moves the output's last observation stamp back by `age`: the observation
/// path stamps `observed_at` with the real clock (poll.rs `observe`), so a
/// test that needs a sighting older than its poll script drives the stamp
/// through this one helper instead of waiting the age out. The fake times
/// the polls read stay the scenario's one clock.
fn age_last_observation(t: &mut TestLoop, age: Duration) {
    let PlaybackOutput::EmbySession(output) = &mut t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output.observed_at = Some(
        Instant::now()
            .checked_sub(age)
            .expect("the aged observation stamp is representable"),
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
    // the observation the estimate reads. The stamp moves back past the
    // 30-second freshness bound instead of waiting out the absence.
    age_last_observation(&mut t, Duration::from_secs(40));
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
    // No polls land for five minutes. The stamp moves back so the idle
    // observation reveals the natural end three seconds after the sighting,
    // minutes in the past.
    age_last_observation(&mut t, Duration::from_secs(300));
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
    // The user stops the output while the lookup runs: the stop clears the
    // loaded item and sends the device its stop, so the lookup result has
    // nothing left to adopt.
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

/// Contract: followups design D4, review correction (PR 966) — the
/// adoption's append runs the owner's admission gate like every other
/// queue-entry path: an audio-only owner cannot play the video item the
/// device started at the queue's end, so the lookup still runs, the ended
/// slot's completion stands, the item stays presented, and the queue gains
/// no slot.
#[test]
fn an_item_the_owner_does_not_admit_is_not_adopted() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    script.set_item(canned_autoplay_item());
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    t.event_loop.audio_only = true;
    let t0 = Instant::now();

    // The sighting of the tracked item, then the device's own autoplay: the
    // lookup is in flight and the direct path records the end.
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    script.set_sessions(vec![session_presenting_at("f", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    pump_session_event(&mut t);

    assert_eq!(
        get_item_calls(&calls),
        vec!["f".to_string()],
        "the lookup still runs: admission is judged when its answer lands"
    );
    assert!(
        slot_played(&t, slots[0]),
        "the ended slot's completion stands: the device did end the item"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        1,
        "the queue gains no slot the owner cannot play"
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
        output.continuation.is_none(),
        "the spent lookup leaves no pending continuation"
    );
    assert_eq!(
        t.event_loop.player.status.lock().unwrap().title,
        "f",
        "the foreign item stays presented"
    );
}
