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
    assert_eq!(session.progress_join_budget(), Duration::from_secs(30));
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
    session.load_state = LoadState::begin_single();

    session.on_playlist_pos_changed(0, 0);

    assert_eq!(session.current_idx, 1);
    assert_eq!(status.lock().unwrap().current_idx, 1);
}

#[test]
fn playlist_pos_does_not_clobber_in_flight_jump_to() {
    let (mut session, status) = make_queue_session_for_pos_tests(0);
    session.pending_initial_playlist_layout = false;
    let target = session.slot_id_at(1).unwrap();
    session.forced_slot_id = Some(target);
    // Rapid Enter on two rows: the in-flight jump also carries its request
    // identity; an intermediate playlist-pos event must not clobber either.
    session.forced_transition =
        Some(crate::playback_transition::Transition::new(42, 1, target));

    session.on_playlist_pos_changed(1, 0);

    assert_eq!(session.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
    assert_eq!(session.forced_slot_id, Some(target));
    assert_eq!(
        session.forced_transition.map(|t| (t.request_id, t.target)),
        Some((42, target)),
        "the in-flight jump's request identity survives an intermediate playlist-pos event"
    );
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
        PlayerEvent::TrackChanged { slot_id, transition } => Some((slot_id, transition)),
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

    session.append_items_to_queue(owner_paired(vec![QueueItem::Emby(Box::new(appended.clone()))]));

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
    session.stop_report = StopReport::Sent; // skip the reporter side effects
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
fn superseded_jump_end_file_is_dropped_only_without_a_forced_slot() {
    // Rapid Enter on two queue rows: the second JumpTo's slot is the live
    // `forced_slot_id`; the first target's stray `Stop` EndFile arrives with
    // none. Only that stray one must be dropped — a `Stop` while a forced
    // jump is still pending is the real target landing and must advance.
    assert!(is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        false,
        false
    ));
    assert!(is_superseded_jump_end_file(
        mpv_end_file_reason::Redirect,
        false,
        false
    ));
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        true,
        false
    ));
    // A finished track (EOF/near-end/next-up) always advances.
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Stop,
        false,
        true
    ));
    // A playback error still skips the broken track forward.
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Error,
        false,
        false
    ));
    assert!(!is_superseded_jump_end_file(
        mpv_end_file_reason::Eof,
        false,
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

#[test]
fn abs_natural_eof_uses_runtime_without_changing_generic_audio_reporting() {
    let abs = abs_item();
    let abs_book = abs_book_item();
    let emby_audio = QueueItem::Emby(Box::new(make_media_item("audio")));
    let runtime = 90_000;
    let actual = 12_000;

    assert_eq!(
        provider_lifecycle_close_pos(&abs, true, runtime, actual),
        runtime
    );
    assert_eq!(
        provider_lifecycle_close_pos(&abs, false, runtime, actual),
        actual
    );
    // Task 2.3: books share the combined classification — a naturally
    // completed book closes at runtime; a non-natural stop keeps the
    // last valid position.
    assert_eq!(
        provider_lifecycle_close_pos(&abs_book, true, runtime, actual),
        runtime
    );
    assert_eq!(
        provider_lifecycle_close_pos(&abs_book, false, runtime, actual),
        actual
    );
    assert_eq!(
        provider_lifecycle_close_pos(&emby_audio, true, runtime, actual),
        actual
    );
    assert_eq!(queue_completed_pos(true, true, false, actual), 0);
}

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
fn near_end_verdict_is_identical_across_exit_paths() {
    // "Same completion, different exit path": the advance, quit and shutdown
    // paths in player_run_events all reach the verdict through `is_near_end`
    // with the completed occurrence's runtime, so a single completion at a
    // single position cannot be judged near-end on one path and not another.
    let pos = 96 * RUNTIME / 100;
    let advance = is_near_end(false, false, pos, RUNTIME);
    let quit = is_near_end(false, false, pos, RUNTIME);
    let shutdown = is_near_end(false, false, pos, RUNTIME);
    assert!(advance, "96% of runtime is past the 95% near-end threshold");
    assert_eq!(advance, quit);
    assert_eq!(quit, shutdown);
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
fn standalone_fresh_start_preserves_saved_position() {
    // Mirrors cmd_load_new's mutation sequence for a fresh one-slot standalone
    // load of a resumable video: origin becomes Standalone, the queue is
    // replaced with the single new item, then load_active_item_state() runs.
    // mpv's load position is configured separately by cmd_load_new; this state
    // still seeds progress reporting at the saved position before events arrive.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);

    let mut item = make_media_item("resumable");
    item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
    assert!(item.should_resume(), "test item must actually be resumable");

    session.origin = PlaybackOrigin::Standalone;
    let position_ticks = item.playback_position_ticks;
    session.queue = ExecutionSequence::from_slot_items(
        vec![(QueueSlotId::from_raw(1), QueueItem::Emby(Box::new(item)))],
        Some(QueueSlotId::from_raw(1)),
    );
    session.current_idx = 0;

    session.load_active_item_state();

    assert_eq!(session.last_valid_pos, position_ticks);
}

#[test]
fn queue_slot_activation_preserves_saved_position() {
    let (mut session, _status) = make_queue_session_for_pos_tests(0);

    let mut item = make_media_item("resumable");
    item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
    assert!(item.should_resume(), "test item must actually be resumable");
    let position_ticks = item.playback_position_ticks;

    session.origin = PlaybackOrigin::Queue;
    session.queue = ExecutionSequence::from_slot_items(
        vec![(QueueSlotId::from_raw(1), QueueItem::Emby(Box::new(item)))],
        Some(QueueSlotId::from_raw(1)),
    );
    session.current_idx = 0;

    session.load_active_item_state();

    assert_eq!(session.last_valid_pos, position_ticks);
}

#[test]
fn resume_start_pos_uses_saved_position_for_resumable_video() {
    // Regression test: cmd_submit_queue's warm-reuse path used to hardcode
    // mpv's `start` property to "0", silently dropping the resume position
    // that cmd_load_new used to set. resume_start_pos() is the extracted
    // decision it now uses instead.
    let mut item = make_media_item("resumable");
    item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
    assert!(item.should_resume(), "test item must actually be resumable");
    let resume_secs = item.resume_seconds();

    let queue_item = QueueItem::Emby(Box::new(item));

    assert_eq!(resume_start_pos(&queue_item), resume_secs);
}

#[test]
fn resume_start_pos_is_zero_for_audio_non_resumable_and_zero_position_feed_items() {
    let mut audio_item = make_media_item("audio");
    audio_item.media_type = "Audio".into();
    audio_item.playback_position_ticks = audio_item.runtime_ticks / 2;
    assert_eq!(
        resume_start_pos(&QueueItem::Emby(Box::new(audio_item))),
        0.0
    );

    let fresh_item = make_media_item("fresh");
    assert!(!fresh_item.should_resume());
    assert_eq!(
        resume_start_pos(&QueueItem::Emby(Box::new(fresh_item))),
        0.0
    );

    let feed_entry = make_feed_entry("feed-1", "Podcast Episode 1");
    // Feed entry with zero position starts from the beginning.
    assert_eq!(resume_start_pos(&QueueItem::Feed(feed_entry)), 0.0);
}

#[test]
fn queue_loads_selected_item_first_without_starting_playback() {
    let mut item = make_media_item("resumable");
    item.playback_position_ticks = item.runtime_ticks / 2;
    let queue_item = QueueItem::Emby(Box::new(item));

    assert!(mpv_load_opts(&queue_item).contains(",start="));
    assert_eq!(
        queue_load_indices(4, 2).collect::<Vec<_>>(),
        vec![2, 0, 1, 3]
    );
    // Design D3: the start slot and later slots load as no-play appends;
    // only earlier slots insert, so no load in the plan starts playback.
    assert_eq!(queue_load_location(2, 2).0, "append");
    assert_eq!(queue_load_location(0, 2), ("insert-at", "0".into()));
    assert_eq!(queue_load_location(1, 2), ("insert-at", "1".into()));
    assert_eq!(queue_load_location(3, 2).0, "append");
}

#[test]
fn no_play_load_plan_builds_canonical_playlist_before_playback_starts() {
    // D3 mock model of mpv's loadfile semantics over an empty playlist:
    // `append` pushes to the end without starting playback, `insert-at i`
    // inserts at ordinal i without starting playback (verified against the
    // mpv IPC contract — `append` maps to LOAD_TYPE_APPEND, play=false).
    // The only playback-start step is the final `start_queue_playback`
    // playlist-pos write, so after the loads the layout must already be
    // exactly what the reassert safety net treats as Ok.
    for (len, start_idx) in [(1, 0), (4, 0), (4, 2), (5, 4), (100, 50)] {
        let mut playlist: Vec<usize> = Vec::new();
        for i in queue_load_indices(len, start_idx) {
            let (mode, index) = queue_load_location(i, start_idx);
            match mode {
                // Neither no-play mode may start playback mid-load.
                "append" => playlist.push(i),
                // pi-lens-ignore: rust-unwrap
                "insert-at" => playlist.insert(index.parse::<usize>().unwrap(), i),
                other => panic!("unexpected load mode {other}"),
            }
        }
        assert_eq!(playlist, (0..len).collect::<Vec<_>>());
        // The start slot plays first, from the very first audible moment.
        assert_eq!(playlist[start_idx], start_idx);
        // Reassert safety net: full layout, mpv on the start slot -> Ok.
        assert_eq!(
            queue_layout_verdict(start_idx, len, start_idx as i64, len as i64),
            QueueLayoutVerdict::Ok
        );
    }
}

#[test]
fn cold_active_file_single_load_starts_playback_via_replace() {
    // Mock model of the cold submit path's active-file (Audiobookshelf)
    // branch: the load plan is exactly vec![start_idx] and
    // `start_queue_playback` is skipped, so the single loadfile must itself
    // start playback. mpv semantics: `replace` on an empty idle playlist
    // plays the file; the D3 no-play queue plan modes never do — which is
    // why this branch must not use `queue_load_location`.
    let start_idx = 0;
    let mut playlist: Vec<usize> = Vec::new();
    let mut playback_started = false;
    // Cold submit: load plan is exactly vec![start_idx] for this projection.
    let (mode, index) = active_file_load_location();
    match mode {
        "replace" => {
            playlist = vec![start_idx];
            playback_started = true;
        }
        "append" => playlist.push(start_idx),
        // pi-lens-ignore: rust-unwrap
        "insert-at" => playlist.insert(index.parse::<usize>().unwrap(), start_idx),
        other => panic!("unexpected load mode {other}"),
    }
    assert_eq!(playlist, vec![start_idx]);
    assert!(
        playback_started,
        "cold active-file load must start playback; no-play plan modes idle the run"
    );
    // Contrast: the same submit loop's full-queue arm is no-play for the
    // start slot (D3), so the carve-out is what keeps the active-file load
    // distinct.
    assert_eq!(queue_load_location(start_idx, start_idx).0, "append");
}

#[test]
fn divergent_entry_names_only_an_entry_mpv_actually_moved_to() {
    // mpv is where the run already believes playback is: nothing to adopt.
    assert_eq!(divergent_entry(2, 2, 4), None);
    // mpv is on another entry: that entry is what is playing now.
    assert_eq!(divergent_entry(0, 2, 4), Some(0));
    assert_eq!(divergent_entry(3, 2, 4), Some(3));
    // Nothing playing, or an ordinal this queue no longer holds.
    assert_eq!(divergent_entry(-1, 2, 4), None);
    assert_eq!(divergent_entry(4, 2, 4), None);
}

#[test]
fn queue_layout_verdict_reasserts_only_a_complete_playlist() {
    // Every item landed and the active one is playing: nothing to repair.
    assert_eq!(queue_layout_verdict(2, 4, 2, 4), QueueLayoutVerdict::Ok);
    // mpv finished the layout on another entry — the ordinal the whole run
    // (current_idx, status, every reported item) is derived from is wrong and
    // has to be reasserted from the layout we built.
    assert_eq!(queue_layout_verdict(2, 4, 0, 4), QueueLayoutVerdict::Reassert);
    assert_eq!(queue_layout_verdict(2, 4, -1, 4), QueueLayoutVerdict::Reassert);
    // A short playlist means an ordinal no longer names its item: report it
    // instead of seeking to a position that means something else.
    assert_eq!(
        queue_layout_verdict(2, 4, 2, 3),
        QueueLayoutVerdict::ShortLayout
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

    assert!(!status.lock().unwrap().active, "EOF must deactivate the status snapshot");
    let PlayerEvent::Stopped { played, .. } = events.try_recv().unwrap() else {
        panic!("expected Stopped event");
    };
    assert!(played, "natural video EOF must surface as played");
}
