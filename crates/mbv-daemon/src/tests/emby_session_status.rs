// emby-session-owner-output row 5.1: the Owner projects an Emby Session
// observation into its presented `PlayerStatus` (design D10) — the position
// estimate, the pause, volume, and mute state, the track lists and their
// selected ids, the video height, and the title fields — and presents content
// it did not load as `now_playing` without moving the active slot. The tests
// drive the daemon tick with the observation's own time, so no test waits.

use std::time::Instant;

use rstest::rstest;

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::emby_session_output::{
    host_session_output, poll_once, pump_session_event, session_selection, wait_for_session_jobs,
};
use super::r#loop::{TestLoop, test_loop_with_queue};
use super::*;

use crate::cast::PlaybackOutput;
use crate::emby_session::fake::{FakeSessionApi, session};
use crate::emby_session::poll::EMBY_SESSION_POLL_INTERVAL;
use mbv_ctrl::{OutputConnection, OutputState, SessionNowPlaying};
use mbv_emby::{SessionAudioStream, SessionInfo, SessionMediaInfo, SessionSubtitleStream};

/// One second of media in ticks.
const SECOND_TICKS: i64 = mbv_emby_model::TICKS_PER_SECOND;

/// The 600 seconds of media the scripted Session reports.
const RUNTIME_TICKS: i64 = 600 * SECOND_TICKS;

/// A Session entry that presents `item_id` at 10 s of a 600 s item, with the
/// media streams and the device state a `/Sessions` payload carries.
fn session_presenting(item_id: &str) -> SessionInfo {
    SessionInfo {
        now_playing: Some(item_id.to_string()),
        now_playing_item_id: Some(item_id.to_string()),
        now_playing_item_type: Some("Movie".to_string()),
        position_s: 10,
        position_ticks: 10 * SECOND_TICKS,
        runtime_s: 600,
        runtime_ticks: RUNTIME_TICKS,
        volume: 42,
        muted: true,
        audio_index: 1,
        sub_index: -1,
        media_info: SessionMediaInfo {
            video_label: "1080p H264".to_string(),
            audio_only: false,
            audio_streams: vec![
                SessionAudioStream {
                    index: 1,
                    label: "English AC3 5.1".to_string(),
                    language: "eng".to_string(),
                },
                SessionAudioStream {
                    index: 2,
                    label: "Japanese AAC Stereo".to_string(),
                    language: "jpn".to_string(),
                },
            ],
            subtitle_streams: vec![
                SessionSubtitleStream {
                    index: 3,
                    label: "English".to_string(),
                    language: "eng".to_string(),
                    forced: false,
                },
                SessionSubtitleStream {
                    index: 4,
                    label: "English (Forced)".to_string(),
                    language: "eng".to_string(),
                    forced: true,
                },
            ],
        },
        ..session("device-1", "session-1")
    }
}

/// The loop with the fake's Session selected as the output over `items` and
/// the queue's first slot loaded onto it, plus the fake's script handle.
fn loaded_session_output(items: Vec<QueueItem>) -> (TestLoop, FakeSessionApi) {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let mut t = test_loop_with_queue(crate::DaemonRole::Local, items, 0);
    host_session_output(&mut t, fake.clone());
    let (client_id, reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);
    (t, fake)
}

/// When the last observation was taken, so a test drives the daemon tick from
/// it instead of waiting.
fn observed_at(t: &TestLoop) -> Instant {
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    output.observed_at.expect("the poll stored the observation")
}

/// Contract: row 5.1 / design D6 and D10 — the presented position is the last
/// observed position plus the time since that observation while the Session
/// plays, and it holds where the Session paused.
#[rstest]
#[case::playing(false, 15)]
#[case::paused(true, 10)]
fn the_presented_position_advances_while_playing_and_holds_while_paused(
    #[case] is_paused: bool,
    #[case] expected_seconds: i64,
) {
    let (mut t, script) = loaded_session_output(vec![emby_qi("movie", "Video", "Movie")]);
    script.set_sessions(vec![SessionInfo {
        is_paused,
        ..session_presenting("movie")
    }]);

    poll_once(&mut t, Instant::now());
    t.event_loop.tick(observed_at(&t) + Duration::from_secs(5));

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert_eq!(status.position_ticks, expected_seconds * SECOND_TICKS);
    assert_eq!(status.paused, is_paused, "the pause state is observed");
    assert!(status.active, "a presenting Session is active playback");
    assert_eq!(
        status.title, "movie",
        "the loaded slot's queue item names the presented title"
    );
    assert_eq!(
        status.runtime_ticks, RUNTIME_TICKS,
        "the Session's own runtime stands in for the queue item's absent one"
    );
    assert_eq!(status.volume, 42, "the device volume is presented");
    assert_eq!(status.volume_max, 100, "a Session's volume scale is 0-100");
    assert!(status.muted, "the device mute state is presented");
}

/// Contract: row 5.1 / design D9 and D10 — the Session's streams become the
/// presented track lists, each stream index is the id the Owner's own track
/// commands address, and Emby's -1 subtitle index is the wire id 0 (off).
#[test]
fn session_streams_map_to_the_presented_track_lists() {
    let (mut t, script) = loaded_session_output(vec![emby_qi("movie", "Video", "Movie")]);
    script.set_sessions(vec![session_presenting("movie")]);

    poll_once(&mut t, Instant::now());

    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert_eq!(
        status.audio_tracks,
        vec![
            (1, "English AC3 5.1".to_string()),
            (2, "Japanese AAC Stereo".to_string()),
        ],
        "each audio stream is one presented track, by its Emby index"
    );
    assert_eq!(
        status.sub_tracks,
        vec![
            (3, "English".to_string(), false),
            (4, "English (Forced)".to_string(), true),
        ],
        "each subtitle stream is one presented track, with its forced flag"
    );
    assert_eq!(
        (status.audio_id, status.sub_id),
        (1, 0),
        "the selected audio index is presented and Emby's -1 is the off id"
    );
    assert_eq!(
        (status.audio_lang.as_str(), status.sub_lang.as_str()),
        ("eng", ""),
        "the selected audio language is presented and an off subtitle has none"
    );
    assert_eq!(
        status.video_height, 1080,
        "the media label carries the height"
    );
}

/// Contract: row 5.1 / design D3 and D10 and spec "Content the owner did not
/// load is presented, not advanced" — an item the Owner did not load reaches
/// the ctrl output state as `now_playing` and the presented title, while the
/// active slot stays where the Owner left it. The same item in the next
/// observation changes nothing, so no second state event goes out.
#[test]
fn content_the_owner_did_not_load_sets_now_playing_and_keeps_the_active_slot() {
    let (mut t, script) = loaded_session_output(vec![emby_qi("movie", "Video", "Movie")]);
    let (_monitor_id, _monitor_tx, monitor_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let slot = t
        .event_loop
        .owner
        .core
        .queue
        .active_slot_id()
        .expect("the played slot is active");
    let revision = t.event_loop.owner.core.queue.revision();
    script.set_sessions(vec![SessionInfo {
        now_playing: Some("Another item".to_string()),
        now_playing_item_type: Some("Episode".to_string()),
        now_playing_series_id: Some("series-1".to_string()),
        now_playing_series_name: Some("Another show".to_string()),
        now_playing_logo_etag: Some("logo-1".to_string()),
        ..session_presenting("other-item")
    }]);

    poll_once(&mut t, Instant::now());

    let expected = OutputState::EmbySession {
        device_id: "device-1".to_string(),
        name: "Living room TV".to_string(),
        connection: OutputConnection::Connected,
        audio_only: false,
        now_playing: Some(SessionNowPlaying {
            item_id: "other-item".to_string(),
            item_type: "Episode".to_string(),
            name: "Another item".to_string(),
            series_id: Some("series-1".to_string()),
            series_name: Some("Another show".to_string()),
            logo_tag: Some("logo-1".to_string()),
        }),
    };
    assert_eq!(
        format!("{:?}", recv_event(&monitor_rx)),
        format!("{:?}", CtrlEvent::PlaybackOutput(expected)),
        "the device's own item is presented as now_playing"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.active_slot_id(),
        Some(slot),
        "content the Owner did not load moves no active slot"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.revision(),
        revision,
        "content the Owner did not load changes no queue revision"
    );
    let status = t.event_loop.player.status.lock().unwrap().clone();
    assert_eq!(
        status.title, "Another item",
        "the item's own name is presented"
    );
    assert_eq!(
        status.art_item_id, "",
        "content the Owner did not load has no queue item identity"
    );
    assert!(
        (10 * SECOND_TICKS..11 * SECOND_TICKS).contains(&status.position_ticks),
        "the device's own position is presented"
    );

    poll_once(&mut t, Instant::now() + EMBY_SESSION_POLL_INTERVAL);

    assert!(
        drained_events(&monitor_rx).is_empty(),
        "an unchanged now-playing item broadcasts no second state"
    );
}
