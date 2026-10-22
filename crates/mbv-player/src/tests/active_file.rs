use super::*;
use crate::run::ForcedJump;
use std::sync::Mutex;

/// The active-file projection holds no armed jump: a jump armed earlier in
/// the playlist projection must not survive the switch into the projection.
/// It keeps one entry in mpv's playlist and never moves mpv with a
/// `playlist-pos` write, so an armed jump from the playlist projection names
/// a slot nothing there is moving to — a promoted entry's later
/// `PlaybackRestart` would otherwise settle the stale arm (the from-idle
/// shape below) and report the old jump target as the active slot while mpv
/// plays the projection's own entry.
#[test]
fn entering_the_active_file_projection_clears_an_armed_playlist_jump() {
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    let target = session.slot_id_at(1).unwrap();
    // The arm shape a playlist jump arms when the run is idle: its target is
    // settled by a `PlaybackRestart`, not by an outgoing end-file.
    session.set_pending_playlist_jump(ForcedJump {
        slot_id: target,
        transition: Some(crate::transition::Transition::new(42, 1, target)),
        resume_ticks: None,
        from_idle: true,
    });

    session.run_lifecycle.enter_active_file();

    assert!(
        session.run_lifecycle.jump_arm().is_idle(),
        "the active-file projection holds no armed jump"
    );
}

/// Shared fixture for the failed active-file jump tests: rebuilds the
/// session's queue over the session's own slot identities with `items`
/// (start = slot 0), lifts the active-file projection, scripts the two
/// outgoing stop-report responses, and pairs a test mpv. The far socket end
/// travels with the tuple so the mpv connection stays open for the caller.
fn failed_active_file_jump_fixture(
    items: Vec<QueueItem>,
) -> (
    PlaybackRun,
    Arc<Mutex<PlayerStatus>>,
    mpsc::Receiver<PlayerEvent>,
    Mpv,
    std::os::unix::net::UnixStream,
) {
    let (mut session, status, events, http) = make_queue_session_for_pos_tests_with_mock(0);
    let slot_ids: Vec<QueueSlotId> = (0..items.len())
        .map(|idx| session.slot_id_at(idx).unwrap())
        .collect();
    let holder = slot_ids[0];
    session.queue = mbv_queue::ExecutionSequence::from_slot_items(
        slot_ids.into_iter().zip(items).collect(),
        Some(holder),
    );
    session.current_idx = 0;
    session.run_lifecycle.enter_active_file();
    // The outgoing item's stop report is the only HTTP call the failure makes.
    http.respond(200, "");
    http.respond(200, "");
    let (ours, peer) = std::os::unix::net::UnixStream::pair().expect("socket pair");
    let mpv =
        Mpv::for_test(crate::mpv_session::ipc::IpcSession::from_stream(ours).expect("session"));
    (session, status, events, mpv, peer)
}

/// Spec requirement "An active-file selection that fails ends the run",
/// scenario "An active-file jump fails": a selection failure during a jump
/// into the active-file projection ends the run with one `Stopped` that
/// carries the error and names the slot the run tried to select.
#[test]
fn failed_active_file_jump_reports_one_stopped_naming_the_target_slot() {
    let (mut session, status, events, mpv, _peer) =
        failed_active_file_jump_fixture(vec![abs_item(), abs_item(), abs_book_item()]);
    let target = session.slot_id_at(1).unwrap();
    let mut progress = noop_progress();

    session.handle_command(
        PlayerCommand::JumpTo {
            slot_id: target,
            request_id: 42,
            generation: 1,
            resume_ticks: None,
        },
        &mpv,
        &mut progress,
    );

    let event = events
        .try_recv()
        .expect("a failed active-file jump reports one Stopped");
    let PlayerEvent::Stopped { slot_id, error, .. } = event else {
        panic!("expected a Stopped event, got {event:?}");
    };
    assert_eq!(
        slot_id,
        Some(target),
        "the stop names the failed selection target"
    );
    assert!(error.is_some(), "the stop carries the selection error");
    assert!(
        events.try_recv().is_err(),
        "a failed active-file jump ends the run exactly once"
    );
    assert!(
        !status.lock().unwrap().active,
        "the run must not stay marked active with nothing selected"
    );
}

/// The failure teardown's own `mpv stop` produces an `EndFile`, and the run
/// must drain it like a failed replacement does (`begin_item_lifecycle`
/// arms the replacement drain) — a real end-file here would advance the
/// parked run or, on a standalone run, tear it down
/// (`on_end_file_standalone` returns false, ending `run()`).
#[test]
fn a_failed_active_file_jump_drains_its_teardown_end_file_and_parks_the_run() {
    let (mut session, status, events, mpv, _peer) =
        failed_active_file_jump_fixture(vec![abs_item(), abs_item()]);
    let holder = session.slot_id_at(0).unwrap();
    let target = session.slot_id_at(1).unwrap();
    let mut progress = noop_progress();

    session.handle_command(
        PlayerCommand::JumpTo {
            slot_id: target,
            request_id: 42,
            generation: 1,
            resume_ticks: None,
        },
        &mpv,
        &mut progress,
    );
    let PlayerEvent::Stopped { .. } = events
        .try_recv()
        .expect("the failed jump reports its one Stopped first")
    else {
        panic!("expected the failed jump's Stopped");
    };
    // The teardown's stop ends the (parked) run's playback; a standalone
    // run's end-file handler would otherwise end it for real.
    session.origin = PlaybackOrigin::Standalone;

    let continues = session.on_end_file(EndFileReason::Stop, &mpv, &mut progress);

    assert!(continues, "the drained end-file keeps the event loop alive");
    assert!(
        events.try_recv().is_err(),
        "the drained end-file must not produce a second Stopped"
    );
    assert_eq!(session.current_idx, 0, "no advance out of the parked slot");
    assert_eq!(
        session.active_slot_id(),
        Some(holder),
        "no advance happened"
    );
    assert!(
        !status.lock().unwrap().active,
        "the parked run stays deactivated"
    );
}

pub(crate) fn noop_progress() -> ProgressGuard {
    let (stop_tx, _) = mpsc::channel();
    ProgressGuard {
        stop_tx,
        handle: None,
    }
}

pub(crate) fn abs_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "show".into(),
            episode_id: "episode".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: Some(100_u64),
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        },
    ))
}

pub(crate) fn abs_book_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(
        mbv_queue::AudiobookshelfBookQueueItem {
            library_item_id: "book".into(),
            title: "Book".into(),
            author: None,
            duration_ticks: Some(100_u64),
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: None,
        },
    ))
}
