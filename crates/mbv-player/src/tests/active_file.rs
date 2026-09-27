use super::*;

pub(crate) fn noop_progress() -> ProgressGuard {
    let (stop_tx, _) = mpsc::channel();
    ProgressGuard {
        stop_tx,
        handle: None,
    }
}

pub(crate) fn abs_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
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
        },
    ))
}

pub(crate) fn abs_book_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(
        mbv_queue::AudiobookshelfBookQueueItem {
            library_item_id: "book".into(),
            title: "Book".into(),
            author: None,
            duration_ticks: Some(100_u64),
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: None,
        },
    ))
}
