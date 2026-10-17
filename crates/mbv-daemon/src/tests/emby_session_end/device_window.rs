// emby-session-owner-output rows 9.3 and 9.4: the device's window on the Emby
// Session output. The device holds the loaded item and the queued next one
// (design D14), so the Owner follows the device's own move to the queued slot
// with one `play_last`, and reconciles that window with the queue on each
// observation and tick: a removed queued slot is never adopted, the queue's
// correct next slot corrects the device, and a queue with no next admissible
// slot stops it. The polls are driven with their own injected times, so no test
// waits.

use super::*;

use mbv_player::owner_state::StepTarget;

/// The one `play_last` call the Session receives for `item_ids` (design D14),
/// the slot the device holds after the active one.
fn play_last(item_ids: &[&str]) -> SessionCall {
    SessionCall::PlayLast {
        session_id: "session-1".to_string(),
        item_ids: item_ids.iter().map(|id| (*id).to_string()).collect(),
    }
}

/// The `play_last` calls the Session received, in order: the slots the Owner
/// appended to the device (design D14).
fn play_last_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| matches!(call, SessionCall::PlayLast { .. }))
        .cloned()
        .collect()
}

/// The one `transport("Stop")` call the Session receives (design D14).
fn stop() -> SessionCall {
    SessionCall::Transport {
        session_id: "session-1".to_string(),
        command: "Stop".to_string(),
    }
}

/// The transport calls the Session received, in order (design D9, D14).
fn transport_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| matches!(call, SessionCall::Transport { .. }))
        .cloned()
        .collect()
}

/// Removes `slot` from the owner's queue through the queue edit command, so a
/// test drives a queue edit after the load put the slot on the device.
fn remove_slot(t: &mut TestLoop, client_id: u64, slot: mbv_queue::QueueSlotId) {
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
}

/// Appends `item` to the owner's queue through the queue edit command.
fn append_item(t: &mut TestLoop, client_id: u64, item: QueueItem) {
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueAppend {
            items: vec![item],
            before: None,
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
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

/// Contract: row 9.3 / design D14 and spec "The Emby Session device holds the
/// active item and the next item" — the device moves to the queued next item
/// by itself: that slot becomes the active one with no `play`, the old slot's
/// completion is recorded only within 10 seconds of its runtime, and the slot
/// after the new active one is appended with one `play_last`. The same item in
/// both slots does not change the id the Session shows, so that move is the
/// position drop to the item's start.
#[rstest]
#[case::the_loaded_item_ends_and_the_device_starts_the_queued_slot(
    &["a", "b", "c"],
    true
)]
#[case::a_device_next_60_seconds_before_the_end_records_no_completion(
    &["a", "b", "c"],
    false
)]
#[case::the_same_item_in_both_slots_advances_on_the_position_drop(&["a", "a", "b"], true)]
fn the_device_advance_follows_the_device_with_one_play_last(
    #[case] names: &[&str],
    #[case] near_end: bool,
) {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        names
            .iter()
            .map(|name| timed(name, RUNTIME_TICKS))
            .collect(),
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    assert_eq!(
        play_calls(&calls),
        vec![play(&[names[0], names[1]])],
        "the load gives the device the active item and the queued next one"
    );
    let t0 = Instant::now();

    // The sighting of the loaded item: within 10 seconds of the runtime is a
    // natural end, 60 seconds before it is a next on the device.
    let position = if near_end {
        RUNTIME_TICKS - 3 * SECOND_TICKS
    } else {
        RUNTIME_TICKS - 60 * SECOND_TICKS
    };
    script.set_sessions(vec![session_presenting_at(
        names[0],
        position - SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    // A second sighting of the loaded item is no advance, however near its end
    // it is: the same item in both slots moves only on a drop to its start.
    script.set_sessions(vec![session_presenting_at(
        names[0],
        position,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "a sighting of the loaded item is no advance"
    );
    assert!(
        play_last_calls(&calls).is_empty(),
        "nothing is appended while the loaded item still shows"
    );
    // The device starts the queued slot by itself, at its start.
    script.set_sessions(vec![session_presenting_at(names[1], 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play(&[names[0], names[1]])],
        "the device's own move loads nothing"
    );
    assert_eq!(
        play_last_calls(&calls),
        vec![play_last(&[names[2]])],
        "the slot after the new active one is appended to the device"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[1]),
        "the queued slot becomes the active slot without a load"
    );
    assert_eq!(
        slot_played(&t, slots[0]),
        near_end,
        "completion is recorded only for an end within 10 seconds of the runtime"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        Some(slots[1]),
        "the queued slot is the loaded item without a load"
    );
    assert_eq!(
        output
            .queued_next
            .as_ref()
            .map(|queued| (queued.slot, queued.item_id.as_str())),
        Some((slots[2], names[2])),
        "the appended slot is the queued next item"
    );
}

/// Contract: row 9.4 / design D14 "Queue edits" and "the queue corrects the
/// device" — a queue edit that removes the queued slot interrupts nothing; when
/// the device moves to the removed slot, the Owner does not adopt it: it
/// records the old slot's completion and loads the queue's correct next slot,
/// with the slot after it when the queue has one.
#[rstest]
#[case::the_corrected_slot_is_the_last_one(&["a", "b", "c"], &["c"], None)]
#[case::the_slot_after_the_corrected_one_is_loaded_too(
    &["a", "b", "c", "d"],
    &["c", "d"],
    Some(3)
)]
fn a_removed_queued_slot_is_corrected_with_the_queue_next_slot(
    #[case] names: &[&str],
    #[case] expected_play: &[&str],
    #[case] queued_index: Option<usize>,
) {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(
        fake,
        names
            .iter()
            .map(|name| timed(name, RUNTIME_TICKS))
            .collect(),
    );
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    let t0 = Instant::now();

    script.set_sessions(vec![session_presenting_at(
        names[0],
        RUNTIME_TICKS - 3 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    remove_slot(&mut t, client_id, slots[1]);
    script.set_sessions(vec![session_presenting_at(
        names[0],
        RUNTIME_TICKS - 2 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play(&[names[0], names[1]])],
        "removing the queued slot sends nothing at once"
    );
    assert!(
        transport_calls(&calls).is_empty(),
        "the active item is never interrupted"
    );

    script.set_sessions(vec![session_presenting_at(names[1], 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play(&[names[0], names[1]]), play(expected_play)],
        "the move to the removed slot loads the queue's next admissible slot and the one after it"
    );
    assert!(
        transport_calls(&calls).is_empty(),
        "a correction does not stop the device"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[2]),
        "the corrected slot becomes the active slot"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.slots().len(),
        names.len() - 1,
        "only the removed slot leaves the queue: the completion consumes nothing by default"
    );
    assert!(
        slot_played(&t, slots[0]),
        "the old slot's natural end is recorded"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output.loaded.as_ref().map(|loaded| loaded.slot),
        Some(slots[2]),
        "the corrected slot is the loaded item"
    );
    assert_eq!(
        output.queued_next.as_ref().map(|queued| queued.slot),
        queued_index.map(|index| slots[index]),
        "the window after the corrected slot is what the queue holds"
    );
    // The corrected slot is what the device plays now, so the queue records it
    // as the observed slot: a following Next steps from it instead of reloading
    // it from the slot the device left, and Clients highlight that row.
    assert_eq!(
        t.event_loop.owner.core.observed_active_slot(),
        Some(slots[2]),
        "the corrected slot is the observed active slot"
    );
    assert_eq!(
        *t.event_loop
            .shared_queue
            .observed_active_slot
            .lock()
            .unwrap(),
        Some(slots[2]),
        "the published queue names the corrected slot"
    );
    assert_eq!(
        t.event_loop
            .owner
            .core
            .relative_step_target(mbv_ctrl::Direction::Next),
        queued_index.map_or(StepTarget::AtEdge, |index| StepTarget::Jump(slots[index])),
        "a following Next steps from the corrected slot"
    );
}

/// Contract: row 9.4 / design D14 "the queue corrects the device" — a queue
/// edit that removes the only next slot interrupts nothing; when the device
/// moves to the removed slot, the Owner does not adopt it, records the old
/// slot's completion, and stops the device, so the queue ends on the old slot.
#[test]
fn a_removed_only_next_slot_stops_the_device_when_it_moves_to_it() {
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
    remove_slot(&mut t, client_id, slots[1]);
    script.set_sessions(vec![session_presenting_at(
        "a",
        RUNTIME_TICKS - 2 * SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![play(&["a", "b"])],
        "removing the only next slot sends nothing at once"
    );
    assert!(
        transport_calls(&calls).is_empty(),
        "the active item is never interrupted"
    );

    script.set_sessions(vec![session_presenting_at("b", 0, RUNTIME_TICKS)]);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        transport_calls(&calls),
        vec![stop()],
        "the move to the removed slot stops the device"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a", "b"])],
        "the correction loads nothing"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slots[0]),
        "the queue ends on the old slot"
    );
    assert!(
        slot_played(&t, slots[0]),
        "the old slot's natural end is recorded"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert!(
        output.loaded.is_none() && output.queued_next.is_none(),
        "the stopped device holds none of the Owner's items"
    );
}

/// Contract: row 9.4 / design D14 "Queue edits" — an append at the end of a
/// queue the device holds nothing next for is sent to the device with one
/// `play_last`, so the next item boundary stays the queue's.
#[test]
fn an_append_to_a_queue_with_nothing_queued_sends_one_play_last() {
    let fake = FakeSessionApi::with_sessions(vec![idle_session()]);
    let script = fake.clone();
    let (mut t, calls, _rx) = session_output_with_queue(fake, vec![timed("a", RUNTIME_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slots = slot_ids(&t);
    play_slot(&mut t, client_id, slots[0]);
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "a queue with one slot gives the device no next item"
    );
    let t0 = Instant::now();

    script.set_sessions(vec![session_presenting_at(
        "a",
        SECOND_TICKS,
        RUNTIME_TICKS,
    )]);
    poll_once(&mut t, t0);
    // The queue-append gate admits an Emby item only for a runtime that holds an
    // Emby token (`daemon_admits`); the fake Session answers every call.
    t.event_loop.client.lock().unwrap().token = "test-token".to_string();
    append_item(&mut t, client_id, timed("b", RUNTIME_TICKS));
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);

    let appended = slot_ids(&t)[1];
    assert_eq!(
        play_last_calls(&calls),
        vec![play_last(&["b"])],
        "the appended slot is queued on the device once"
    );
    assert_eq!(
        play_calls(&calls),
        vec![play(&["a"])],
        "an append loads nothing"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output.queued_next.as_ref().map(|queued| queued.slot),
        Some(appended),
        "the appended slot is the queued next item"
    );
}
