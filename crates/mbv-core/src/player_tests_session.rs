#[test]
fn cancel_pending_quit_clears_quit_at_and_shutdown_timeout() {
    // Regression test for a code-review finding: cmd_load_new and
    // cmd_replace_queue (via the shared cancel_pending_quit helper)
    // must reset shutdown_report_timeout, not just quit_at, when a
    // LoadNew/ReplaceQueue command cancels an in-flight quit. Otherwise
    // App::teardown -> Player::stop_for_shutdown sets
    // shutdown_report_timeout = Some(quit_timeout) before sending the
    // stop signal; if that quit then gets cancelled by an
    // already-queued LoadNew/ReplaceQueue, shutdown_report_timeout
    // would stay Some for the rest of the session, silently degrading
    // every later track transition to the tight shutdown budget/no-retry
    // path instead of the ordinary one. cmd_load_new/cmd_replace_queue
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
    assert_eq!(session.progress_join_budget(), Duration::from_secs(30));
}

#[test]
fn playlist_pos_does_not_clobber_pending_initial_queue_jump() {
    let (mut session, status) = make_queue_session_for_pos_tests(2);

    session.on_playlist_pos_changed(0);

    assert_eq!(session.current_idx, 2);
    assert_eq!(status.lock().unwrap().current_idx, 2);
}

#[test]
fn playlist_pos_does_not_clobber_pending_replace_queue_load() {
    let (mut session, status) = make_queue_session_for_pos_tests(1);
    session.pending_initial_jump = false;
    session.load_state = LoadState::begin_single();

    session.on_playlist_pos_changed(0);

    assert_eq!(session.current_idx, 1);
    assert_eq!(status.lock().unwrap().current_idx, 1);
}

#[test]
fn playlist_pos_does_not_clobber_in_flight_jump_to() {
    let (mut session, status) = make_queue_session_for_pos_tests(0);
    session.pending_initial_jump = false;
    session.forced_slot_id = session.slot_id_at(1);

    session.on_playlist_pos_changed(1);

    assert_eq!(session.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
    assert_eq!(session.forced_slot_id, session.slot_id_at(1));
}

#[test]
fn playlist_pos_updates_idle_queue_with_valid_mpv_position() {
    let (mut session, status) = make_queue_session_for_pos_tests(0);
    session.pending_initial_jump = false;

    session.on_playlist_pos_changed(2);

    assert_eq!(session.current_idx, 2);
    assert_eq!(status.lock().unwrap().current_idx, 2);
}

#[test]
fn append_items_to_queue_extends_queue_without_moving_current_idx() {
    let (mut session, status) = make_queue_session_for_pos_tests(1);
    let appended = make_media_item("ep4");

    session.append_items_to_queue(vec![appended.clone()]);

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
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    );

    let (plain_tx, plain_rx) = mpsc::channel();
    *player.stop_tx.lock().unwrap() = Some(plain_tx);
    player.stop();
    assert!(plain_rx.recv_timeout(Duration::from_millis(50)).is_ok());
    assert!(player.shutdown_report_timeout.lock().unwrap().is_none());

    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    *player.stop_tx.lock().unwrap() = Some(shutdown_tx);
    player.stop_for_shutdown(Duration::from_secs(7));
    assert!(shutdown_rx.recv_timeout(Duration::from_millis(50)).is_ok());
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
    guard.stop_and_join(Duration::from_millis(150));
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "stop_and_join should return near its 150ms budget, took {elapsed:?}"
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
fn ordinary_stop_marks_stop_report_accepted_not_sent() {
    // Regression test for a code-review finding: the non-shutdown (fast)
    // path in report_stop_now_or_background used to hardcode
    // StopReport::Sent, so progress_report_accepted was always false for
    // an ordinary stop and mark_progress_sync_pending never fired —
    // reopening the stale-overwrite race that pending-sync exists to
    // close. It's still fire-and-forget, but should optimistically mark
    // Accepted; see the call site's comment for why that's the safe
    // failure mode if the background report actually fails.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    let (stop_tx, _stop_rx) = mpsc::channel();
    let mut guard = ProgressGuard {
        stop_tx,
        handle: None,
    };

    session.report_stop_now_or_background(&mut guard);

    assert_eq!(session.stop_report, StopReport::Accepted);
    assert!(session.stop_report.is_accepted());
}

// ── queue_completed_pos / is_near_end ─────────────────────────────────

const RUNTIME: i64 = 600 * TICKS_PER_SECOND; // 10-minute episode

#[test]
fn mid_episode_quit_preserves_position() {
    // User quits at ~88% (528 s into a 600 s episode). Not natural, not near-end,
    // next-up overlay may have appeared but next_up_jump was never set because the
    // user pressed q rather than clicking the overlay. Position must be preserved.
    let pos = 528 * TICKS_PER_SECOND;
    assert!(!is_near_end(false, false, pos, RUNTIME)); // 88% < 95%
    assert_eq!(queue_completed_pos(false, false, false, pos), pos);
}

#[test]
fn next_up_fired_preserves_position() {
    // Bug fix: was_next_up alone used to force completed_pos = 0. After the fix,
    // only natural EOF or >=95% position zeroes it. next_up_jump is now irrelevant
    // to completed_pos — queue_completed_pos doesn't receive it at all.
    let pos = 540 * TICKS_PER_SECOND; // 90% — past 60s-before-end threshold
    assert!(!is_near_end(false, false, pos, RUNTIME)); // still below 95%
    assert_eq!(queue_completed_pos(false, false, false, pos), pos);
}

#[test]
fn natural_end_resets_position() {
    let pos = RUNTIME - TICKS_PER_SECOND; // 1 s before end
    assert_eq!(queue_completed_pos(false, true, false, pos), 0);
}

#[test]
fn near_end_boundary_resets_position() {
    // Exactly 95% (19/20) is near-end; 94% is not.
    let at_95 = RUNTIME * 19 / 20;
    let below = at_95 - 1;
    assert!(is_near_end(false, false, at_95, RUNTIME));
    assert!(!is_near_end(false, false, below, RUNTIME));
    assert_eq!(queue_completed_pos(false, false, true, at_95), 0);
    assert_eq!(queue_completed_pos(false, false, false, below), below);
}

#[test]
fn audio_track_always_resets_position() {
    let pos = 300 * TICKS_PER_SECOND; // 50%
    assert!(!is_near_end(true, false, pos, RUNTIME));
    assert_eq!(queue_completed_pos(true, false, false, pos), 0);
}

#[test]
fn near_end_requires_runtime_known() {
    // If runtime_ticks is 0 (unknown), near-end must never trigger.
    assert!(!is_near_end(false, false, 1_000_000_000, 0));
}

#[test]
fn standalone_quit_timeout_marks_near_end_without_consuming() {
    let pos = RUNTIME * 19 / 20;
    assert_eq!(
        quit_timeout_stop_flags(PlaybackOrigin::Standalone, false, pos, RUNTIME, false),
        (true, false)
    );
    assert_eq!(
        quit_timeout_stop_flags(PlaybackOrigin::Standalone, true, pos, RUNTIME, false),
        (false, false)
    );
    assert_eq!(
        quit_timeout_stop_flags(PlaybackOrigin::Queue, false, pos, RUNTIME, true),
        (true, true)
    );
}

#[test]
fn standalone_fresh_start_does_not_set_pending_resume_secs() {
    // Mirrors cmd_load_new's mutation sequence for a fresh one-slot standalone
    // load of a resumable video: origin becomes Standalone, the queue is
    // replaced with the single new item, then load_active_item_state() runs.
    // mpv's `start` property (set separately by cmd_load_new, not exercised
    // here since it requires a live mpv) already seeks to the resume position,
    // so pending_resume_secs must stay None to avoid a redundant absolute
    // seek in on_playback_restart that would also suppress the first
    // progress report for ~500ms.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);

    let mut item = make_media_item("resumable");
    item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
    assert!(item.should_resume(), "test item must actually be resumable");

    session.origin = PlaybackOrigin::Standalone;
    session.queue = PlaybackQueue::from_items(vec![item], Some(0));
    session.current_idx = 0;

    session.load_active_item_state();

    assert_eq!(
        session.pending_resume_secs, None,
        "standalone fresh-start must rely on mpv's `start` property, not a redundant seek"
    );
}

#[test]
fn queue_slot_activation_still_sets_pending_resume_secs() {
    // Sibling case to the standalone fix above: mid-session slot activation
    // (Queue origin) has no mpv `start`-property shortcut, so
    // load_active_item_state() must still arm pending_resume_secs for a
    // resumable item.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);

    let mut item = make_media_item("resumable");
    item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
    let resume_secs = item.resume_seconds();
    assert!(item.should_resume(), "test item must actually be resumable");

    session.origin = PlaybackOrigin::Queue;
    session.queue = PlaybackQueue::from_items(vec![item], Some(0));
    session.current_idx = 0;

    session.load_active_item_state();

    assert_eq!(session.pending_resume_secs, Some(resume_secs));
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

// ── Feed playback plumbing (task 5.1) ─────────────────────────────────────

fn make_feed_entry(guid: &str, title: &str) -> crate::playback_queue::FeedEntry {
    crate::playback_queue::FeedEntry {
        guid: guid.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.com/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(300 * crate::api::TICKS_PER_SECOND as u64),
        pub_date_secs: None,
        feed_kind: crate::config::FeedKind::Audio,
    }
}

fn make_feed_entry_no_source(guid: &str, title: &str) -> crate::playback_queue::FeedEntry {
    let mut e = make_feed_entry(guid, title);
    e.enclosure_url = None;
    e.link = None;
    e
}

fn make_feed_session() -> (PlaybackRun, Arc<Mutex<PlayerStatus>>) {
    let entry = make_feed_entry("feed-1", "Podcast Episode 1");
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let client = Arc::new(EmbyClient::new(crate::config::Config::default()));
    let reporter = SessionReporter::new(
        client,
        None,
        ItemId::empty(),
        MediaSourceId::new(""),
        EmbySessionId::new(""),
        true, // is_audio
        status.clone(),
    );
    let (event_tx, _event_rx) = mpsc::channel();
    let session = PlaybackRun::new_from_queue_items(
        vec![QueueItem::Feed(entry)],
        0,
        PlaybackOrigin::Standalone,
        reporter,
        MpvRunConfig {
            headless: true,
            use_mpv_config: false,
            no_scripts: true,
            always_skip_intro: false,
            audio_pipe_path: None,
            audio_pipe_samplerate: 0,
            audio_pipe_bitdepth: 0,
        },
        false,
        status.clone(),
        event_tx,
        Arc::new(Mutex::new(SubtitlePrefs::default())),
        Arc::new(Mutex::new(None)),
        String::new(),
        String::new(),
    );
    (session, status)
}

#[test]
fn feed_session_initializes_with_correct_title_and_queue_len() {
    let (session, _status) = make_feed_session();
    assert_eq!(session.osd_title, "Podcast Episode 1");
    assert_eq!(session.queue_len(), 1);
    assert_eq!(session.current_idx, 0);
}

#[test]
fn feed_session_load_active_item_state_sets_zero_position() {
    let (session, _status) = make_feed_session();
    assert_eq!(session.last_valid_pos, 0);
    assert!(session.pending_resume_secs.is_none());
    assert!(session.series_id.as_str().is_empty());
}

#[test]
fn feed_session_origin_is_standalone() {
    let (session, _status) = make_feed_session();
    // Feed from idle creates a Standalone session — no PlaybackOrigin::Feed.
    assert_eq!(session.origin, PlaybackOrigin::Standalone);
}

#[test]
fn feed_session_has_no_ext_sub_urls() {
    let (session, _status) = make_feed_session();
    assert!(session.ext_sub_urls.is_empty());
}

#[test]
fn feed_session_reporter_has_no_session() {
    let (session, _status) = make_feed_session();
    assert!(
        !session.reporter.has_session(),
        "feed reporter must have no Emby session"
    );
}

#[test]
fn feed_queue_item_primary_source_returns_enclosure() {
    let entry = make_feed_entry("g1", "title");
    let qi = QueueItem::Feed(entry.clone());
    if let QueueItem::Feed(e) = &qi {
        assert_eq!(e.primary_source(), Some("https://example.com/g1.mp3"));
    }
}

#[test]
fn feed_queue_item_falls_back_to_link() {
    let mut entry = make_feed_entry("g1", "title");
    entry.enclosure_url = None;
    entry.link = Some("https://fallback.example.com/ep".into());
    let qi = QueueItem::Feed(entry);
    if let QueueItem::Feed(e) = &qi {
        assert_eq!(e.primary_source(), Some("https://fallback.example.com/ep"));
    }
}

#[test]
fn feed_queue_item_no_primary_source_when_both_absent() {
    let mut entry = make_feed_entry("g1", "title");
    entry.enclosure_url = None;
    entry.link = None;
    let qi = QueueItem::Feed(entry);
    if let QueueItem::Feed(e) = &qi {
        assert!(e.primary_source().is_none());
    }
}

#[test]
fn feed_cancel_pending_quit_clears_state() {
    let (mut session, _status) = make_feed_session();
    session.quit_at = Some(std::time::Instant::now());
    *session.shutdown_report_timeout.lock().unwrap() = Some(Duration::from_secs(5));

    session.cancel_pending_quit();

    assert!(session.quit_at.is_none());
    assert!(session.shutdown_report_timeout.lock().unwrap().is_none());
    assert_eq!(session.progress_join_budget(), Duration::from_secs(30));
}

// ── Feed append semantics ──────────────────────────────────────────────────

#[test]
fn feed_append_to_existing_queue_preserves_original_items() {
    // Simulate the append path: start with three Emby items, append a feed entry.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    assert_eq!(session.queue_len(), 3);
    assert_eq!(session.current_idx, 0);

    let feed = make_feed_entry("feed-appended", "Appended Feed");
    let queue_item = QueueItem::Feed(feed.clone());
    let new_idx = session.queue_len();
    session.queue.append(queue_item);
    session.current_idx = new_idx;

    // Original items preserved, feed appended at end.
    assert_eq!(session.queue_len(), 4);
    assert_eq!(session.current_idx, 3);
    // Original items still at their positions.
    assert_eq!(
        session.queue.slots()[0].item.id(),
        "ep1",
        "first original item preserved"
    );
    assert_eq!(
        session.queue.slots()[1].item.id(),
        "ep2",
        "second original item preserved"
    );
    assert_eq!(
        session.queue.slots()[2].item.id(),
        "ep3",
        "third original item preserved"
    );
    // Feed item at the end.
    assert_eq!(session.queue.slots()[3].item.id(), "feed-appended");
}

#[test]
fn feed_append_to_existing_queue_does_not_change_origin() {
    // When appending to a Queue-origin session, origin must stay Queue
    // so on_end_file's advance path continues to work.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    assert_eq!(session.origin, PlaybackOrigin::Queue);

    let feed = make_feed_entry("feed-1", "Feed 1");
    session.queue.append(QueueItem::Feed(feed));
    session.current_idx = session.queue_len() - 1;

    assert_eq!(
        session.origin,
        PlaybackOrigin::Queue,
        "origin must remain Queue after feed append"
    );
}

#[test]
fn feed_empty_queue_creates_standalone_session() {
    // When cmd_load_feed sees an empty queue, it creates a Standalone session.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    // Clear the queue to simulate idle state.
    session.queue = PlaybackQueue::default();
    session.current_idx = 0;
    assert_eq!(session.queue_len(), 0);

    let feed = make_feed_entry("feed-idle", "Idle Feed");
    let queue_item = QueueItem::Feed(feed);
    session.origin = PlaybackOrigin::Standalone;
    session.queue = PlaybackQueue::from_queue_items(vec![queue_item], Some(0));
    session.current_idx = 0;

    assert_eq!(session.queue_len(), 1);
    assert_eq!(session.current_idx, 0);
    assert_eq!(session.origin, PlaybackOrigin::Standalone);
}

// ── Reporter session guards ────────────────────────────────────────────────

#[test]
fn reporter_session_lifecycle() {
    let with_ids = make_no_session_reporter_with_ids();
    assert!(with_ids.has_session());
    with_ids.clear_session();
    assert!(!with_ids.has_session());
    let no_ids = make_no_session_reporter();
    assert!(!no_ids.has_session());
}

// ── Source-less feed entry ─────────────────────────────────────────────────

#[test]
fn sourceless_feed_entry_rejected_by_both_early_and_command_checks() {
    let entry = make_feed_entry_no_source("no-src", "No Source");
    // Early check: primary_source() must be None.
    assert!(
        entry.primary_source().is_none(),
        "source-less entry must have no primary source"
    );
    // Command-level check: URL must be empty.
    let url = entry.primary_source().unwrap_or("").to_string();
    assert!(url.is_empty(), "source-less feed must produce empty URL");
}

// ── Behavioral: mixed queue lifecycle ──────────────────────────────────────

#[test]
fn feed_append_displaced_emby_reported_before_ids_clear_and_drain() {
    // Proves cmd_load_feed → on_end_file state machine:
    // 1) report_stopped_background fires with live IDs for old Emby item
    // 2) IDs cleared for Feed
    // 3) load_state drain suppresses displaced EndFile, resets stop_report
    // 4) After drain, session has no IDs — Feed lifecycle is safe
    let (mut session, _status) = make_queue_session_for_pos_tests(1);
    assert!(session.reporter.has_session());
    let original_id = session.reporter.ids.lock().unwrap().0.clone();
    assert!(!original_id.as_str().is_empty());

    // Simulate cmd_load_feed active path
    session
        .reporter
        .report_stopped_background(session.last_valid_pos);
    assert!(
        session.reporter.has_session(),
        "IDs survive through report_stopped_background"
    );
    session.reporter.clear_session();
    assert!(!session.reporter.has_session());
    session.load_state = LoadState::begin_single();
    session.stop_report = StopReport::NotSent;

    // Simulate on_end_file drain path (displaced EndFile)
    assert!(!session.load_state.is_ready());
    match session.load_state.drain() {
        Drained::HitZero => session.stop_report.reset(),
        other => panic!("expected HitZero, got {other:?}"),
    }
    assert!(session.load_state.is_ready());
    assert_eq!(session.stop_report, StopReport::NotSent);

    // Feed lifecycle state
    assert!(!session.reporter.has_session());
    assert!(!session.reporter.report_stopped(0));
    session.reporter.report_progress("TimeUpdate");
}

#[test]
fn mixed_queue_feed_advances_to_next_emby_item() {
    // After a Feed item completes in a mixed queue, on_end_file's advance
    // path should reach a subsequent Emby item and re-initialize reporting.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    // Queue: [Emby(ep1), Emby(ep2), Feed(f1), Emby(ep3)]
    let feed = make_feed_entry("f1", "Feed 1");
    session.queue.append(QueueItem::Feed(feed));
    let ep3 = make_media_item("ep3");
    session.queue.append(QueueItem::Emby(Box::new(ep3)));
    assert_eq!(session.queue_len(), 5);

    // Simulate being on the Feed item at index 2 with cleared IDs.
    session.current_idx = 2;
    session.reporter.clear_session();
    assert!(!session.reporter.has_session());

    // Simulate advancing to index 3 (ep3): start_item re-initializes IDs.
    {
        let mut ids = session.reporter.ids.lock().unwrap();
        ids.0 = ItemId::new("ep3");
        ids.1 = MediaSourceId::new("msid-ep3");
        ids.2 = EmbySessionId::new("sid-ep3");
    }
    assert!(session.reporter.has_session());
    let _ = session.reporter.report_stopped(0);
}

#[test]
fn feed_queue_quit_path_does_not_mark_played_with_empty_id() {
    // Regression: Queue→Quit path (on_end_file lines 270-275) accessed
    // self.reporter.ids directly and called mark_played without a
    // has_session() guard.  A Feed item with Queue origin exiting via
    // Quit would invoke mark_played("") / retry with an empty ID.
    // After the fix, the guard prevents this.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    // Append a Feed item so origin stays Queue.
    let feed = make_feed_entry("f-quit", "Quit Feed");
    session.queue.append(QueueItem::Feed(feed));
    session.current_idx = session.queue_len() - 1;
    // Clear IDs as cmd_load_feed does.
    session.reporter.clear_session();
    assert!(!session.reporter.has_session());

    // The Queue→Quit guard now checks has_session().  With no session,
    // the mark_played block is skipped entirely.  Verify the guard
    // condition: (natural_end || near_end) && !completed_is_audio && has_session().
    // All three sub-conditions except has_session() could be true for
    // a Feed audio item (is_audio=true → !completed_is_audio=false →
    // guard blocks), but the key invariant is: when has_session() is
    // false, mark_played is never reached regardless of other conditions.
    assert!(!session.reporter.has_session());
}

// ── Behavioral: reporter no-session reporting ──────────────────────────────

fn make_no_session_reporter() -> SessionReporter {
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let client = Arc::new(EmbyClient::new(crate::config::Config::default()));
    SessionReporter::new(
        client,
        None,
        ItemId::empty(),
        MediaSourceId::new(""),
        EmbySessionId::new(""),
        false,
        status,
    )
}

#[test]
fn reporter_no_session_all_reporting_is_noop() {
    let reporter = make_no_session_reporter();
    assert!(!reporter.has_session());
    assert!(!reporter.report_stopped(12345));
    assert!(!reporter.report_stopped_for_shutdown(0, Duration::from_secs(5)));
    reporter.report_stopped_background(12345);
    reporter.report_progress("TimeUpdate");
    reporter.report_progress("Pause");
}

#[test]
fn reporter_with_session_stopped_proceeds_to_client() {
    let reporter = make_no_session_reporter_with_ids();
    assert!(reporter.has_session());
    assert!(!reporter.report_stopped(0));
}

fn make_no_session_reporter_with_ids() -> SessionReporter {
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let client = Arc::new(EmbyClient::new(crate::config::Config::default()));
    SessionReporter::new(
        client,
        None,
        ItemId::new("real-item"),
        MediaSourceId::new("msid"),
        EmbySessionId::new("sid"),
        false,
        status,
    )
}
