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
        description: None,
        duration_ticks: Some(100_u64),
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}

fn abs_book_item() -> QueueItem {
    QueueItem::AudiobookshelfBook(crate::playback_queue::AudiobookshelfBookQueueItem {
        library_item_id: "book".into(),
        title: "Book".into(),
        author: None,
        duration_ticks: Some(100_u64),
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    })
}

#[test]
fn active_file_preparation_finalizes_before_opening_next_session() {
    let (first_base, first_requests) = super::source_tests::serve_close(1.0);
    let (second_base, second_requests) = super::source_tests::serve_close(1.0);
    let first_context = AudiobookshelfPlayerContext::new(
        crate::service_runtime::SetupGeneration::new(12),
        crate::config::AudiobookshelfSetup::new(first_base),
        "secret".into(),
        "device".into(),
    )
    .unwrap();
    let first = prepare_source(&abs_item(), "", "", Some(&first_context)).unwrap();
    let (mut run, _) = make_queue_session_for_pos_tests(0);
    run.prepared_source = Some(first);
    run.audiobookshelf_context = Some(
        AudiobookshelfPlayerContext::new(
            crate::service_runtime::SetupGeneration::new(13),
            crate::config::AudiobookshelfSetup::new(second_base),
            "secret".into(),
            "device".into(),
        )
        .unwrap(),
    );

    let second = run.prepare_item(&abs_item()).unwrap();
    let first_requests: Vec<_> = (0..3).map(|_| first_requests.recv().unwrap()).collect();
    assert!(first_requests[0].starts_with("POST /api/items/show/play/episode HTTP/1.1"));
    assert!(first_requests[1].starts_with("POST /api/session/%3CSESSION_ID%3E/sync HTTP/1.1"));
    assert!(first_requests[2].starts_with("POST /api/session/%3CSESSION_ID%3E/close HTTP/1.1"));
    assert!(second_requests
        .recv()
        .unwrap()
        .starts_with("POST /api/items/show/play/episode HTTP/1.1"));
    drop(second);
    assert!(second_requests
        .recv()
        .unwrap()
        .starts_with("POST /api/session/%3CSESSION_ID%3E/sync HTTP/1.1"));
    assert!(second_requests
        .recv()
        .unwrap()
        .starts_with("POST /api/session/%3CSESSION_ID%3E/close HTTP/1.1"));
}
