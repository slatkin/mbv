// Tests for task 4.1: mixed-version initial snapshots, later broadcasts,
// reconnects, and inbound mutations with capable and older unified peers
// attached simultaneously.

use super::*;

pub fn abs_qi(library_item_id: &str, episode_id: &str) -> QueueItem {
    QueueItem::Audiobookshelf(crate::playback_queue::AudiobookshelfItem::Episode(
        AudiobookshelfQueueItem {
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
        },
    ))
}

fn connect_old_unified_peer(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    // abs_queue=false, abs_progress=false, abs_book_*=false
    let id = clients.connect(
        tx,
        CtrlTransport::Local,
        crate::ctrl::CtrlAudiobookshelfCapabilities::default(),
        false,
    );
    (id, rx)
}

fn recv_unified_queue(rx: &mpsc::Receiver<CtrlOutbound>) -> crate::ctrl::UnifiedQueueStateData {
    match recv_event(rx) {
        CtrlEvent::UnifiedQueueState(data) => data,
        _ => panic!("expected UnifiedQueueState"),
    }
}

fn progress_update(
    generation: u64,
    current_time_seconds: f64,
    is_finished: bool,
) -> AudiobookshelfProgressUpdate {
    AudiobookshelfProgressUpdate {
        generation: SetupGeneration::new(generation),
        library_item_id: "li_1".into(),
        episode_id: "ep_1".into(),
        current_time_seconds,
        duration_seconds: 100.0,
        is_finished,
    }
}

fn abs_queue_with_slot() -> PlaybackQueue {
    PlaybackQueue::from_queue_items(vec![abs_qi("li_1", "ep_1")], Some(0))
}

mod admission;
mod progress;
mod projection;
