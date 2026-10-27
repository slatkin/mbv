use super::*;

#[test]
fn jump_to_absent_slot_is_discarded_and_rejected() {
    let (run, _status, events) = make_queue_session_for_pos_tests_with_events(0);
    let stale = QueueSlotId::from_raw(u64::MAX);
    assert!(run.queue.slot(stale).is_none());
    reject_stale_jump(&run.event_tx, stale);
    match events.try_recv().expect("stale jump must emit rejection") {
        PlayerEvent::CommandRejected(reason) => assert!(reason.contains("stale slot")),
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn jump_to_after_replacement_resolves_new_occurrence() {
    let (mut run, _status, _events) = make_queue_session_for_pos_tests_with_events(0);
    let replacement_id = QueueSlotId::from_raw(50_000);
    let replacement = QueueItem::Feed(make_feed_entry("replacement", "Replacement"));
    run.queue = ExecutionSequence::from_slot_items(
        vec![(replacement_id, replacement)],
        Some(replacement_id),
    );
    assert_eq!(run.queue.slot_index(replacement_id), Some(0));
    assert_eq!(
        run.queue.slot(replacement_id).unwrap().item.id(),
        "replacement"
    );
}

// Task 2.3: shared-boundary routing and failure surfacing.
// Verifies that bare-local and stay-alive-local playback routes through
// `submit_queue_slots` — the replacement-decision table below covers the
// reuse path's SubmitQueue send and every cold-start reason — that the fast
// path seeds the status for the start item, that the cold path sets
// status.active before spawning, and that selecting an existing Feed slot
// in a mixed queue preserves queue contents.

#[test]
fn submit_queue_slots_preserves_caller_slot_ids() {
    // The app's canonical queue owns slot identity. The SubmitQueue command
    // must carry those ids verbatim (not controller-minted ones), because the
    // run's TrackChanged/JumpTo observations are resolved against the app's
    // queue by identity — a minted id pointing at the wrong item desyncs the
    // queue panel from what mpv is playing.
    // The shared stub recipe below arms `stop_tx`, so the submission takes
    // the reuse path.
    let (player, _stop_rx, cmd_rx) = submit_stub_player(StopState::Armed, Commands::Spy);
    player.current_is_headless.store(false, Ordering::Relaxed);

    let ids = [QueueSlotId::from_raw(7), QueueSlotId::from_raw(3)];
    let items: Vec<_> = ids
        .iter()
        .zip(["feed-a", "feed-b"])
        .map(|(id, guid)| ExecSlot {
            slot_id: *id,
            item: QueueItem::Feed(make_feed_entry(guid, guid)),
        })
        .collect();
    player.submit_queue_slots(items, 1, None, false, 100);

    match cmd_rx
        .expect("the stub installed the command spy")
        .try_recv()
        .expect("expected a command")
    {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert_eq!(start_idx, 1);
            assert_eq!(
                items.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
                ids,
                "caller slot identities must survive the submission boundary"
            );
        }
        _ => panic!("expected SubmitQueue command"),
    }
}

#[test]
fn submit_queue_fast_path_seeds_status_for_the_start_item() {
    // Once the live run accepted the replacement, the status must reflect
    // the new queue items (design D2 seeds after the send is accepted), so
    // any reader that sees the seeded status also sees consistent queue_len
    // and title. The shared stub recipe below arms `stop_tx`, so the
    // submission takes the reuse path and the seed asserted here is the
    // reuse path's own.
    let (player, _stop_rx, cmd_rx) = submit_stub_player(StopState::Armed, Commands::Spy);
    player.current_is_headless.store(false, Ordering::Relaxed);

    let entry = make_feed_entry("feed-test", "Test Episode");
    player.submit_queue_slots(
        vec![ExecSlot {
            slot_id: QueueSlotId::from_raw(1),
            item: QueueItem::Feed(entry),
        }],
        0,
        None,
        false,
        100,
    );

    assert!(matches!(
        cmd_rx
            .expect("the stub installed the command spy")
            .try_recv()
            .expect("expected a command from submit_queue_slots"),
        PlayerCommand::SubmitQueue { .. }
    ));

    let st = player.status.lock().unwrap();
    assert_eq!(st.queue_len, 1);
    assert_eq!(st.current_idx, 0);
    assert_eq!(st.title, "Test Episode");
    assert!(!st.paused);
}

#[test]
fn selecting_existing_feed_slot_preserves_mixed_queue() {
    // Regression: selecting an existing Feed slot in a mixed queue must
    // preserve queue length and order — the correct operation is slot
    // selection (set_active_slot), not queue replacement.
    let mut queue = PlaybackQueue::from_queue_items(
        Vec::new(),
        None,
        std::sync::Arc::new(mbv_queue::QueueRevisionMint::default()),
    );
    let emby_slot = queue.append(QueueItem::Emby(Box::new(EmbyItem {
        id: "emby-1".into(),
        name: "Emby Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
        child_count: None,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 30 * mbv_emby_model::TICKS_PER_SECOND,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        studios: Vec::new(),
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: mbv_emby_model::EmbyImageTags::default(),
    })));
    let feed_slot = queue.append(QueueItem::Feed(make_feed_entry("podcast-ep", "Podcast Ep")));
    let other = queue.append(QueueItem::Feed(make_feed_entry("other-ep", "Other Ep")));
    assert_eq!(queue.len(), 3);

    // Select the existing Feed slot — this is the correct operation for
    // playing an item already in the queue (JumpTo), not queue replacement.
    let result = queue.set_active_slot(feed_slot);
    assert!(
        matches!(result, QueueMutationResult::Applied(())),
        "set_active_slot must succeed for an existing slot"
    );

    // Queue contents are unchanged: same length, same order, same slot IDs.
    assert_eq!(queue.len(), 3, "queue length must be preserved");
    assert_eq!(
        queue.slots().iter().map(|s| s.slot_id).collect::<Vec<_>>(),
        vec![emby_slot, feed_slot, other],
        "slot IDs and order must be preserved"
    );
    assert_eq!(
        queue.active_slot_id(),
        Some(feed_slot),
        "the selected Feed slot must be active"
    );
    assert_eq!(
        queue.active_index(),
        Some(1),
        "active index points to the Feed slot"
    );
}

fn audiobookshelf_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "show-1".into(),
            episode_id: "episode-1".into(),
            title: "Episode 1".into(),
            show_title: Some("Show".into()),
            author: None,
            description: None,
            duration_ticks: Some(100),
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        },
    ))
}

fn audiobookshelf_book_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(
        mbv_queue::AudiobookshelfBookQueueItem {
            library_item_id: "book-1".into(),
            title: "Book 1".into(),
            author: None,
            duration_ticks: Some(100),
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: None,
        },
    ))
}

fn audiobookshelf_context() -> AudiobookshelfPlayerContext {
    AudiobookshelfPlayerContext::new(
        mbv_core::service_runtime::SetupGeneration::new(7),
        mbv_config::AudiobookshelfSetup::new("https://books.example"),
        "secret".into(),
        "device".into(),
    )
    .unwrap()
}

#[test]
fn complete_bare_player_admits_audiobookshelf_without_ctrl_transport() {
    let (player, _stop_rx, commands) = submit_stub_player(StopState::Armed, Commands::Spy);
    player.update_audiobookshelf_context(Some(audiobookshelf_context()));

    assert!(player.can_admit_audiobookshelf());
    assert!(player.submit_queue_slots(
        vec![ExecSlot {
            slot_id: QueueSlotId::from_raw(1),
            item: audiobookshelf_item(),
        }],
        0,
        None,
        false,
        100,
    ));
    assert!(matches!(
        commands
            .expect("the stub installed the command spy")
            .try_recv()
            .unwrap(),
        PlayerCommand::SubmitQueue { items, start_idx }
            if start_idx == 0 && items.len() == 1 && items[0].item.is_audiobookshelf()
    ));
}

#[test]
fn mpv_audio_errors_only_classify_alsa_initialization_failures() {
    assert!(is_clocked_audio_error(&MpvFailure::AudioOutputInit, true));
    assert!(!is_clocked_audio_error(
        &MpvFailure::Other("loading failed".into()),
        true,
    ));
    assert!(!is_clocked_audio_error(&MpvFailure::AudioOutputInit, false));
}

#[test]
fn alsa_initialization_error_stops_run_with_output_error() {
    let (mut run, status, events) = make_queue_session_for_pos_tests_with_events(0);
    run.config.audio_pipe_path = None;
    run.config.audio_device = Some("alsa/hw:Loopback,0,0".into());
    run.reporter.clear_session();
    let mut progress = noop_progress();

    assert!(run.on_mpv_error(&MpvFailure::AudioOutputInit, &mut progress));
    assert!(!status.lock().unwrap().active);
    let PlayerEvent::Stopped { error, .. } = events.recv().unwrap() else {
        panic!("expected stopped event");
    };
    assert_eq!(
        error.as_deref(),
        Some("audio output failed to start (device: alsa/hw:Loopback,0,0)")
    );
}

#[test]
fn process_exit_stops_run_with_the_last_position_and_an_error() {
    let (mut run, status, events) = make_queue_session_for_pos_tests_with_events(0);
    run.reporter.clear_session();
    run.last_valid_pos = 42;
    let mut progress = noop_progress();

    assert!(run.on_mpv_error(&MpvFailure::ProcessExited, &mut progress));
    assert!(!status.lock().unwrap().active);
    let PlayerEvent::Stopped {
        error,
        position_ticks,
        ..
    } = events.recv().unwrap()
    else {
        panic!("expected stopped event");
    };
    assert!(error.is_some());
    assert_eq!(position_ticks, 42);
}

#[test]
fn generic_start_failure_stops_the_run_with_an_error() {
    // Regression (#941): an end-file reason=error before any playback began
    // (missing file, HTTP 404, decode error) decoded to Failed(Other) and was
    // only warned about, so the run stayed active with no playback and the
    // user never got a failed-to-start signal. It must end the run like the
    // process-exited and audio-output paths do.
    let (mut run, status, events) = make_queue_session_for_pos_tests_with_events(0);
    run.reporter.clear_session();
    let mut progress = noop_progress();

    assert!(run.on_mpv_error(&MpvFailure::Other("loading failed".into()), &mut progress));
    assert!(!status.lock().unwrap().active);
    let PlayerEvent::Stopped {
        error,
        position_ticks,
        ..
    } = events.recv().unwrap()
    else {
        panic!("expected stopped event");
    };
    assert_eq!(
        error.as_deref(),
        Some("failed to start media: loading failed")
    );
    assert_eq!(position_ticks, 0);
    // The run ended: no further events follow the one error stop.
    events.try_recv().unwrap_err();
}

#[test]
fn failure_after_playback_began_with_a_next_entry_keeps_the_run_active() {
    // Regression (#941 probe S5): once playback began, mpv skips a failed
    // playlist entry with a next entry on its own and the drift machinery
    // follows it — the run must stay active and send no Stopped from the
    // failure (spec: "A failure after playback began ends the run only when
    // nothing follows", a middle entry fails).
    let (mut run, status, events) = make_queue_session_for_pos_tests_with_events(0);
    run.reporter.clear_session();
    run.run_lifecycle.playback_started();
    let mut progress = noop_progress();

    assert!(!run.on_mpv_error(&MpvFailure::Other("loading failed".into()), &mut progress));

    assert!(status.lock().unwrap().active);
    events.try_recv().unwrap_err();
}

#[test]
fn failure_of_the_last_entry_after_playback_began_reports_the_play_media_error() {
    // Regression (#942, #943): a failed last entry stops the run, and the
    // after-start phasing decides the user text — "failed to play media"
    // must reach the Stopped error, not the start-failure wording.
    let (mut run, _status, events) = make_queue_session_for_pos_tests_with_events(2);
    run.reporter.clear_session();
    run.run_lifecycle.playback_started();
    let mut progress = noop_progress();

    assert!(run.on_mpv_error(&MpvFailure::Other("loading failed".into()), &mut progress));

    let PlayerEvent::Stopped { error, .. } = events.recv().unwrap() else {
        panic!("expected stopped event");
    };
    assert_eq!(
        error.as_deref(),
        Some("failed to play media: loading failed")
    );
}

#[test]
fn context_loss_rejects_audiobookshelf_without_mutating_bound_submission() {
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
    player.update_audiobookshelf_context(Some(audiobookshelf_context()));
    player.status.lock().unwrap().active = true;
    let commands = player.spy_on_commands();
    player.update_audiobookshelf_context(None);

    assert!(!player.can_admit_audiobookshelf());
    assert!(!player.submit_queue_slots(
        vec![ExecSlot {
            slot_id: QueueSlotId::from_raw(1),
            item: audiobookshelf_item(),
        }],
        0,
        None,
        false,
        100,
    ));
    // Book-shaped items hit the same combined-classification refusal.
    assert!(!player.submit_queue_slots(
        vec![ExecSlot {
            slot_id: QueueSlotId::from_raw(2),
            item: audiobookshelf_book_item(),
        }],
        0,
        None,
        false,
        100,
    ));
    commands.try_recv().unwrap_err();
}

// ── queue replacement decision (submit-reuses-live-run) ──────────────────────

/// The playback state the status reports before the submission.
#[derive(Clone, Copy)]
enum Reported {
    Playing,
    Idle,
}

/// Whether `stop_tx` is armed for the running run.
#[derive(Clone, Copy)]
enum StopState {
    Armed,
    Taken,
}

/// Whether the running run's loop has exited (design D7).
#[derive(Clone, Copy)]
enum RunState {
    Live,
    Exited,
}

/// One replacement-decision setup and its expected outcome (design D1).
struct ReplaceDecision {
    /// The reported playback state before the submission. The Controller must
    /// not read it: an idle or parked run reports idle and is still reused.
    reported: Reported,
    /// `current_is_headless` before the submission; the submitted class is
    /// `false`.
    current_headless: bool,
    /// Whether `stop_tx` is armed for the running run.
    stop: StopState,
    /// Whether the running run's loop has exited (design D7).
    run: RunState,
    /// The command channel the stub installed.
    commands: Commands,
    /// The expected decision.
    expect: Expect,
}

/// The command channel the stub installed.
#[derive(Clone, Copy)]
enum Commands {
    /// A live spy receiver.
    Spy,
    /// A spy whose receiver is dropped, so the send must fail.
    Dropped,
    /// No `cmd_tx` at all.
    None,
}

enum Expect {
    /// The live run takes the submission.
    Reuse,
    /// A cold start, with its D5 reason.
    Cold(&'static str),
}

/// The stub recipe every submission test in this file arms: an
/// mpv-inhibited player (a test never starts the real mpv child, issue
/// #757), `stop_tx` armed unless the case simulates a taken stop, and the
/// command channel the submission's send meets — a live spy receiver the
/// test reads, a dropped one whose send must fail, or none at all.
fn submit_stub_player(
    stop: StopState,
    commands: Commands,
) -> (
    Player,
    Option<mpsc::Receiver<()>>,
    Option<mpsc::Receiver<PlayerCommand>>,
) {
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
    )
    .with_mpv_inhibited();
    let mut stop_rx = None;
    if let StopState::Armed = stop {
        let (armed_tx, rx) = mpsc::channel();
        *player.stop_tx.lock().unwrap() = Some(armed_tx);
        stop_rx = Some(rx);
    }
    let cmd_rx = match commands {
        Commands::Spy => Some(player.spy_on_commands()),
        Commands::Dropped => {
            drop(player.spy_on_commands());
            None
        }
        Commands::None => None,
    };
    (player, stop_rx, cmd_rx)
}

/// The replacement-decision setup for one `#[case]`: the shared stub recipe
/// with the case's reported state, headless class, and exit flag on top.
fn player_for_replacement_decision(
    case: &ReplaceDecision,
) -> (
    Player,
    Option<mpsc::Receiver<()>>,
    Option<mpsc::Receiver<PlayerCommand>>,
) {
    let (player, stop_rx, cmd_rx) = submit_stub_player(case.stop, case.commands);
    player.status.lock().unwrap().active = matches!(case.reported, Reported::Playing);
    player
        .current_is_headless
        .store(case.current_headless, Ordering::Relaxed);
    if let RunState::Exited = case.run {
        player
            .run_exited
            .lock()
            .unwrap()
            .store(true, Ordering::Relaxed);
    }
    (player, stop_rx, cmd_rx)
}

// Task 2.1: the Controller-level replacement decision follows the D1 order —
// run exists, headless class, `stop_tx` armed, `run_exited` unset, send — and
// every cold start carries its D5 reason. No case exercises the loop-exit
// window itself: it needs a live mpv child, which hermetic tests must not
// start (issue #757); `run_exited` is set directly instead. There is no
// parked-run case: a parked run (ended, mpv child alive) presents exactly
// the idle row's Player-level state — armed `stop_tx`, unset `run_exited`,
// inactive status — because the decision consults no reported state, so the
// idle row already pins the parked run's reuse.
#[rstest::rstest]
#[case::idle_run_reuses(ReplaceDecision {
    reported: Reported::Idle,
    current_headless: false,
    stop: StopState::Armed,
    run: RunState::Live,
    commands: Commands::Spy,
    expect: Expect::Reuse,
})]
#[case::headless_mismatch_cold_starts(ReplaceDecision {
    reported: Reported::Playing,
    current_headless: true,
    stop: StopState::Armed,
    run: RunState::Live,
    commands: Commands::Spy,
    expect: Expect::Cold("headless_mismatch"),
})]
#[case::stop_requested_cold_starts(ReplaceDecision {
    reported: Reported::Playing,
    current_headless: false,
    stop: StopState::Taken,
    run: RunState::Live,
    commands: Commands::Spy,
    expect: Expect::Cold("stop_requested"),
})]
#[case::run_exited_cold_starts(ReplaceDecision {
    reported: Reported::Playing,
    current_headless: false,
    stop: StopState::Armed,
    run: RunState::Exited,
    commands: Commands::Spy,
    expect: Expect::Cold("run_exited"),
})]
#[case::send_failed_cold_starts(ReplaceDecision {
    reported: Reported::Playing,
    current_headless: false,
    stop: StopState::Armed,
    run: RunState::Live,
    commands: Commands::Dropped,
    expect: Expect::Cold("send_failed"),
})]
#[case::no_run_cold_starts(ReplaceDecision {
    reported: Reported::Idle,
    current_headless: false,
    stop: StopState::Taken,
    run: RunState::Live,
    commands: Commands::None,
    expect: Expect::Cold("no_run"),
})]
fn queue_replacement_decision_follows_the_d1_order(#[case] case: ReplaceDecision) {
    let (player, stop_rx, cmd_rx) = player_for_replacement_decision(&case);

    let lines = mbv_core::applog::test_support::capture_log_lines(|| {
        let accepted = player.submit_queue_slots(
            vec![ExecSlot {
                slot_id: QueueSlotId::from_raw(1),
                item: QueueItem::Feed(make_feed_entry("replacement", "Replacement")),
            }],
            0,
            None,
            false,
            100,
        );
        assert!(accepted, "every decision path accepts the submission");
    });

    let decisions: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("event=player.submit."))
        .collect();
    assert_eq!(
        decisions.len(),
        1,
        "exactly one decision event per submission: {lines:?}"
    );
    match case.expect {
        Expect::Reuse => {
            assert!(
                decisions[0].contains("event=player.submit.reused_run"),
                "reuse event: {}",
                decisions[0]
            );
            assert!(
                decisions[0].contains("run_identity="),
                "reuse event names the run identity: {}",
                decisions[0]
            );
            let cmd = cmd_rx
                .expect("a reuse decision sends into the live run")
                .try_recv()
                .expect("the live run received the replacement");
            match cmd {
                PlayerCommand::SubmitQueue { items, start_idx } => {
                    assert_eq!(start_idx, 0);
                    assert_eq!(items.len(), 1);
                    assert!(matches!(items[0].item, QueueItem::Feed(_)));
                    // The submitted payload reaches the run verbatim (the
                    // guid the submission built is the guid the run got).
                    assert!(
                        matches!(&items[0].item, QueueItem::Feed(e) if e.guid == "replacement")
                    );
                }
                _ => panic!("expected SubmitQueue"),
            }
            if let Some(stop_rx) = stop_rx {
                assert!(stop_rx.try_recv().is_err(), "a reused run is never stopped");
            }
        }
        Expect::Cold(reason) => {
            assert!(
                decisions[0].contains("event=player.submit.cold_started")
                    && decisions[0].contains(&format!("reason={reason}")),
                "cold-start event with its reason: {}",
                decisions[0]
            );
            if let Some(cmd_rx) = cmd_rx {
                assert!(
                    cmd_rx.try_recv().is_err(),
                    "a cold start sends no command into the old run"
                );
            }
            if let Some(stop_rx) = stop_rx {
                assert!(stop_rx.try_recv().is_ok(), "a cold start stops the old run");
            }
            assert!(
                player.status.lock().unwrap().active,
                "a cold start seeds the new run as active"
            );
        }
    }
}
