fn test_mpv() -> Mpv {
    init_mpv(&MpvRunConfig {
        headless: true,
        use_mpv_config: false,
        no_scripts: true,
        always_skip_intro: false,
        audio_pipe_path: None,
        audio_pipe_samplerate: 0,
        audio_pipe_bitdepth: 0,
    })
    .unwrap()
    .0
}

fn noop_progress() -> ProgressGuard {
    let (stop_tx, _) = mpsc::channel();
    ProgressGuard {
        stop_tx,
        handle: None,
    }
}

fn abs_item() -> QueueItem {
    QueueItem::Audiobookshelf(crate::playback_queue::AudiobookshelfQueueItem {
        library_item_id: "show".into(),
        episode_id: "episode".into(),
        title: "Episode".into(),
        show_title: None,
        author: None,
        duration_ticks: Some(TICKS_PER_SECOND as u64),
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}

#[test]
fn failed_eager_transition_preserves_canonical_queue_and_mode() {
    let (mut run, _) = make_queue_session_for_pos_tests(1);
    let active_abs = run.queue.append(abs_item());
    let _ = run.queue.set_active_slot(active_abs);
    run.refresh_current_idx_from_queue();
    let old_slots: Vec<_> = run.queue.slots().iter().map(|slot| slot.slot_id).collect();
    let old_active = run.active_slot_id();
    let mpv = test_mpv();

    run.cmd_append_queue(vec![abs_item()], &mpv);

    assert_eq!(
        run.queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        old_slots
    );
    assert_eq!(run.active_slot_id(), old_active);
    assert!(!run.projection.is_active_file());
}

#[test]
fn active_file_replacement_uses_canonical_item_generic_path_and_one_mpv_entry() {
    let (mut run, status) = make_queue_session_for_pos_tests(0);
    run.projection.activate();
    let mpv = test_mpv();
    let mut progress = noop_progress();
    let items = vec![
        QueueItem::Emby(Box::new(make_media_item("replacement-a"))),
        QueueItem::Emby(Box::new(make_media_item("replacement-b"))),
    ];

    run.replace_with_queue_items(items, 1, &mpv, &mut progress);

    assert!(run.projection.is_active_file());
    assert_eq!(run.queue_len(), 2);
    assert_eq!(run.current_idx, 1);
    assert_eq!(mpv.get_property::<i64>("playlist-count").unwrap(), 1);
    assert!(status.lock().unwrap().active);
}

#[test]
fn asynchronous_active_file_start_error_stops_and_preserves_canonical_queue() {
    let (mut run, status) = make_queue_session_for_pos_tests(0);
    run.projection.activate();
    run.active_file_starting = true;
    let slots: Vec<_> = run.queue.slots().iter().map(|slot| slot.slot_id).collect();
    let active_slot = run.active_slot_id();
    let mpv = test_mpv();
    let mut progress = noop_progress();

    assert!(!run.on_end_file(mpv_end_file_reason::Error, &mpv, &mut progress));

    assert!(!status.lock().unwrap().active);
    assert_eq!(
        run.queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        slots
    );
    assert_eq!(run.active_slot_id(), active_slot);
    assert!(run.prepared_source.is_none());
}
