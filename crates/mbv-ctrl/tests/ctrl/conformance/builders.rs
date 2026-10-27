//! Shared fixed-literal payload builders for the fixture corpora (design
//! note N2): the corpus pins wire shapes, not runtime state, so every
//! builder here uses fixed constants and never reads the environment or the
//! clock. The `events/` corpus and the later `shared/` unit both build from
//! these, so a payload literal changes in exactly one place.

use mbv_emby_model::{EmbyImageTags, EmbyItem};
use mbv_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry, FeedKind,
    QueueItem, QueueItemContentId, QueueLineage, QueueSource,
};

use super::*;

// ── Fixed literals ───────────────────────────────────────────────────────

pub(crate) const EMBY_ITEM_ID: &str = "emby-conformance-item";
pub(crate) const EMBY_ITEM_NAME: &str = "Conformance Episode";
pub(crate) const EMBY_SERIES_ID: &str = "emby-series-1";
pub(crate) const FEED_GUID: &str = "conformance-feed-guid";
pub(crate) const FEED_TITLE: &str = "Conformance Feed Entry";
pub(crate) const FEED_URL: &str = "https://feeds.example/conformance/episode-1.mp3";
pub(crate) const FEED_ID: &str = "https://feeds.example/conformance.xml";
pub(crate) const PLAYLIST_NAME: &str = "Conformance Playlist";
pub(crate) const REQUEST_ID: u64 = 7;
pub(crate) const GENERATION: u64 = 3;
pub(crate) const REVISION: u64 = 4;
pub(crate) const CAST_RECEIVER_ID: &str = "cast-receiver-1";
pub(crate) const CAST_NAME: &str = "Conformance Display";
pub(crate) const CAST_HOST: &str = "192.168.1.50";
pub(crate) const CAST_PORT: u16 = 8009;
pub(crate) const EMBY_DEVICE_ID: &str = "emby-device-1";
pub(crate) const EMBY_DEVICE_NAME: &str = "Conformance Session";
pub(crate) const ABS_LIBRARY_ITEM_ID: &str = "abs-library-item-1";
pub(crate) const ABS_EPISODE_ID: &str = "abs-episode-1";
pub(crate) const ABS_BOOK_ITEM_ID: &str = "abs-book-1";
/// Playback position the progress-shaped fixtures carry (30 s in ticks).
pub(crate) const POSITION_TICKS: i64 = 30_000_000;
/// The queued transition's request id, distinct from `REQUEST_ID` so the
/// two `TransitionSummary` rows of the transitions fixture stay separable.
pub(crate) const QUEUED_REQUEST_ID: u64 = 8;
/// The queued transition's generation, distinct from `GENERATION` for the
/// same reason.
pub(crate) const QUEUED_GENERATION: u64 = 4;

// ── Queue payload builders ───────────────────────────────────────────────

/// The playlist `QueueSource` the queue-state fixtures carry.
pub(crate) fn queue_source() -> QueueSource {
    QueueSource::Playlist {
        id: Some("conformance-playlist".to_owned()),
        name: PLAYLIST_NAME.to_owned(),
    }
}

/// The Emby queue item the queue fixtures carry.
pub(crate) fn emby_queue_item() -> QueueItem {
    QueueItem::Emby(Box::new(EmbyItem {
        id: EMBY_ITEM_ID.to_owned(),
        name: EMBY_ITEM_NAME.to_owned(),
        item_type: "Episode".to_owned(),
        is_folder: false,
        child_count: None,
        studios: Vec::new(),
        media_type: "Video".to_owned(),
        collection_type: String::new(),
        runtime_ticks: 1_800_000_000,
        played: false,
        playback_position_ticks: 0,
        series_id: EMBY_SERIES_ID.to_owned(),
        series_name: "Conformance Series".to_owned(),
        album_id: String::new(),
        album: String::new(),
        index_number: 1,
        parent_index_number: 1,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: "conformance episode".to_owned(),
        production_year: 2026,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: "2026-01-01T00:00:00Z".to_owned(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: EmbyImageTags::default(),
    }))
}

/// The feed queue item the queue fixtures carry.
pub(crate) fn feed_queue_item() -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: FEED_GUID.to_owned(),
        title: FEED_TITLE.to_owned(),
        enclosure_url: Some(FEED_URL.to_owned()),
        link: None,
        mime_type: Some("audio/mpeg".to_owned()),
        duration_ticks: Some(1_800_000_000),
        pub_date_secs: Some(1_700_000_000),
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some(FEED_ID.to_owned()),
        position_ticks: 0,
        played: false,
    })
}

/// The provider-qualified progress update the progress-relay fixture
/// carries: an Audiobookshelf episode position, redacted of everything but
/// identity and progress.
pub(crate) fn progress_update() -> ProgressUpdate {
    ProgressUpdate {
        content_id: QueueItemContentId::Audiobookshelf {
            library_item_id: ABS_LIBRARY_ITEM_ID.to_owned(),
            episode_id: ABS_EPISODE_ID.to_owned(),
        },
        position_ticks: POSITION_TICKS,
        finished: false,
    }
}

// ── Status and queue-state builders ──────────────────────────────────────

/// The canonical `PlayerStatus` the status and queue-state fixtures carry:
/// an active Emby episode at a fixed position, English audio, one forced
/// subtitle track.
pub(crate) fn player_status() -> PlayerStatus {
    PlayerStatus {
        position_ticks: POSITION_TICKS,
        last_valid_pos: POSITION_TICKS,
        runtime_ticks: 1_800_000_000,
        paused: false,
        volume: 55,
        volume_max: 130,
        current_idx: 1,
        queue_len: 2,
        sequence_generation: GENERATION,
        active: true,
        title: EMBY_ITEM_NAME.to_owned(),
        artist: "Conformance Artist".to_owned(),
        album: "Conformance Album".to_owned(),
        art_item_id: EMBY_ITEM_ID.to_owned(),
        art_album_id: "emby-album-1".to_owned(),
        audio_tracks: vec![(1, "English".to_owned())],
        sub_tracks: vec![(2, "Forced".to_owned(), true)],
        sub_track_stream_indexes: vec![(2, 5)],
        audio_id: 1,
        audio_lang: "en".to_owned(),
        sub_id: 0,
        sub_lang: String::new(),
        muted: false,
        video_height: 1080,
        audio_codec: String::new(),
        video_is_image: false,
    }
}

/// The canonical `UnifiedQueueStateData`: two slots (Emby episode, feed
/// entry), the first active, under a playlist source, with no transition in
/// flight or queued.
pub(crate) fn unified_queue_state() -> UnifiedQueueStateData {
    UnifiedQueueStateData {
        status: player_status(),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 1,
                item: emby_queue_item(),
            },
            UnifiedQueueSlot {
                slot_id: 2,
                item: feed_queue_item(),
            },
        ],
        active_slot: Some(1),
        revision: 12,
        source: queue_source(),
        lineage: QueueLineage(2),
        in_flight_transition: None,
        queued_latest_transition: None,
    }
}

/// The canonical `UnifiedQueueStateData` with both transition fields set: a
/// `JumpTo` dispatched to the Playback run and awaiting observation, and a
/// newer one queued behind it. Distinct request ids and generations so the
/// two summaries stay separable in the fixture.
pub(crate) fn unified_queue_state_with_transitions() -> UnifiedQueueStateData {
    let mut state = unified_queue_state();
    state.in_flight_transition = Some(TransitionSummary {
        request_id: REQUEST_ID,
        generation: GENERATION,
        target_slot: 2,
    });
    state.queued_latest_transition = Some(TransitionSummary {
        request_id: QUEUED_REQUEST_ID,
        generation: QUEUED_GENERATION,
        target_slot: 2,
    });
    state
}

/// The Audiobookshelf episode queue item the shared fixtures carry.
pub(crate) fn abs_episode_queue_item() -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
        library_item_id: ABS_LIBRARY_ITEM_ID.to_owned(),
        episode_id: ABS_EPISODE_ID.to_owned(),
        title: "Conformance Audiobookshelf Episode".to_owned(),
        show_title: Some("Conformance Show".to_owned()),
        author: Some("Conformance Author".to_owned()),
        description: Some("Conformance episode description".to_owned()),
        duration_ticks: Some(1_800_000_000),
        position_ticks: POSITION_TICKS,
        played: false,
        pub_date_secs: Some(1_700_000_000),
        is_finished: false,
        cover_path: Some("covers/conformance-episode.jpg".to_owned()),
    }))
}

/// The Audiobookshelf book queue item the shared fixtures carry.
pub(crate) fn abs_book_queue_item() -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfItem::Book(AudiobookshelfBookQueueItem {
        library_item_id: ABS_BOOK_ITEM_ID.to_owned(),
        title: "Conformance Audiobookshelf Book".to_owned(),
        author: Some("Conformance Author".to_owned()),
        duration_ticks: Some(3_600_000_000),
        position_ticks: POSITION_TICKS,
        played: false,
        is_finished: false,
        cover_path: None,
    }))
}
