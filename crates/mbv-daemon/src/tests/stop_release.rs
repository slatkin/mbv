// The `StopScope::Release` stop (keep-old-output-playing-on-switch row 1.1):
// a switch-only selection releases the old remote output — the Owner keeps
// its stop-report bookkeeping and forgets the loaded item but sends the
// device no stop — while the local run, which the Owner hosts, still stops
// (design D1/D2/D4).

use super::cast_handoff::load_slot_on_receiver;
use super::cast_playback::{feed_qi_with_url, select_cast_output};
use super::cast_resolve::wait_for_worker_jobs;
use super::r#loop::test_loop_with_queue;
use crate::cast::{PlaybackOutput, StopScope, stop_active_output};

/// Contract: keep-old-output-playing-on-switch row 1.1 (design D1/D2) — a
/// `Release` stop on a Cast output queues no media stop and no app teardown
/// on the receiver: the device keeps playing what it plays, while the Owner
/// forgets the loaded item and presents no playback.
#[test]
fn a_release_stop_queues_no_media_stop_on_the_cast_receiver() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (client_id, _rx, calls) = select_cast_output(&mut t, mbv_cast::CastStatus::idle());
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    load_slot_on_receiver(&mut t, client_id, slot);

    stop_active_output(
        &t.event_loop.player,
        &mut t.event_loop.owner.output,
        &t.event_loop.owner.core.queue,
        &t.event_loop.ctrl_clients,
        StopScope::Release,
    );
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let log = calls.lock().unwrap();
    assert!(
        !log.iter().any(|call| call == "stop" || call == "teardown"),
        "a release queues no stop and no teardown on the receiver: {log:?}"
    );
    let PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    assert!(cast.loaded.is_none(), "the release forgets the loaded item");
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "the release presents no active playback"
    );
}
