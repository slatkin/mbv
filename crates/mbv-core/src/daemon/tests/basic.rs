use super::*;
use crate::api::EmbyImageTags;
use crate::player::PlayerOwnerState;

pub fn item(name: &str, media_type: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: name.into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        child_count: None,
        media_type: media_type.into(),
        collection_type: String::new(),
        runtime_ticks: 0,
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
        artist_items: Vec::new(),
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
        image_tags: EmbyImageTags::default(),
    }
}

pub fn emby_qi(name: &str, media_type: &str, item_type: &str) -> QueueItem {
    QueueItem::Emby(Box::new(item(name, media_type, item_type)))
}

pub fn video_feed_qi(guid: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("video/mp4".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Video),
        feed_id: None,
        position_ticks: 0,
        played: false,
    })
}
/// Connects a client the same way the accept thread does.
pub fn connect_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(
        tx,
        CtrlTransport::Local,
        crate::ctrl::CtrlAudiobookshelfCapabilities {
            queue: true,
            progress: true,
            book_queue: true,
            book_progress: true,
        },
        true,
    );
    (id, rx)
}

pub fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        queue: Arc::new(Mutex::new(PlaybackQueue::default())),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
        lineage: Arc::new(Mutex::new(crate::ctrl::QueueLineage::default())),
        observed_active_slot: Arc::new(Mutex::new(None)),
    }
}

pub fn cold_player() -> Player {
    let (event_tx, _event_rx) = mpsc::channel::<PlayerEvent>();
    Player::new(
        String::new(),
        String::new(),
        false,
        false,
        true,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    )
}

pub fn recv_event(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
    match rx.recv().unwrap() {
        CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
        CtrlOutbound::Flush(_) => panic!("expected a control event"),
    }
}

/// Helper: builds a `PlaybackQueue` from a list of `EmbyItem`s with an active index.
pub fn queue_from_items(items: &[EmbyItem], active: usize) -> PlaybackQueue {
    let qi: Vec<QueueItem> = items
        .iter()
        .cloned()
        .map(|i| QueueItem::Emby(Box::new(i)))
        .collect();
    PlaybackQueue::from_queue_items(qi, Some(active))
}

// Control-client authority and lifetime behavior.
mod clients;

// Queue adoption and asynchronous enrichment behavior.
mod adoption;

// Stale identity and observation rejection behavior.
mod stale_identity;

// Relative playback intents and active-file jump confirmation behavior.
mod playback_intents;
