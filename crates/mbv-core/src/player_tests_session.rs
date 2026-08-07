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
                .map(|slot| slot.item.id.as_str()),
            Some(appended.id.as_str())
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
