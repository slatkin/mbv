// cast-owner-output rows 6.1 and 6.2: the Owner translates a receiver status
// into its presented `PlayerStatus` — the loaded slot as the active slot with
// the receiver's position, duration, and paused state — advances the position
// on the daemon tick while the receiver plays, holds it while the receiver is
// paused or buffering, and lets a fresh report correct the extrapolation.
// Row 6.2 applies the advance table of design D7: a `Finished` end of the
// loaded item advances the queue once through the shared step funnel, an
// `Error` end is reported and advances nothing, a `Cancelled` end and foreign
// content advance nothing, an uncastable step target is reported and stepped
// over, and the edge leaves the output idle. The daemon's status-broadcast
// thread (`run.rs` `spawn_status_broadcast`) publishes the presented status to
// Clients at the local broadcast cadence, so the presented status is what
// Clients receive as `StatusOnly` (design D8).

use super::*;

use mbv_cast::CastStatus;
use mbv_cast::client::CastIdleReason;
use mbv_cast::client::CastPlaybackState;

use super::cast_output::{cast_selection, connect_output_client, pump_cast_event, select_output};
use super::cast_playback::{
    drained_events, feed_qi_with_url, feed_qi_without_url, playing_status_at, pump,
    status_connector,
};
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
/// did not dispatch") — media the Owner did not load is presented by the
/// receiver's reported state without treating it as an error: active and
/// playing, with the receiver's position and duration, and no item identity.
/// It is never presented as the queue's loaded slot.
#[test]
fn a_receiver_playing_foreign_media_presents_its_state_without_an_item() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let foreign = CastStatus {
        duration_seconds: Some(600.0),
        ..playing_status_at("https://feed/other.mp3", 30.0)
    };
    let (_client_id, _rx) = casting_fixture(&mut t, foreign);

    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert!(
        status.active && !status.paused,
        "the receiver's playing state is presented"
    );
    presented_near(&t, 30);
    assert_eq!(
        status.runtime_ticks,
        600 * SECOND_TICKS,
        "the receiver's reported duration is presented"
    );
    assert!(
        status.title.is_empty(),
        "the foreign media names no queue item"
    );
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

/// Selects the receiver as the output, loads the queue's first slot on it,
/// and lets the fixture transport's confirming status settle the load's
/// transition — the shape every advance test starts from: the end report
/// under test is injected as the next poll's result. Returns the selecting
/// client's id, its event channel, the transport's call log, and the instant
/// the settle tick ran at, so the end report's tick can be placed a poll
/// cadence later.
fn settled_casting_fixture(
    t: &mut TestLoop,
) -> (
    u64,
    mpsc::Receiver<CtrlOutbound>,
    Arc<Mutex<Vec<String>>>,
    std::time::Instant,
) {
    let (connector, calls) = status_connector(playing_status_at("https://feed/a.mp3", 10.0));
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
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(t);
    (client_id, rx, calls, t0)
}

/// Reports `end` as the receiver's next poll result (design D7): the tick's
/// poll is drained unapplied and `end` is delivered under its poll id, the
/// same injection the drift-correction test uses.
fn report_end(t: &mut TestLoop, end: CastStatus, at: std::time::Instant) {
    t.event_loop.tick(at);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    let event = t.merged_rx.try_recv().expect("the poll reported a result");
    let DaemonEvent::Cast(CastOwnerEvent::Status { poll_id, .. }) = event else {
        panic!("expected the poll's status result");
    };
    t.event_loop
        .handle_event(DaemonEvent::Cast(CastOwnerEvent::Status {
            receiver_id: "rcv-1".to_string(),
            poll_id,
            result: Ok(end),
        }));
}

/// A receiver status that reports `content_id`'s media session went idle for
/// `reason`: idle, no position, the content session still named.
fn idle_status(content_id: &str, reason: CastIdleReason) -> CastStatus {
    CastStatus {
        state: CastPlaybackState::Idle,
        position_seconds: None,
        duration_seconds: None,
        playback_rate: 1.0,
        playing_content_id: Some(content_id.to_string()),
        idle_reason: Some(reason),
    }
}

/// The load calls the transport recorded, in order.
pub(super) fn load_calls(calls: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.starts_with("load("))
        .cloned()
        .collect()
}

/// The `CommandRejected` reports the client's event channel carries.
fn rejected_reports(rx: &mpsc::Receiver<CtrlOutbound>) -> Vec<String> {
    drained_events(rx)
        .into_iter()
        .filter_map(|event| match event {
            CtrlEvent::CommandRejected(report) => Some(report),
            _ => None,
        })
        .collect()
}

/// Contract: cast-owner-output row 6.2 (design D7) — a `Finished` end of the
/// loaded item advances once: the next slot of the play order is resolved and
/// loaded, and only that load is sent.
#[test]
fn a_finished_end_loads_the_next_slot_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    let (_client_id, _rx, calls, t0) = settled_casting_fixture(&mut t);

    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        t0 + std::time::Duration::from_secs(120),
    );
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        load_calls(&calls),
        vec![
            "load(https://feed/a.mp3, 0)".to_string(),
            "load(https://feed/b.mp3, 0)".to_string()
        ],
        "the finished item's end advances exactly one slot"
    );
}

/// Contract: cast-owner-output row 6.2 (design D7) — the end-handled flag: a
/// second poll of the same `Finished` entry steps nothing again. The step
/// target is uncastable and the step-over ends at the edge, so the loaded item
/// stays put and the same end report is still handled once: exactly one
/// report and no second step.
#[test]
fn a_second_poll_of_the_same_finished_end_steps_nothing_again() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_without_url("b"),
        ],
        0,
    );
    let (_client_id, rx, calls, t0) = settled_casting_fixture(&mut t);
    let later = t0 + std::time::Duration::from_secs(120);

    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        later,
    );
    pump(&mut t);
    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        later + std::time::Duration::from_secs(120),
    );

    assert!(
        t.merged_rx.try_recv().is_err(),
        "the second poll of the same Finished end produces no event"
    );
    assert_eq!(
        load_calls(&calls),
        vec!["load(https://feed/a.mp3, 0)".to_string()],
        "the handled end steps nothing again"
    );
    assert_eq!(
        rejected_reports(&rx).len(),
        1,
        "the uncastable step is reported once, not once per poll"
    );
}

/// Contract: cast-owner-output row 6.2 (design D7, spec "Receiver fails to
/// play the item") — an `Error` end is reported to Clients and advances
/// nothing: no slot is loaded after the failure.
#[test]
fn a_receiver_error_is_reported_and_loads_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    let (_client_id, rx, calls, t0) = settled_casting_fixture(&mut t);

    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Error),
        t0 + std::time::Duration::from_secs(120),
    );

    let reports = rejected_reports(&rx);
    assert_eq!(
        reports.len(),
        1,
        "the receiver's error is reported to Clients: {reports:?}"
    );
    assert!(
        reports[0].contains('a') && reports[0].contains("failed to play"),
        "the report names the item: {reports:?}"
    );
    assert_eq!(
        load_calls(&calls),
        vec!["load(https://feed/a.mp3, 0)".to_string()],
        "the error end skips ahead nowhere"
    );
}

/// Contract: cast-owner-output row 6.2 (design D7, spec "User stops on the
/// device" and "Receiver plays something mbv did not dispatch") — an end that
/// does not finish the loaded item advances nothing: no slot is loaded and the
/// loaded item keeps its place.
#[rstest]
// Design D7, `Idle(Cancelled)`: the loaded item was cancelled, not finished.
#[case::cancelled_loaded_item("https://feed/a.mp3", CastIdleReason::Cancelled)]
// Design D7, foreign content: a `Finished` end of media the Owner did not load.
#[case::finished_foreign_media("https://feed/other.mp3", CastIdleReason::Finished)]
fn an_end_that_does_not_finish_the_loaded_item_advances_nothing(
    #[case] content_id: &str,
    #[case] reason: CastIdleReason,
) {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    let (_client_id, _rx, calls, t0) = settled_casting_fixture(&mut t);

    report_end(
        &mut t,
        idle_status(content_id, reason),
        t0 + std::time::Duration::from_secs(120),
    );

    assert!(t.merged_rx.try_recv().is_err(), "the end starts no load");
    assert_eq!(
        load_calls(&calls),
        vec!["load(https://feed/a.mp3, 0)".to_string()],
        "the loaded item keeps its place"
    );
}

/// Contract: cast-owner-output row 6.2 (spec "Advance reaches an uncastable
/// slot") — the advance steps over an uncastable slot: it is reported to
/// Clients by name and reason, and the next castable slot after it is loaded.
#[test]
fn an_advance_steps_over_an_uncastable_slot_and_reports_it() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_without_url("b"),
            feed_qi_with_url("c", "https://feed/c.mp3"),
        ],
        0,
    );
    let (_client_id, rx, calls, t0) = settled_casting_fixture(&mut t);

    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        t0 + std::time::Duration::from_secs(120),
    );
    pump(&mut t);
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let reports = rejected_reports(&rx);
    assert!(
        reports
            .iter()
            .any(|report| report.contains("\"b\" has no media URL")),
        "the uncastable step target is reported by name and reason: {reports:?}"
    );
    assert_eq!(
        load_calls(&calls),
        vec![
            "load(https://feed/a.mp3, 0)".to_string(),
            "load(https://feed/c.mp3, 0)".to_string()
        ],
        "the advance skips the uncastable slot and loads the next castable one"
    );
}

/// Contract: cast-owner-output row 6.2 (design D7, spec "Last item ends") —
/// an advance at the edge of the queue loads nothing and the output stays
/// idle.
#[test]
fn an_advance_at_the_queue_edge_leaves_the_output_idle() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let (_client_id, _rx, calls, t0) = settled_casting_fixture(&mut t);

    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        t0 + std::time::Duration::from_secs(120),
    );

    assert!(
        t.merged_rx.try_recv().is_err(),
        "the edge advance starts no load"
    );
    assert_eq!(
        load_calls(&calls),
        vec!["load(https://feed/a.mp3, 0)".to_string()],
        "the last slot's end loads nothing"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "the output stays idle at the edge"
    );
}

/// Contract: cast-owner-output row 6.2 review fix — a load whose in-flight
/// transition expired without a settling status never recorded its slot as
/// the observed active slot, and the shared step funnel would step from the
/// stale predecessor and reload the item that just finished. The Finished end
/// records the ended slot before stepping (design D7's Finished row), so the
/// advance loads the slot after the loaded one.
#[test]
fn an_unsettled_loads_natural_end_advances_past_it_not_back_onto_it() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("x", "https://feed/x.mp3"),
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    // The transport confirms only x: the predecessor whose stale observation
    // would mislead the step funnel.
    let (connector, calls) = status_connector(playing_status_at("https://feed/x.mp3", 5.0));
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    let slots: Vec<_> = t
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();

    // Play x and let its confirming status settle it.
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slots[0]),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    pump(&mut t);
    let t0 = std::time::Instant::now();
    t.event_loop.tick(t0);
    pump(&mut t);

    // Play a: its transition expires without a settling status, so a is
    // never recorded as the observed active slot.
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slots[1]),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    let backdated = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .expect("the monotonic clock reaches back ten seconds");
    t.event_loop.owner.core.transitions.expire(backdated);
    t.event_loop.on_recv_timeout(std::time::Instant::now());
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "a's load transition expired without a settling status"
    );

    // a's natural end: the advance steps to the slot after a.
    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        t0 + std::time::Duration::from_secs(120),
    );
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        load_calls(&calls),
        vec![
            "load(https://feed/x.mp3, 0)".to_string(),
            "load(https://feed/a.mp3, 0)".to_string(),
            "load(https://feed/b.mp3, 0)".to_string()
        ],
        "the unsettled load's end advances past it instead of reloading it"
    );
}

/// The feed slots' canonical completion state, in queue order: guid,
/// position, and played, so a Cast outcome can be compared against the
/// local-playback outcome slot by slot.
fn feed_completion_state(owner: &DaemonPlayerOwner) -> Vec<(String, i64, bool)> {
    owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| match &slot.item {
            QueueItem::Feed(entry) => (entry.guid.clone(), entry.position_ticks, entry.played),
            _ => panic!("the fixture queue holds feed slots"),
        })
        .collect()
}

/// Contract: cast-owner-output row 6.3 (design D7's Finished row) — a natural
/// end records the completed slot through the same observation funnel
/// `TrackCompleted` records through. With the consume policy removing audio
/// slots, the Cast outcome for a naturally finished Feed slot equals the
/// outcome local playback records for the same slot kind: a reference owner
/// that receives the payload a local natural end of an audio item carries
/// (the natural-end position resets, `played` stays video-only, a finished
/// track is consumable — `advance_decision`'s `FinishReason::Natural` outcome)
/// through `apply_track_completed_observation` holds the same canonical queue
/// the Cast advance left behind, and the receiver is still loaded with the
/// successor.
#[test]
fn a_natural_end_records_the_completed_slot_like_local_playback() {
    let items = vec![
        feed_qi_with_url("a", "https://feed/a.mp3"),
        feed_qi_with_url("b", "https://feed/b.mp3"),
    ];
    let mut t = test_loop_with_queue(crate::DaemonRole::Local, items.clone(), 0);
    t.settings.lock().unwrap().consume.audio = true;
    let (_client_id, _rx, calls, t0) = settled_casting_fixture(&mut t);

    report_end(
        &mut t,
        idle_status("https://feed/a.mp3", CastIdleReason::Finished),
        t0 + std::time::Duration::from_secs(120),
    );
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let mut local = owner_with(items, 0);
    let local_completed = local.core.queue.slots()[0].slot_id;
    let player = cold_player();
    let run = player.status.lock().unwrap().sequence_generation;
    assert!(
        apply_track_completed_observation(
            &mut local,
            &player,
            &shared_queue_state(),
            run,
            local_completed,
            0,
            false,
            true,
            t.settings.lock().unwrap().consume,
        ),
        "the local reference records the natural end of the current run"
    );

    assert_eq!(
        feed_completion_state(&t.event_loop.owner),
        feed_completion_state(&local),
        "the Cast natural end's canonical state matches the local-playback outcome"
    );
    assert_eq!(
        feed_completion_state(&t.event_loop.owner),
        vec![("b".to_string(), 0, false)],
        "the completed slot is consumed and the retained successor keeps its progress"
    );
    assert_eq!(
        load_calls(&calls),
        vec![
            "load(https://feed/a.mp3, 0)".to_string(),
            "load(https://feed/b.mp3, 0)".to_string()
        ],
        "the advance still loads the successor on the receiver"
    );
}
