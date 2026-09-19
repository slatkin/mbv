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
    assert_eq!(run.queue.slot(replacement_id).unwrap().item.id(), "replacement");
}

// Task 2.3: shared-boundary routing and failure surfacing.
// Verifies that bare-local and stay-alive-local playback routes through
// `submit_queue_slots`, that the fast path sends a SubmitQueue command, that
// the cold path sets status.active before spawning, and that selecting
// an existing Feed slot in a mixed queue preserves queue contents.

#[test]
fn submit_queue_fast_path_sends_command_for_feed_entry() {
    // When the player is already active with matching headless state,
    // submit_queue_slots must route through the SubmitQueue command (fast path)
    // rather than spawning a new thread. This proves bare-local and
    // stay-alive-local playback share the same boundary.
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
    player.status.lock().unwrap().active = true;
    player.current_is_headless.store(false, Ordering::Relaxed);
    let cmd_rx = player.spy_on_commands();

    let entry = make_feed_entry("feed-1", "Podcast Episode 1");
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

    let cmd = cmd_rx
        .try_recv()
        .expect("expected a command from submit_queue_slots");
    match cmd {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert_eq!(items.len(), 1);
            assert_eq!(start_idx, 0);
            assert!(matches!(&items[0].item, QueueItem::Feed(e) if e.guid == "feed-1"));
        }
        _ => panic!("expected SubmitQueue command"),
    }
}

#[test]
fn queue_append_forwards_caller_slot_ids() {
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
    let cmd_rx = player.spy_on_commands();
    let slots = vec![
        ExecSlot {
            slot_id: QueueSlotId::from_raw(17),
            item: QueueItem::Feed(make_feed_entry("append-a", "Append A")),
        },
        ExecSlot {
            slot_id: QueueSlotId::from_raw(4),
            item: QueueItem::Feed(make_feed_entry("append-b", "Append B")),
        },
    ];
    assert!(player.queue_append(slots));

    match cmd_rx.try_recv().expect("expected QueueAppend command") {
        PlayerCommand::QueueAppend { items } => {
            assert_eq!(items.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
                       vec![QueueSlotId::from_raw(17), QueueSlotId::from_raw(4)]);
        }
        _ => panic!("expected QueueAppend command"),
    }
}

#[test]
fn submit_queue_slots_preserves_caller_slot_ids() {
    // The app's canonical queue owns slot identity. The SubmitQueue command
    // must carry those ids verbatim (not controller-minted ones), because the
    // run's TrackChanged/JumpTo observations are resolved against the app's
    // queue by identity — a minted id pointing at the wrong item desyncs the
    // queue panel from what mpv is playing.
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
    player.status.lock().unwrap().active = true;
    player.current_is_headless.store(false, Ordering::Relaxed);
    let cmd_rx = player.spy_on_commands();

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

    match cmd_rx.try_recv().expect("expected a command") {
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
fn submit_queue_fast_path_updates_status_before_sending() {
    // The status must reflect the new queue items before the command is
    // sent, so any reader that sees status.active = true also sees
    // consistent queue_len and title.
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
    player.status.lock().unwrap().active = true;
    player.current_is_headless.store(false, Ordering::Relaxed);
    let _cmd_rx = player.spy_on_commands();

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
    let mut queue = PlaybackQueue::default();
    let emby_slot = queue.append(QueueItem::Emby(Box::new(EmbyItem {
        id: "emby-1".into(),
        name: "Emby Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
        child_count: None,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 30 * crate::api::TICKS_PER_SECOND,
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
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
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
        image_tags: Default::default(),
    })));
    let feed_slot = queue.append(QueueItem::Feed(make_feed_entry("podcast-ep", "Podcast Ep")));
    let _other = queue.append(QueueItem::Feed(make_feed_entry("other-ep", "Other Ep")));
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
        vec![emby_slot, feed_slot, _other],
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
    QueueItem::Audiobookshelf(crate::playback_queue::AudiobookshelfQueueItem {
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
    })
}

fn audiobookshelf_book_item() -> QueueItem {
    QueueItem::AudiobookshelfBook(crate::playback_queue::AudiobookshelfBookQueueItem {
        library_item_id: "book-1".into(),
        title: "Book 1".into(),
        author: None,
        duration_ticks: Some(100),
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    })
}

fn audiobookshelf_context() -> AudiobookshelfPlayerContext {
    AudiobookshelfPlayerContext::new(
        crate::service_runtime::SetupGeneration::new(7),
        crate::config::AudiobookshelfSetup::new("https://books.example"),
        "secret".into(),
        "device".into(),
    )
    .unwrap()
}

#[test]
fn complete_bare_player_admits_audiobookshelf_without_ctrl_transport() {
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
        commands.try_recv().unwrap(),
        PlayerCommand::SubmitQueue { items, start_idx }
            if start_idx == 0 && items.len() == 1 && items[0].item.is_audiobookshelf()
    ));
}

#[test]
fn mpv_audio_errors_only_classify_alsa_initialization_failures() {
    assert!(is_clocked_audio_error(
        &libmpv2::Error::Raw(libmpv2::mpv_error::AoInitFailed),
        true,
    ));
    assert!(!is_clocked_audio_error(
        &libmpv2::Error::Raw(libmpv2::mpv_error::LoadingFailed),
        true,
    ));
    assert!(!is_clocked_audio_error(
        &libmpv2::Error::Raw(libmpv2::mpv_error::AoInitFailed),
        false,
    ));
}

#[test]
fn alsa_initialization_error_stops_run_with_output_error() {
    let (mut run, status, events) = make_queue_session_for_pos_tests_with_events(0);
    run.config.audio_pipe_path = None;
    run.config.audio_device = Some("alsa/hw:Loopback,0,0".into());
    run.reporter.clear_session();
    let mut progress = noop_progress();

    assert!(run.on_mpv_error(
        libmpv2::Error::Raw(libmpv2::mpv_error::AoInitFailed),
        &mut progress,
    ));
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
    assert!(commands.try_recv().is_err());
}
