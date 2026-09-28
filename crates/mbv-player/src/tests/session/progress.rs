use super::*;

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
    // paths in crates/mbv-player/src/run/events all reach the verdict through `is_near_end`
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

    assert!((resume_start_pos(&queue_item) - resume_secs).abs() < f64::EPSILON);
}

#[test]
fn resume_start_pos_is_zero_for_audio_non_resumable_and_zero_position_feed_items() {
    let mut audio_item = make_media_item("audio");
    audio_item.media_type = "Audio".into();
    audio_item.playback_position_ticks = audio_item.runtime_ticks / 2;
    assert!(resume_start_pos(&QueueItem::Emby(Box::new(audio_item))).abs() < f64::EPSILON);

    let fresh_item = make_media_item("fresh");
    assert!(!fresh_item.should_resume());
    assert!(resume_start_pos(&QueueItem::Emby(Box::new(fresh_item))).abs() < f64::EPSILON);

    let feed_entry = make_feed_entry("feed-1", "Podcast Episode 1");
    // Feed entry with zero position starts from the beginning.
    assert!(resume_start_pos(&QueueItem::Feed(feed_entry)).abs() < f64::EPSILON);
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
fn queue_load_plan_never_starts_playback_mid_load() {
    // Design D3: the only playback start in the queue plan is the
    // `start_queue_playback` write after every load, so no load may use a mode
    // that starts playback itself (`replace`). mpv's no-play behaviour for the
    // modes used here (`append`, `insert-at`) is mpv's documented contract, not
    // something a mock can prove — what this pins is that the production plan
    // never reaches for a louder mode, at any length or start index.
    for (len, start_idx) in [(1, 0), (4, 0), (4, 2), (5, 4), (100, 50)] {
        assert_eq!(queue_load_indices(len, start_idx).next(), Some(start_idx));
        for i in queue_load_indices(len, start_idx) {
            let (mode, _) = queue_load_location(i, start_idx);
            assert_ne!(
                mode, "replace",
                "load {i} of a queue starting at {start_idx} must not start playback (D3)"
            );
        }
        // With the plan built, the reassert safety net must observe Ok: a
        // mismatch there means the no-play load plan drifted.
        assert_eq!(
            queue_layout_verdict(
                start_idx,
                len,
                i64::try_from(start_idx).unwrap(),
                i64::try_from(len).unwrap(),
                false,
            ),
            QueueLayoutVerdict::Ok
        );
    }
}

#[test]
fn active_file_jump_loads_non_audiobookshelf_resume_ticks() {
    let (mut run, _) = make_queue_session_for_pos_tests(0);
    let target_id = QueueSlotId::from_raw(2);
    let target = QueueItem::Feed(make_feed_entry("feed", "episode"));
    run.queue = ExecutionSequence::from_slot_items(
        vec![
            (QueueSlotId::from_raw(1), abs_item()),
            (target_id, target.clone()),
        ],
        Some(QueueSlotId::from_raw(1)),
    );
    assert!(run.queue.has_audiobookshelf_entries());

    let resume_ticks = 42 * TICKS_PER_SECOND;
    let (_, prepared) = run
        .prepare_active_slot_with_resume(target_id, Some(resume_ticks))
        .unwrap();

    assert!(prepared.mpv_load_options(&target).contains(",start=42"));
}

#[test]
fn cold_active_file_single_load_starts_playback_via_replace() {
    // The cold submit path's active-file (Audiobookshelf) branch loads exactly
    // one slot and skips `start_queue_playback`, so its load must start
    // playback itself; reusing the D3 no-play plan modes there would idle the
    // run. The two projections are what keep the load modes distinct.
    assert_eq!(active_file_load_location().0, "replace");
    assert_ne!(queue_load_location(0, 0).0, "replace");
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
    assert_eq!(
        queue_layout_verdict(2, 4, 2, 4, false),
        QueueLayoutVerdict::Ok
    );
    // Correct ordinal is not enough: an idle player means the first item
    // never actually started, so the complete layout must be reasserted.
    assert_eq!(
        queue_layout_verdict(2, 4, 2, 4, true),
        QueueLayoutVerdict::Reassert
    );
    // mpv finished the layout on another entry — the ordinal the whole run
    // (current_idx, status, every reported item) is derived from is wrong and
    // has to be reasserted from the layout we built.
    assert_eq!(
        queue_layout_verdict(2, 4, 0, 4, false),
        QueueLayoutVerdict::Reassert
    );
    assert_eq!(
        queue_layout_verdict(2, 4, -1, 4, false),
        QueueLayoutVerdict::Reassert
    );
    // A short playlist means an ordinal no longer names its item: report it
    // instead of seeking to a position that means something else.
    assert_eq!(
        queue_layout_verdict(2, 4, 2, 3, false),
        QueueLayoutVerdict::ShortLayout
    );
}
