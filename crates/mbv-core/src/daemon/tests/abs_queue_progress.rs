fn book_qi(library_item_id: &str) -> QueueItem {
    QueueItem::AudiobookshelfBook(AudiobookshelfBookQueueItem {
        library_item_id: library_item_id.into(),
        title: "Test Book".into(),
        author: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    })
}

// A book progress update must never match an episode-shaped queue slot, even
// when the `library_item_id` collides — the two kinds share no identity.
#[test]
fn book_progress_update_does_not_touch_episode_slots() {
    let mut queue = PlaybackQueue::from_queue_items(vec![abs_qi("li_1", "ep")], Some(0));
    let before = queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap()
        .position_ticks;

    apply_audiobookshelf_book_progress(
        AudiobookshelfBookProgressUpdate {
            generation: SetupGeneration::new(1),
            library_item_id: "li_1".into(),
            current_time_seconds: 30.0,
            duration_seconds: 100.0,
            is_finished: false,
        },
        Some(SetupGeneration::new(1)),
        &mut queue,
        &Arc::new(Mutex::new(CtrlClients::default())),
    );

    let episode = queue.slots()[0].item.as_audiobookshelf().unwrap();
    assert_eq!(
        episode.position_ticks, before,
        "a book progress event must not update an episode-shaped slot"
    );
    assert!(!episode.is_finished);
}

// An episode progress update must not match a book queue slot, even on a
// colliding `library_item_id`.
#[test]
fn episode_progress_update_does_not_touch_book_slots() {
    let mut queue = PlaybackQueue::from_queue_items(vec![book_qi("shared_1")], Some(0));
    let before = queue.slots()[0]
        .item
        .as_audiobookshelf_book()
        .unwrap()
        .position_ticks;

    apply_audiobookshelf_progress(
        AudiobookshelfProgressUpdate {
            generation: SetupGeneration::new(1),
            library_item_id: "shared_1".into(),
            episode_id: "ep-1".into(),
            current_time_seconds: 30.0,
            duration_seconds: 100.0,
            is_finished: false,
        },
        Some(SetupGeneration::new(1)),
        &mut queue,
        &Arc::new(Mutex::new(CtrlClients::default())),
    );

    let book = queue.slots()[0].item.as_audiobookshelf_book().unwrap();
    assert_eq!(
        book.position_ticks, before,
        "an episode progress update must not move a book-shaped slot"
    );
    assert!(!book.is_finished);
}
