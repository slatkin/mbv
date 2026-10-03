use super::*;
use crate::run::{ForcedJump, StopAction};

const RUNTIME: i64 = 600 * TICKS_PER_SECOND; // 10-minute episode

mod progress;

#[test]
fn cancel_pending_quit_clears_quit_at_and_shutdown_timeout() {
    // Regression test for a code-review finding: cmd_load_new and
    // queue submission (via the shared cancel_pending_quit helper)
    // must reset shutdown_report_timeout, not just quit_at, when a
    // LoadNew/SubmitQueue command cancels an in-flight quit. Otherwise
    // App::teardown -> Player::stop_for_shutdown sets
    // shutdown_report_timeout = Some(quit_timeout) before sending the
    // stop signal; if that quit then gets cancelled by an
    // already-queued LoadNew/SubmitQueue, shutdown_report_timeout
    // would stay Some for the rest of the session, silently degrading
    // every later track transition to the tight shutdown budget/no-retry
    // path instead of the ordinary one. cmd_load_new/cmd_submit_queue
    // themselves aren't unit-tested directly here since they require a
    // real Mpv handle; this exercises the exact reset logic they share.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    session.quit_at = Some(Instant::now());
    *session.shutdown_report_timeout.lock().unwrap() = Some(Duration::from_secs(5));

    session.cancel_pending_quit();

    assert!(session.quit_at.is_none());
    assert!(session.shutdown_report_timeout.lock().unwrap().is_none());
    // progress_join_budget/report_stopped_for_current_context both key off
    // shutdown_report_timeout being None to behave as ordinary mid-playback
    // calls again — asserting the None state above is the load-bearing
    // check; both helpers are exercised directly by other tests.
    assert_eq!(PlaybackRun::progress_join_budget(), Duration::from_secs(30));
}

#[test]
fn playlist_pos_does_not_clobber_pending_initial_playlist_layout() {
    let (mut session, status) = make_queue_session_for_pos_tests(2);

    session.on_playlist_pos_changed(0, 0);

    assert_eq!(session.current_idx, 2);
    assert_eq!(status.lock().unwrap().current_idx, 2);
}

#[test]
fn playlist_pos_does_not_clobber_pending_replace_queue_load() {
    let (mut session, status) = make_queue_session_for_pos_tests(1);
    session.pending_initial_playlist_layout = false;
    session.begin_item_lifecycle(StopAction::NothingPlaying);

    session.on_playlist_pos_changed(0, 0);

    assert_eq!(session.current_idx, 1);
    assert_eq!(status.lock().unwrap().current_idx, 1);
}

#[test]
fn playlist_pos_does_not_clobber_in_flight_jump_to() {
    let (mut session, status) = make_queue_session_for_pos_tests(0);
    session.pending_initial_playlist_layout = false;
    let target = session.slot_id_at(1).unwrap();
    // Rapid Enter on two rows: the in-flight jump also carries its request
    // identity; an intermediate playlist-pos event must not clobber either.
    let transition = crate::transition::Transition::new(42, 1, target);
    session.forced_jump = Some(ForcedJump {
        slot_id: target,
        transition: Some(transition),
        resume_ticks: None,
        from_idle: false,
    });

    session.on_playlist_pos_changed(1, 0);

    assert_eq!(session.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
    assert_eq!(session.forced_jump.map(|jump| jump.slot_id), Some(target));
    assert_eq!(
        session
            .forced_jump
            .and_then(|jump| jump.transition)
            .map(|t| (t.request_id, t.target)),
        Some((42, target)),
        "the in-flight jump's request identity survives an intermediate playlist-pos event"
    );
}

#[test]
fn idle_jump_settles_from_playback_restart_and_emits_the_transition_observation() {
    let (mut session, status, events, http) = make_queue_session_for_pos_tests_with_mock(0);
    session.pending_initial_playlist_layout = false;
    let slot_id = session.slot_id_at(1).unwrap();
    let transition = crate::transition::Transition::new(42, 7, slot_id);
    session.forced_jump = Some(ForcedJump {
        slot_id,
        transition: Some(transition),
        resume_ticks: None,
        from_idle: true,
    });
    status.lock().unwrap().active = false;
    // `report_active_item` uses the same mocked Emby transport; supply the
    // expected successful responses so the test stays synchronous and hermetic.
    for body in ["", "{}", "", "{}", ""] {
        http.respond(200, body);
    }

    let settled = session.settle_idle_jump_on_restart(0);
    assert_eq!(settled, Some((slot_id, Some(transition))));
    let (settled_slot, settled_transition) = settled.unwrap();
    session.emit_track_changed(settled_slot, settled_transition);

    assert_eq!(session.current_idx, 1);
    assert!(status.lock().unwrap().active);
    assert_eq!(session.forced_jump, None);
    assert!(matches!(
        events.try_recv(),
        Ok(PlayerEvent::TrackChanged {
            slot_id: observed,
            transition: Some((42, 7)),
        }) if observed == slot_id
    ));
}

#[test]
fn idle_jump_to_removed_slot_clears_the_pending_transition() {
    let (mut session, _status, _events) = make_queue_session_for_pos_tests_with_events(0);
    let missing = QueueSlotId::from_raw(u64::MAX);
    let transition = crate::transition::Transition::new(42, 7, missing);
    session.forced_jump = Some(ForcedJump {
        slot_id: missing,
        transition: Some(transition),
        resume_ticks: None,
        from_idle: true,
    });

    assert_eq!(session.settle_idle_jump_on_restart(0), None);

    assert_eq!(session.forced_jump, None);
}

#[test]
fn playlist_pos_updates_idle_queue_with_valid_mpv_position() {
    let (mut session, status, events, http) = make_queue_session_for_pos_tests_with_mock(0);
    session.pending_initial_playlist_layout = false;
    // Adoption reports the abandoned item stopped, resolves the new item via
    // PlaybackInfo, and starts it — all succeed instantly on the mock (plus
    // slack for any extra bookkeeping calls).
    http.respond(200, "");
    http.respond(200, "{}");
    http.respond(200, "");
    http.respond(200, "{}");
    http.respond(200, "");

    session.on_playlist_pos_changed(2, 0);

    assert_eq!(session.current_idx, 2);
    assert_eq!(status.lock().unwrap().current_idx, 2);
    // Nothing asked for this move, so mpv is authoritative for what is playing
    // now: the adoption has to be announced, or the owner and the UI keep
    // reporting the entry mpv left.
    let announced = events.try_iter().find_map(|event| match event {
        PlayerEvent::TrackChanged {
            slot_id,
            transition,
        } => Some((slot_id, transition)),
        _ => None,
    });
    assert_eq!(
        announced,
        Some((session.slot_id_at(2).unwrap(), None)),
        "an mpv-initiated move is announced as a natural track change"
    );
    // The abandoned item was actually reported stopped, not just left behind.
    assert!(
        http.requests()
            .iter()
            .any(|r| r.starts_with("POST /Sessions/Playing/Stopped")),
    );
}

#[test]
fn append_items_to_queue_extends_queue_without_moving_current_idx() {
    let (mut session, status) = make_queue_session_for_pos_tests(1);
    let appended = make_media_item("ep4");

    session.append_items_to_queue(owner_paired(vec![QueueItem::Emby(Box::new(
        appended.clone(),
    ))]));

    assert_eq!(session.queue_len(), 4);
    assert_eq!(session.current_idx, 1);
    let status = status.lock().unwrap();
    assert_eq!(status.current_idx, 1);
    assert_eq!(status.queue_len, 4);
    assert_eq!(
        session
            .queue
            .slots()
            .last()
            .map(|slot| slot.item.id().to_string()),
        Some(appended.id.clone())
    );
}

#[test]
fn deferred_stop_keeps_the_slot_observed_at_end_file_not_the_one_now_at_that_index() {
    // Task 2.3 / design D2: the Queue+Quit end-file defers its Stopped emit
    // until the mpv Shutdown event. A QueueMove drained in between must not
    // change which occurrence the event names. `stop_slot` is the identity
    // captured when the stop was first observed; here it points at ep2 while
    // the ordinal it used to occupy now holds ep3's slot.
    let (mut session, _status, events) = make_queue_session_for_pos_tests_with_events(1);
    let observed = session.active_slot_id().expect("active slot at end-file");
    let displaced = session.slot_id_at(2).expect("slot at index 2");
    session.stop_slot = Some(observed); // captured in on_end_file's Queue+Quit path
    session.mark_reported(StopReport::Sent); // skip the reporter side effects
    let mut progress = noop_progress();

    // QueueMove drained between the end-file and the Shutdown event.
    assert!(session.queue.move_slot(observed, 2));
    assert_eq!(session.slot_id_at(1), Some(displaced));

    session.on_shutdown(&mut progress);

    let event = events.recv().unwrap();
    let PlayerEvent::Stopped { slot_id, .. } = event else {
        panic!("expected Stopped event");
    };
    assert_eq!(slot_id, Some(observed));
}

#[test]
fn load_new_serde_roundtrip() {
    let cmd = PlayerCommand::LoadNew {
        url: "http://emby.local/Videos/ep1/stream".into(),
        start_pos: 0.0,
        item: Box::new(make_media_item("ep1")),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: PlayerCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, PlayerCommand::LoadNew { .. }));
}

#[test]
fn shutdown_stop_sets_timeout_without_changing_plain_stop() {
    let (event_tx, _event_rx) = mpsc::channel();
    let player = Player::new(
        String::new(),
        String::new(),
        false,
        false,
        false,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    );

    let (plain_tx, plain_rx) = mpsc::channel();
    *player.stop_tx.lock().unwrap() = Some(plain_tx);
    player.stop();
    plain_rx.recv_timeout(Duration::from_millis(50)).unwrap();
    assert!(player.shutdown_report_timeout.lock().unwrap().is_none());

    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    *player.stop_tx.lock().unwrap() = Some(shutdown_tx);
    player.stop_for_shutdown(Duration::from_secs(7));
    shutdown_rx.recv_timeout(Duration::from_millis(50)).unwrap();
    assert_eq!(
        *player.shutdown_report_timeout.lock().unwrap(),
        Some(Duration::from_secs(7))
    );
}

#[test]
fn end_file_quit_uses_shutdown_aware_stop_report_context() {
    assert_eq!(
        end_file_stop_report_context(mpv_end_file_reason::Quit),
        StopReportContext::ShutdownAware
    );
    assert_eq!(
        end_file_stop_report_context(mpv_end_file_reason::Eof),
        StopReportContext::Ordinary
    );
    assert_eq!(
        end_file_stop_report_context(mpv_end_file_reason::Error),
        StopReportContext::Ordinary
    );
}

#[test]
fn superseded_jump_end_file_is_dropped_without_a_pending_transition() {
    let slot_id = QueueSlotId::from_raw(2);
    let rearmed_jump = ForcedJump {
        slot_id,
        transition: None,
        resume_ticks: Some(42),
        from_idle: false,
    };
    let in_flight_jump = ForcedJump {
        transition: Some(crate::transition::Transition::new(42, 1, slot_id)),
        ..rearmed_jump
    };

    assert!(is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        None,
        false
    ));
    assert!(is_superseded_jump_end_file(
        mpv_end_file_reason::Redirect,
        None,
        false
    ));
    assert!(is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        Some(rearmed_jump),
        false
    ));
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        Some(in_flight_jump),
        false
    ));
    // A finished track (EOF/near-end/next-up) always advances.
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        None,
        true
    ));
    // A playback error still skips the broken track forward.
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Error,
        None,
        false
    ));
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Eof,
        None,
        false
    ));
}

#[test]
fn progress_guard_stop_and_join_bounded_when_thread_hangs() {
    let (stop_tx, _stop_rx) = mpsc::channel();
    let handle = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
    });
    let mut guard = ProgressGuard {
        stop_tx,
        handle: Some(handle),
    };

    let started = std::time::Instant::now();
    // Small budget on purpose: the bound is enforced by recv_timeout, so any
    // budget proves boundedness; a small one keeps the test fast. The 5s hang
    // still exceeds the 1s assertion, so an unbounded-join regression fails
    // instead of passing slowly.
    guard.stop_and_join(Duration::from_millis(20));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "stop_and_join should return near its 20ms budget, took {elapsed:?}"
    );
    assert!(
        guard.handle.is_none(),
        "handle should be taken regardless of outcome"
    );
}

#[test]
fn progress_guard_stop_and_join_fast_when_thread_finishes_quickly() {
    let (stop_tx, _stop_rx) = mpsc::channel();
    let handle = std::thread::spawn(|| {});
    let mut guard = ProgressGuard {
        stop_tx,
        handle: Some(handle),
    };

    let started = std::time::Instant::now();
    guard.stop_and_join(Duration::from_secs(30));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "a thread that finishes immediately should not add latency, took {elapsed:?}"
    );
}

#[test]
fn player_join_or_timeout_does_not_wait_for_a_stuck_run() {
    let (event_tx, _event_rx) = mpsc::channel();
    let player = Player::new(
        String::new(),
        String::new(),
        false,
        false,
        false,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    );
    *player.thread_handle.lock().unwrap() = Some(std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
    }));

    let started = Instant::now();
    // Small budget on purpose: the bound is a recv_timeout, so any budget
    // proves boundedness; a small one keeps the test fast. The 5s hang
    // still exceeds the 1s assertion, so an unbounded-join regression fails
    // instead of passing slowly.
    player.join_or_timeout(Duration::from_millis(20));

    assert!(
        started.elapsed() < Duration::from_secs(1),
        "player teardown exceeded its bound: {:?}",
        started.elapsed()
    );
}

#[test]
fn subtitle_stream_index_maps_to_mpv_subtitle_id() {
    let status = PlayerStatus {
        active: true,
        sub_tracks: vec![(1, "English".to_string(), false)],
        sub_track_stream_indexes: vec![(1, 2)],
        video_height: 1080,
        ..Default::default()
    };

    assert_eq!(status.subtitle_stream_index_to_mpv_id(2), Some(1));
    assert_eq!(status.subtitle_stream_index_to_mpv_id(-1), Some(0));
    assert_eq!(status.subtitle_stream_index_to_mpv_id(1), None);
}

// ── PlayerStatus::next_idx / previous_idx / toggle_to_reach ──────────────
// (issue #80: single source of truth for next/previous/toggle-play bounds
// and paused-state logic, replacing four near-identical copies.)

#[test]
fn standalone_natural_end_file_marks_status_inactive() {
    // Regression: a daemon's Standalone run reached natural EOF, reported
    // Stopped + mark_played to Emby, and idled — but the shared status
    // snapshot kept active=true at the final position. A client attaching
    // later inherited a "still playing, 1s left" now-playing panel, and
    // every StatusOnly broadcast kept replaying it.
    let (mut session, status, events, http) = make_queue_session_for_pos_tests_with_mock(0);
    session.origin = PlaybackOrigin::Standalone;
    status.lock().unwrap().position_ticks = RUNTIME - TICKS_PER_SECOND;
    // Script the Stopped + mark_played responses the EOF path will send.
    http.respond(200, "");
    http.respond(200, "");
    let mut progress = noop_progress();

    assert!(!session.on_end_file_standalone(mpv_end_file_reason::Eof, &mut progress));

    assert!(
        !status.lock().unwrap().active,
        "EOF must deactivate the status snapshot"
    );
    let PlayerEvent::Stopped { played, .. } = events.try_recv().unwrap() else {
        panic!("expected Stopped event");
    };
    assert!(played, "natural video EOF must surface as played");
}

/// Regression (2026-09-28 music queue): an active-file `JumpTo` left the status
/// mirror on the queue's first item, so the playing row kept its duration.
#[test]
fn active_file_jump_points_status_at_jumped_item() {
    let (mut session, status, _events, http) = make_queue_session_for_pos_tests_with_mock(0);
    let (first, target) = (
        session.slot_id_at(0).unwrap(),
        session.slot_id_at(1).unwrap(),
    );
    let target_item = mbv_emby_model::EmbyItem {
        runtime_ticks: 158 * TICKS_PER_SECOND,
        ..make_media_item("ep2")
    };
    session.queue = mbv_queue::ExecutionSequence::from_slot_items(
        vec![
            (first, QueueItem::Emby(Box::new(make_media_item("ep1")))),
            (target, QueueItem::Emby(Box::new(target_item))),
        ],
        Some(target),
    );
    session.current_idx = 1;
    for body in ["", "{}", "", "{}", ""] {
        http.respond(200, body);
    }

    session.report_jumped_item(0, &mut noop_progress());

    assert_eq!(status.lock().unwrap().runtime_ticks, 158 * TICKS_PER_SECOND);
}
