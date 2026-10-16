// cast-owner-output row 6.4 (the Emby half): the Owner drives the loaded
// item's provider reporting from the receiver's status — the session start
// goes out once, on the first status that presents the loaded item, progress
// is reported only while the receiver's reported media is the item the Owner
// loaded (spec "Progress is reported while mbv is attached"), and an ended
// item's stop report goes out before the next item's start report (design
// D9). The reporter boundary is mocked with a recording double; the real
// reporter behind it is the player's `SessionReporter`, built from the
// resolved `CastProgressTarget` ids.

use super::*;

use mbv_cast::CastStatus;
use mbv_cast::client::{CastIdleReason, CastPlaybackState};

use super::cast_output::{cast_selection, connect_output_client, pump_cast_event, select_output};
use super::cast_playback::{feed_qi_with_url, playing_status_at, pump, status_connector};
use super::cast_resolve::wait_for_worker_jobs;
use super::r#loop::{TestLoop, test_loop_with_queue};
use crate::cast::progress::CastProgressReporting;

/// The reporter boundary's recording double (design D9): every report the
/// Owner drives lands in the shared log under the item's name, so a test can
/// assert which reports ran and in what order without a live Emby.
struct RecordingReporter {
    name: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

impl CastProgressReporting for RecordingReporter {
    fn report_start(&self) {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:start", self.name));
    }

    fn report_progress(&self) {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:progress", self.name));
    }

    fn report_stop(&self, _position_ticks: i64) {
        self.log.lock().unwrap().push(format!("{}:stop", self.name));
    }
}

/// Selects the receiver as the output and loads the queue's first slot on
/// it. The fixture transport answers every status poll with an idle status,
/// so no status presents the loaded item until the test reports one. Returns
/// the selecting client's event channel: the test holds it for the client's
/// lifetime, since a dropped receiver reads as a disconnected Client.
fn loaded_fixture(t: &mut TestLoop) -> mpsc::Receiver<CtrlOutbound> {
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
    rx
}

/// Installs the named recording double as the loaded item's reporting.
fn install_reporting(t: &mut TestLoop, name: &'static str, log: Arc<Mutex<Vec<String>>>) {
    let crate::cast::PlaybackOutput::Cast(cast) = &mut t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    cast.loaded
        .as_mut()
        .expect("the load records the item")
        .reporting = Some(Box::new(RecordingReporter { name, log }));
}

/// The cast URL the ABS resolve builds for the fixture episode (an IP
/// literal, so nothing here can resolve a name).
const ABS_EPISODE_URL: &str = "http://127.0.0.1:1/episode.mp3";

/// Installs the ABS episode `slot` as the receiver's loaded item, the state
/// the resolve outcome installs in production: the episode's cast URL as the
/// content id, the Audiobookshelf progress target, and the named recording
/// double as its reporting. The resolve's own bounded session open is unit
/// 4.1's tested territory; the boundary under test here is the reporting the
/// loaded item drives.
fn install_abs_loaded(
    t: &mut TestLoop,
    slot: mbv_queue::QueueSlotId,
    log: Arc<Mutex<Vec<String>>>,
) {
    let crate::cast::PlaybackOutput::Cast(cast) = &mut t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    cast.loaded = Some(crate::cast::CastLoadedItem {
        slot,
        content_id: ABS_EPISODE_URL.to_string(),
        progress: crate::cast_resolve::CastProgressTarget::AudiobookshelfEpisode {
            session_id: "abs-session".to_string(),
            duration_seconds: 600.0,
        },
        reporting: Some(Box::new(RecordingReporter { name: "abs", log })),
        start_reported: false,
        end_handled: false,
    });
}

/// Reports `status` as the receiver's next poll result (design D7's
/// injection): the tick's poll is drained unapplied and `status` is
/// delivered under its poll id, the same injection the drift-correction test
/// uses.
pub(super) fn report_status(t: &mut TestLoop, status: CastStatus, at: std::time::Instant) {
    t.event_loop.tick(at);
    wait_for_worker_jobs(&t.event_loop.owner.output);
    let event = t.merged_rx.try_recv().expect("the poll reported a result");
    let DaemonEvent::Cast(crate::cast::CastOwnerEvent::Status { poll_id, .. }) = event else {
        panic!("expected the poll's status result");
    };
    t.event_loop
        .handle_event(DaemonEvent::Cast(crate::cast::CastOwnerEvent::Status {
            receiver_id: "rcv-1".to_string(),
            poll_id,
            result: Ok(status),
        }));
}

/// Contract: cast-owner-output row 6.4 (spec "Progress is reported while mbv
/// is attached") — the Owner reports the loaded item's progress only while
/// the receiver's reported media is that item: the first presenting status
/// reports the session start, every presenting status reports progress, and
/// a status naming foreign media reports nothing.
#[test]
fn progress_is_reported_only_while_the_receiver_plays_the_loaded_item() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![feed_qi_with_url("a", "https://feed/a.mp3")],
        0,
    );
    let _rx = loaded_fixture(&mut t);
    let log = Arc::new(Mutex::new(Vec::new()));
    install_reporting(&mut t, "a", Arc::clone(&log));
    let t0 = std::time::Instant::now();

    report_status(&mut t, playing_status_at("https://feed/a.mp3", 10.0), t0);
    report_status(
        &mut t,
        playing_status_at("https://feed/other.mp3", 30.0),
        t0 + std::time::Duration::from_secs(120),
    );
    report_status(
        &mut t,
        playing_status_at("https://feed/a.mp3", 11.0),
        t0 + std::time::Duration::from_secs(240),
    );

    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "a:start".to_string(),
            "a:progress".to_string(),
            "a:progress".to_string(),
        ],
        "the start and the presenting statuses' progress are reported; the foreign media reports nothing"
    );
}

/// Contract: cast-owner-output row 6.4 (design D9) — a natural end reports
/// the ended item's stop before the advance's next load reports its start:
/// the stop goes out when the end is handled, the replacement load's start
/// on its first presenting status, in that order.
#[test]
fn an_ended_item_reports_stop_before_the_next_item_reports_start() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    let _rx = loaded_fixture(&mut t);
    let log = Arc::new(Mutex::new(Vec::new()));
    install_reporting(&mut t, "a", Arc::clone(&log));
    let t0 = std::time::Instant::now();
    report_status(&mut t, playing_status_at("https://feed/a.mp3", 10.0), t0);

    report_status(
        &mut t,
        CastStatus {
            state: CastPlaybackState::Idle,
            position_seconds: None,
            duration_seconds: None,
            playback_rate: 1.0,
            playing_content_id: Some("https://feed/a.mp3".to_string()),
            idle_reason: Some(CastIdleReason::Finished),
        },
        t0 + std::time::Duration::from_secs(120),
    );
    pump(&mut t);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    install_reporting(&mut t, "b", Arc::clone(&log));
    report_status(
        &mut t,
        playing_status_at("https://feed/b.mp3", 10.0),
        t0 + std::time::Duration::from_secs(240),
    );

    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "a:start".to_string(),
            "a:progress".to_string(),
            "a:stop".to_string(),
            "b:start".to_string(),
            "b:progress".to_string(),
        ],
        "the ended item's stop precedes the next item's start"
    );
}

/// Contract: cast-owner-output row 6.4, the Audiobookshelf half (design D9,
/// spec "Progress is reported while mbv is attached") — the episode lifecycle
/// a loaded ABS episode drives is observed and synced only while the
/// receiver's reported media is that episode, and the session closes on a
/// user stop. The recording double stands in for the lifecycle-backed
/// reporter: its start/progress records are the lifecycle's observe and
/// sync, its stop record the session close.
#[test]
fn an_abs_episode_is_synced_only_while_the_receiver_plays_it_and_closed_on_stop() {
    let mut t = test_loop_with_queue(crate::DaemonRole::Local, vec![abs_qi("li_1", "ep_1")], 0);
    let (connector, _calls) = status_connector(CastStatus::idle());
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let log = Arc::new(Mutex::new(Vec::new()));
    install_abs_loaded(&mut t, slot, Arc::clone(&log));
    let t0 = std::time::Instant::now();

    report_status(&mut t, playing_status_at(ABS_EPISODE_URL, 10.0), t0);
    report_status(
        &mut t,
        playing_status_at("http://127.0.0.1:2/other.mp3", 30.0),
        t0 + std::time::Duration::from_secs(120),
    );

    // The user stop closes the loaded episode's session (design D9's stop
    // ordering: the close goes out for the presented position).
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::Stop,
        client_id,
        mpsc::channel().0,
    ));

    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "abs:start".to_string(),
            "abs:progress".to_string(),
            "abs:stop".to_string(),
        ],
        "the presenting status starts and syncs the episode's session, the foreign media reports nothing, the stop closes it"
    );
}
