// cast-owner-output row 6.1: the Owner translates a receiver status into its
// presented `PlayerStatus` — the loaded slot as the active slot with the
// receiver's position, duration, and paused state — advances the position on
// the daemon tick while the receiver plays, holds it while the receiver is
// paused or buffering, and lets a fresh report correct the extrapolation.
// The daemon's status-broadcast thread (`run.rs` `spawn_status_broadcast`)
// publishes this status to Clients at the local broadcast cadence, so the
// presented status is what Clients receive as `StatusOnly` (design D8).

use super::*;

use mbv_cast::CastStatus;
use mbv_cast::client::CastPlaybackState;

use super::cast_output::{cast_selection, connect_output_client, pump_cast_event, select_output};
use super::cast_playback::{feed_qi_with_url, playing_status_at, pump, status_connector};
use super::cast_resolve::wait_for_worker_jobs;
use super::r#loop::{TestLoop, test_loop_with_queue};

/// One second of media in ticks.
const SECOND_TICKS: i64 = mbv_emby_model::TICKS_PER_SECOND;

/// Asserts the presented position is `seconds` within half a tick-second and
/// returns it, so the numeric assertions stay integer-valued.
fn presented_near(t: &TestLoop, seconds: i64) -> i64 {
    let ticks = t.event_loop.player.status.lock().unwrap().position_ticks;
    let window =
        (seconds * SECOND_TICKS - SECOND_TICKS / 2)..=(seconds * SECOND_TICKS + SECOND_TICKS / 2);
    assert!(
        window.contains(&ticks),
        "presented {ticks} ticks, expected ~{seconds}s"
    );
    ticks
}

/// A receiver status in `state` at 10s of a 600s item of the fixture queue.
fn status_in_state(state: CastPlaybackState) -> CastStatus {
    CastStatus {
        state,
        duration_seconds: Some(600.0),
        ..playing_status_at("https://feed/a.mp3", 10.0)
    }
}

/// Selects the receiver as the output, loads the queue's first slot on it,
/// and returns the selecting client's id and event channel. The fixture's
/// fake transport answers every status poll with `status`.
fn casting_fixture(t: &mut TestLoop, status: CastStatus) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (connector, _calls) = status_connector(status);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(t, client_id, &reply_tx, cast_selection());
    pump_cast_event(t);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    pump(t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    (client_id, rx)
}

/// Contract: cast-owner-output row 6.1 — a receiver playing the loaded item
/// presents that slot as the active slot with the receiver's position and
/// duration, and the daemon tick advances the presented position from the
/// last report and the elapsed time (design D8).
#[test]
fn a_playing_receiver_presents_its_slot_and_the_tick_advances_the_position() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (_client_id, _rx) = casting_fixture(&mut t, status_in_state(CastPlaybackState::Playing));
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert!(
        status.active && !status.paused,
        "a playing receiver presents active, unpaused playback"
    );
    presented_near(&t, 10);
    assert_eq!(
        status.runtime_ticks,
        600 * SECOND_TICKS,
        "the receiver's reported duration is presented"
    );
    assert_eq!(status.current_idx, 0, "the loaded slot is the active slot");
    assert_eq!(status.queue_len, 1);
    assert_eq!(status.title, "a", "the loaded slot's item is named");

    t.event_loop.tick(t0 + std::time::Duration::from_secs(5));
    presented_near(&t, 15);
}

/// Contract: cast-owner-output row 6.1 — the presented position holds while
/// the receiver reports a paused or buffering state, and the paused flag
/// follows the receiver's state (spec "Receiver stalls").
#[rstest]
#[case::paused(CastPlaybackState::Paused, true)]
#[case::buffering(CastPlaybackState::Buffering, false)]
fn a_paused_or_buffering_receiver_holds_the_presented_position(
    #[case] state: CastPlaybackState,
    #[case] paused: bool,
) {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (_client_id, _rx) = casting_fixture(&mut t, status_in_state(state));
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);

    t.event_loop.tick(t0 + std::time::Duration::from_secs(5));

    let status = t.event_loop.player.status.lock().unwrap().clone();
    presented_near(&t, 10);
    assert!(status.active, "the loaded item is still the presented slot");
    assert_eq!(status.paused, paused, "the paused flag follows the state");
}

/// Contract: cast-owner-output row 6.1 — a fresh status report corrects the
/// extrapolated position: the presented position derives from the last
/// report, so a report that disagrees with the extrapolation adopts the
/// reported position (spec "Extrapolation has drifted").
#[test]
fn a_fresh_report_corrects_the_extrapolated_position() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (_client_id, _rx) = casting_fixture(&mut t, status_in_state(CastPlaybackState::Playing));
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);
    t.event_loop.tick(t0 + std::time::Duration::from_secs(120));
    presented_near(&t, 130);

    // The next poll went out with the extrapolating tick; its fake result is
    // drained unapplied and a corrective report is delivered under the same
    // poll id, as the real worker would for a receiver that disagrees with
    // the extrapolation.
    wait_for_worker_jobs(&t.event_loop.owner.output);
    let event = t.merged_rx.try_recv().expect("the poll reported a result");
    let DaemonEvent::Cast(CastOwnerEvent::Status { poll_id, .. }) = event else {
        panic!("expected the poll's status result");
    };
    t.event_loop
        .handle_event(DaemonEvent::Cast(CastOwnerEvent::Status {
            receiver_id: "rcv-1".to_string(),
            poll_id,
            result: Ok(playing_status_at("https://feed/a.mp3", 50.0)),
        }));

    presented_near(&t, 50);
}

/// Contract: cast-owner-output row 6.1 (spec "Receiver is idle") — a
/// receiver that reports no active media presents no active playback.
#[test]
fn an_idle_receiver_presents_no_active_playback() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (_client_id, _rx) = casting_fixture(&mut t, CastStatus::idle());
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert!(
        !status.active,
        "an idle receiver presents no active playback"
    );
    assert_eq!(status.position_ticks, 0);
    assert!(status.title.is_empty(), "no now-playing metadata");
}

/// Contract: cast-owner-output row 6.1 (spec "Receiver plays something mbv
/// did not dispatch") — media the Owner did not load is presented without
/// treating it as an error and never as the queue's active slot: the
/// presented status shows no active playback, because the owner cannot name
/// a slot for media it did not load.
#[test]
fn a_receiver_playing_foreign_media_presents_no_active_playback() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (_client_id, _rx) =
        casting_fixture(&mut t, playing_status_at("https://feed/other.mp3", 30.0));

    // The receiver reports playing something else. A tick would otherwise
    // extrapolate and present; the content-id guard holds the presentation.
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert!(
        !status.active,
        "foreign media is not presented as the queue's active slot"
    );
    assert_eq!(status.position_ticks, 0);
}

/// Contract: cast-owner-output row 6.1 — stopping the receiver's media
/// forgets the loaded item, and the presented status follows: no active
/// playback is presented for a stopped output (design D8).
#[test]
fn stopping_the_receiver_presents_no_active_playback() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (client_id, _rx) = casting_fixture(&mut t, status_in_state(CastPlaybackState::Playing));
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);
    assert!(
        t.event_loop.player.status.lock().unwrap().active,
        "the playing receiver is presented before the stop"
    );

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert!(!status.active, "the stopped output presents no playback");
    assert!(status.title.is_empty(), "the stopped item is unnamed");
}
