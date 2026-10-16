// cast-owner-output row 6.4 (the Feed piece): the Cast output emits the ctrl
// `PlayerEvent`s that drive the TUI's `persist_feed_slot_lifecycle` — a
// receiver pause change emits `PausedChanged`, a settled seek emits
// `OutputStarted`, a user stop emits `Stopped`, and a natural end emits
// `TrackCompleted` beside the row 6.3 completion record. The Client writes
// the Feed state file from these events; the Owner never does. The events
// ride the same broadcast the event loop's player-event handlers use, and
// only a loaded Feed slot emits them.

use super::*;

use mbv_cast::CastStatus;
use mbv_cast::client::{CastIdleReason, CastPlaybackState};
use mbv_emby_model::TICKS_PER_SECOND;

use super::cast_output::{cast_selection, connect_output_client, pump_cast_event, select_output};
use super::cast_playback::{
    drained_events, feed_qi_with_url, playing_status_at, pump, status_connector,
};
use super::cast_progress::report_status;
use super::cast_resolve::wait_for_worker_jobs;
use super::r#loop::{TestLoop, test_loop_with_queue};

/// A video Feed queue item with an enclosure URL and a known runtime: the
/// stop and completion payloads' near-end and played flags need a non-audio
/// item with a runtime to carry meaning.
fn timed_video_feed_qi(guid: &str, url: &str, duration_ticks: u64) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        enclosure_url: Some(url.into()),
        mime_type: Some("video/mp4".into()),
        feed_kind: Some(mbv_queue::FeedKind::Video),
        duration_ticks: Some(duration_ticks),
        ..feed_qi_with_url(guid, url).as_feed().unwrap().clone()
    })
}

/// The runtime one ten-minute fixture item carries.
fn ten_minutes() -> u64 {
    u64::try_from(600 * TICKS_PER_SECOND).expect("the fixture runtime fits u64")
}

/// Selects the receiver as the output and plays the queue's first slot on
/// it. The fixture transport answers every status poll with an idle status,
/// so no status presents the loaded item until the test reports one.
/// Returns the client id (for commands) and its event channel.
fn loaded_feed_fixture(t: &mut TestLoop) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (connector, _calls) = status_connector(CastStatus::idle());
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

/// The `PlayerEvent`s a client's event channel carries, in order.
fn player_events(rx: &mpsc::Receiver<CtrlOutbound>) -> Vec<PlayerEvent> {
    drained_events(rx)
        .into_iter()
        .filter_map(|event| match event {
            CtrlEvent::Player(pe) => Some(pe),
            _ => None,
        })
        .collect()
}

/// The receiver status that presents `content_id` paused at 10s.
fn paused_status(content_id: &str) -> CastStatus {
    CastStatus {
        state: CastPlaybackState::Paused,
        ..playing_status_at(content_id, 10.0)
    }
}

/// The receiver status that reports `content_id`'s media session finished.
fn finished_status(content_id: &str) -> CastStatus {
    CastStatus {
        state: CastPlaybackState::Idle,
        position_seconds: None,
        duration_seconds: None,
        playback_rate: 1.0,
        playing_content_id: Some(content_id.to_string()),
        idle_reason: Some(CastIdleReason::Finished),
    }
}

/// The owner's current playback generation: the run identity a Cast-emitted
/// event carries, the same field a local run stamps its events with.
fn owner_generation(t: &TestLoop) -> mbv_ctrl::PlaybackGeneration {
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation
}

/// Contract: cast-owner-output row 6.4 (design D9) — a receiver pause change
/// on a loaded Feed slot emits `PausedChanged` to Clients, the event the
/// TUI's `persist_feed_slot_lifecycle` persists the Feed position from. The
/// first presenting status carries no pause change: only a transition
/// between two presenting statuses is one.
#[test]
fn a_receiver_pause_change_on_a_feed_slot_emits_paused_changed_to_clients() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![timed_video_feed_qi(
            "a",
            "https://feed/a.mp4",
            ten_minutes(),
        )],
        0,
    );
    let (_client_id, rx) = loaded_feed_fixture(&mut t);
    let t0 = std::time::Instant::now();

    report_status(&mut t, playing_status_at("https://feed/a.mp4", 10.0), t0);
    assert!(
        !player_events(&rx)
            .iter()
            .any(|event| matches!(event, PlayerEvent::PausedChanged(_))),
        "the first presenting status carries no pause change"
    );

    report_status(
        &mut t,
        paused_status("https://feed/a.mp4"),
        t0 + std::time::Duration::from_secs(120),
    );
    let events = player_events(&rx);
    assert_eq!(
        events.len(),
        1,
        "the pause change is the only event: {events:?}"
    );
    assert!(
        matches!(&events[0], PlayerEvent::PausedChanged(true)),
        "the receiver's pause change emits PausedChanged(true): {:?}",
        events[0]
    );
}

/// Contract: cast-owner-output row 6.4 (design D9) — a natural end of a
/// loaded Feed slot emits `TrackCompleted` to Clients beside the row 6.3
/// completion record, with the recorded outcome as its payload: the position
/// resets, `played` stays video-only, and the finished slot is consumable
/// while the advance found a successor.
#[test]
fn a_natural_end_of_a_feed_slot_emits_track_completed_to_clients() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            timed_video_feed_qi("a", "https://feed/a.mp4", ten_minutes()),
            timed_video_feed_qi("b", "https://feed/b.mp4", ten_minutes()),
        ],
        0,
    );
    let ended_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let (_client_id, rx) = loaded_feed_fixture(&mut t);
    let t0 = std::time::Instant::now();
    report_status(&mut t, playing_status_at("https://feed/a.mp4", 10.0), t0);
    drained_events(&rx);

    report_status(
        &mut t,
        finished_status("https://feed/a.mp4"),
        t0 + std::time::Duration::from_secs(120),
    );
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    let completions: Vec<_> = player_events(&rx)
        .into_iter()
        .filter(|event| matches!(event, PlayerEvent::TrackCompleted { .. }))
        .collect();
    let [
        PlayerEvent::TrackCompleted {
            slot_id,
            run_identity,
            position_ticks,
            played,
            consume,
        },
    ] = &*completions
    else {
        panic!("expected exactly one TrackCompleted: {completions:?}");
    };
    assert_eq!(slot_id, &ended_slot, "the ended slot is named");
    assert_eq!(
        run_identity,
        &owner_generation(&t),
        "the owner's generation"
    );
    assert_eq!(
        (*position_ticks, *played, *consume),
        (0, true, true),
        "the payload is the recorded outcome: position reset, video played, consumable"
    );
}

/// Contract: cast-owner-output row 6.4 (design D9) — a user stop of a loaded
/// Feed slot emits `Stopped` to Clients for the presented position before
/// the presentation is replaced, with the local queue stop's payload: a
/// non-audio item stopped near its end is played and consumable.
#[test]
fn a_user_stop_of_a_feed_slot_emits_stopped_to_clients() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![timed_video_feed_qi(
            "a",
            "https://feed/a.mp4",
            ten_minutes(),
        )],
        0,
    );
    let (client_id, rx) = loaded_feed_fixture(&mut t);
    let stopped_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let t0 = std::time::Instant::now();

    // The receiver presents the loaded item at 570s of 600s: 95%, the local
    // queue stop's near-end boundary. The presented position is what the
    // stop reports.
    report_status(&mut t, playing_status_at("https://feed/a.mp4", 570.0), t0);
    drained_events(&rx);
    let presented_ticks = t.event_loop.player.status.lock().unwrap().position_ticks;

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));

    let events = player_events(&rx);
    assert_eq!(events.len(), 1, "the stop is the only event: {events:?}");
    let PlayerEvent::Stopped {
        slot_id,
        run_identity,
        position_ticks,
        played,
        consume,
        error,
    } = &events[0]
    else {
        panic!("expected a Stopped: {:?}", events[0]);
    };
    assert_eq!(slot_id, &Some(stopped_slot), "the stopped slot is named");
    assert_eq!(
        run_identity,
        &owner_generation(&t),
        "the owner's generation"
    );
    assert_eq!(
        (*position_ticks, *played, *consume, error),
        (presented_ticks, true, true, &None),
        "the payload is the local queue stop's: presented position, near-end played and consume"
    );
}

/// Contract: cast-owner-output row 6.4 (design D9) — a receiver status that
/// presents the loaded Feed slot after a seek command settles the seek and
/// emits `OutputStarted` to Clients, the event the TUI persists the seeked
/// Feed position from; the settle emits once, and a following presenting
/// status without a seek emits nothing.
#[test]
fn a_settling_status_after_a_feed_seek_emits_output_started_to_clients() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![timed_video_feed_qi(
            "a",
            "https://feed/a.mp4",
            ten_minutes(),
        )],
        0,
    );
    let (client_id, rx) = loaded_feed_fixture(&mut t);
    let t0 = std::time::Instant::now();

    report_status(&mut t, playing_status_at("https://feed/a.mp4", 10.0), t0);
    drained_events(&rx);

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::SeekAbsolute(300.0)),
        client_id,
        mpsc::channel().0,
    ));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    report_status(
        &mut t,
        playing_status_at("https://feed/a.mp4", 300.0),
        t0 + std::time::Duration::from_secs(120),
    );
    let events = player_events(&rx);
    assert_eq!(events.len(), 1, "the settle is the only event: {events:?}");
    assert!(
        matches!(&events[0], PlayerEvent::OutputStarted),
        "the settled seek emits OutputStarted to the Clients: {:?}",
        events[0]
    );

    // The settle cleared the marker: the next presenting status emits
    // nothing.
    report_status(
        &mut t,
        playing_status_at("https://feed/a.mp4", 310.0),
        t0 + std::time::Duration::from_secs(240),
    );
    assert!(
        player_events(&rx).is_empty(),
        "a presenting status without a seek emits nothing"
    );
}
