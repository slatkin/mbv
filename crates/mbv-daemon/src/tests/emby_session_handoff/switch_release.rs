// The switch-only release of an Emby Session output
// (keep-old-output-playing-on-switch rows 1.1, 2.1, and 2.2): a switch sends
// the device no stop and forgets the loaded item, the new output plays
// nothing, and a released device's own reports settle and advance nothing.

use super::*;

use super::cast_output::{cast_selection, fake_connector, pump_cast_event};
use super::cast_resolve::wait_for_worker_jobs;
use crate::cast::{StopScope, stop_active_output};
use crate::emby_session::EmbySessionEvent;

/// Contract: keep-old-output-playing-on-switch row 1.1 (design D1/D2) — a
/// `Release` stop on an Emby Session output sends the device no stop: the
/// device keeps playing, while the Owner forgets the loaded item.
#[test]
fn a_release_stop_sends_no_stop_to_the_session_device() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let (mut t, calls, _rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot);
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert!(output.loaded.is_some(), "the play loaded its item");

    stop_active_output(
        &t.event_loop.player,
        &mut t.event_loop.owner.output,
        &t.event_loop.owner.core.queue,
        &t.event_loop.ctrl_clients,
        StopScope::Release,
    );
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert!(
        stop_calls(&calls).is_empty(),
        "a release sends the device no stop: {calls:?}"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is still the output");
    };
    assert!(
        output.loaded.is_none(),
        "the release forgets the loaded item"
    );
}

/// Contract: keep-old-output-playing-on-switch row 2.1 (spec "An Emby Session
/// is released by a switch") — a switch from a Session that plays the loaded
/// item to a Cast receiver releases the Session device: it receives no stop
/// and keeps playing, the receiver becomes the output and is given no load,
/// and the queue keeps its order.
#[test]
fn a_switch_to_a_receiver_releases_the_session_device() {
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
    let queue_before = queue_slot_ids(&t);

    let (connector, receiver_calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    switch_output(&mut t, client_id, &mpsc::channel().0, cast_selection());
    pump_cast_event(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert!(
        stop_calls(&calls).is_empty(),
        "the released device receives no stop: {calls:?}"
    );
    assert!(
        receiver_calls.lock().unwrap().is_empty(),
        "the new receiver is given no load and no teardown"
    );
    assert!(
        matches!(
            &t.event_loop.owner.output,
            PlaybackOutput::Cast(cast) if cast.receiver.id == "rcv-1"
        ),
        "the selected receiver is the output after the switch"
    );
    assert_eq!(
        queue_slot_ids(&t),
        queue_before,
        "the queue and its order are unchanged"
    );
}

/// Contract: keep-old-output-playing-on-switch row 2.1 (spec "A remote output
/// switches to local playback") — a switch from a Session that plays the
/// loaded item to Local releases the device: it receives no stop and keeps
/// playing, no local run starts, and the observed slot is cleared and
/// published as none (invariant 25).
#[test]
fn a_switch_to_local_releases_the_session_device() {
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
    let queue_before = queue_slot_ids(&t);
    let worker = session_worker(&t);

    switch_output(
        &mut t,
        client_id,
        &mpsc::channel().0,
        OutputSelection::Local,
    );
    wait_for_worker(&worker);

    assert!(
        stop_calls(&calls).is_empty(),
        "the released device receives no stop: {calls:?}"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "Local becomes the output"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "no local run starts"
    );
    assert_eq!(queue_slot_ids(&t), queue_before, "the queue is unchanged");
    assert_eq!(
        t.event_loop.owner.core.observed_active_slot(),
        None,
        "the switch records no observed active slot"
    );
    assert_eq!(
        *t.event_loop
            .shared_queue
            .observed_active_slot
            .lock()
            .unwrap(),
        None,
        "the switch publishes the observed slot as none (invariant 25)"
    );
}

/// Contract: keep-old-output-playing-on-switch row 2.2 (spec "A released
/// device does not move the queue") — a poll result from the released device
/// after the switch to a receiver settles and advances nothing: the loaded
/// item is forgotten and the output no longer tracks the device.
#[test]
fn a_poll_result_from_a_released_device_advances_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let (mut t, _calls, _rx) =
        session_output_with_queue(fake, vec![timed("movie", 600 * SECOND_TICKS)]);
    let (client_id, _reply_tx, _client_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    play_slot(&mut t, client_id, slot);
    let queue_before = queue_slot_ids(&t);

    let (connector, _receiver_calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    switch_output(&mut t, client_id, &mpsc::channel().0, cast_selection());
    pump_cast_event(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    // The released device reports it still presents the formerly loaded item.
    t.event_loop
        .handle_event(DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
            poll_id: 0,
            result: Ok(vec![session_presenting_at(
                "movie",
                300 * SECOND_TICKS,
                600 * SECOND_TICKS,
            )]),
        }));

    assert!(
        matches!(
            &t.event_loop.owner.output,
            PlaybackOutput::Cast(cast) if cast.receiver.id == "rcv-1"
        ),
        "the stale poll result does not change the output"
    );
    assert_eq!(
        queue_slot_ids(&t),
        queue_before,
        "the stale poll result does not move the queue"
    );
    assert_eq!(
        t.event_loop.owner.core.observed_active_slot(),
        None,
        "the stale poll result restores no observed slot"
    );
}
