use super::*;

// Tests for task 4.1: mixed-version initial snapshots, later broadcasts,
// reconnects, and inbound mutations with capable and older unified peers
// attached simultaneously.

pub fn abs_qi(library_item_id: &str, episode_id: &str) -> QueueItem {
    use crate::playback_queue::AudiobookshelfQueueItem;
    QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
        library_item_id: library_item_id.into(),
        episode_id: episode_id.into(),
        title: "Test Episode".into(),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}
