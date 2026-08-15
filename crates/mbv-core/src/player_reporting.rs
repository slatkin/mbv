const AUDIOBOOKSHELF_REPORT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

fn seconds_from_ticks(ticks: i64) -> f64 {
    let seconds = ticks.max(0) as f64 / crate::api::TICKS_PER_SECOND as f64;
    (seconds * 1_000_000.0).round() / 1_000_000.0
}

#[derive(Debug, Default)]
struct ListeningTime {
    accumulated: f64,
    last_observed: Option<std::time::Instant>,
    playing: bool,
}

impl ListeningTime {
    fn observe(&mut self, now: std::time::Instant, playing: bool) {
        if let Some(previous) = self.last_observed {
            if self.playing {
                self.accumulated += now.saturating_duration_since(previous).as_secs_f64();
            }
        }
        self.last_observed = Some(now);
        self.playing = playing;
    }

    fn take(&mut self) -> f64 {
        std::mem::take(&mut self.accumulated)
    }
}

pub(crate) struct AudiobookshelfPlaybackLifecycle {
    pub(crate) generation: crate::service_runtime::SetupGeneration,
    client: crate::audiobookshelf::AudiobookshelfClient,
    credential: String,
    pub(crate) session_id: String,
    library_item_id: String,
    episode_id: String,
    progress_sender: Option<std::sync::mpsc::Sender<AudiobookshelfProgressUpdate>>,
    pub(crate) current_position: f64,
    pub(crate) duration: f64,
    pub(crate) last_acknowledgement: Option<crate::audiobookshelf::AudiobookshelfPlaybackProgress>,
    listening_time: ListeningTime,
    in_flight: bool,
    last_sync: std::time::Instant,
    closed: bool,
}

impl AudiobookshelfPlaybackLifecycle {
    pub(crate) fn new(
        generation: crate::service_runtime::SetupGeneration,
        client: crate::audiobookshelf::AudiobookshelfClient,
        credential: String,
        session_id: String,
        library_item_id: String,
        episode_id: String,
        current_position: f64,
        duration: f64,
        progress_sender: Option<std::sync::mpsc::Sender<AudiobookshelfProgressUpdate>>,
    ) -> Self {
        Self {
            generation,
            client,
            credential,
            session_id,
            library_item_id,
            episode_id,
            progress_sender,
            current_position,
            duration,
            last_acknowledgement: None,
            listening_time: ListeningTime::default(),
            in_flight: false,
            last_sync: std::time::Instant::now(),
            closed: false,
        }
    }

    fn observe(&mut self, now: std::time::Instant, playing: bool) {
        self.listening_time.observe(now, playing);
    }

    fn should_sync(&self, now: std::time::Instant, force: bool) -> bool {
        force || now.saturating_duration_since(self.last_sync) >= AUDIOBOOKSHELF_REPORT_INTERVAL
    }

    fn sync(&mut self, position_ticks: i64, now: std::time::Instant, force: bool) {
        if self.closed || self.in_flight || !self.should_sync(now, force) {
            return;
        }
        self.sync_final(position_ticks, now);
    }

    fn sync_final(&mut self, position_ticks: i64, now: std::time::Instant) {
        if self.in_flight {
            return;
        }
        self.current_position = seconds_from_ticks(position_ticks);
        let progress = crate::audiobookshelf::AudiobookshelfPlaybackProgress {
            current_time: self.current_position,
            time_listened: self.listening_time.take(),
            duration: self.duration,
        };
        // Clear the interval before dispatch. A timeout or lost response is
        // deliberately not replayed, because the server may have accepted it.
        self.in_flight = true;
        let result = self.client.sync_playback_session_bounded(
            &self.credential,
            &self.session_id,
            progress,
            crate::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
        );
        self.in_flight = false;
        self.last_sync = now;
        if result.is_ok() {
            self.last_acknowledgement = Some(progress);
            if let Some(sender) = &self.progress_sender {
                let _ = sender.send(AudiobookshelfProgressUpdate {
                    generation: self.generation,
                    library_item_id: self.library_item_id.clone(),
                    episode_id: self.episode_id.clone(),
                    current_time_seconds: self.current_position,
                    duration_seconds: self.duration,
                    is_finished: self.duration > 0.0 && self.current_position >= self.duration,
                });
            }
        } else {
            log::warn!(target: "player", "Audiobookshelf progress synchronization failed");
        }
    }

    fn close(&mut self, position_ticks: i64) {
        if self.closed {
            return;
        }
        self.closed = true;
        log::debug!(target: "player", "closing Audiobookshelf lifecycle generation={}", self.generation.value());
        let now = std::time::Instant::now();
        self.observe(now, false);
        self.sync_final(position_ticks, now);
        let progress = crate::audiobookshelf::AudiobookshelfPlaybackProgress {
            current_time: seconds_from_ticks(position_ticks),
            time_listened: 0.0,
            duration: self.duration,
        };
        let _ = self.client.close_playback_session_bounded(
            &self.credential,
            &self.session_id,
            progress,
            crate::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
        );
    }
}

/// Book-shaped counterpart to `AudiobookshelfPlaybackLifecycle`: identical
/// session sync/close mechanics, but it reports `AudiobookshelfBookProgressUpdate`
/// keyed by `library_item_id` only. Kept separate so a book session can never
/// be matched against (or emit progress for) an episode.
pub(crate) struct AudiobookshelfBookPlaybackLifecycle {
    pub(crate) generation: crate::service_runtime::SetupGeneration,
    client: crate::audiobookshelf::AudiobookshelfClient,
    credential: String,
    pub(crate) session_id: String,
    library_item_id: String,
    progress_sender: Option<std::sync::mpsc::Sender<AudiobookshelfBookProgressUpdate>>,
    pub(crate) current_position: f64,
    pub(crate) duration: f64,
    pub(crate) last_acknowledgement: Option<crate::audiobookshelf::AudiobookshelfPlaybackProgress>,
    listening_time: ListeningTime,
    in_flight: bool,
    last_sync: std::time::Instant,
    closed: bool,
}

impl AudiobookshelfBookPlaybackLifecycle {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        generation: crate::service_runtime::SetupGeneration,
        client: crate::audiobookshelf::AudiobookshelfClient,
        credential: String,
        session_id: String,
        library_item_id: String,
        current_position: f64,
        duration: f64,
        progress_sender: Option<std::sync::mpsc::Sender<AudiobookshelfBookProgressUpdate>>,
    ) -> Self {
        Self {
            generation,
            client,
            credential,
            session_id,
            library_item_id,
            progress_sender,
            current_position,
            duration,
            last_acknowledgement: None,
            listening_time: ListeningTime::default(),
            in_flight: false,
            last_sync: std::time::Instant::now(),
            closed: false,
        }
    }

    fn observe(&mut self, now: std::time::Instant, playing: bool) {
        self.listening_time.observe(now, playing);
    }

    fn should_sync(&self, now: std::time::Instant, force: bool) -> bool {
        force || now.saturating_duration_since(self.last_sync) >= AUDIOBOOKSHELF_REPORT_INTERVAL
    }

    fn sync(&mut self, position_ticks: i64, now: std::time::Instant, force: bool) {
        if self.closed || self.in_flight || !self.should_sync(now, force) {
            return;
        }
        self.sync_final(position_ticks, now);
    }

    fn sync_final(&mut self, position_ticks: i64, now: std::time::Instant) {
        if self.in_flight {
            return;
        }
        self.current_position = seconds_from_ticks(position_ticks);
        let progress = crate::audiobookshelf::AudiobookshelfPlaybackProgress {
            current_time: self.current_position,
            time_listened: self.listening_time.take(),
            duration: self.duration,
        };
        // Clear the interval before dispatch. A timeout or lost response is
        // deliberately not replayed, because the server may have accepted it.
        self.in_flight = true;
        let result = self.client.sync_playback_session_bounded(
            &self.credential,
            &self.session_id,
            progress,
            crate::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
        );
        self.in_flight = false;
        self.last_sync = now;
        if result.is_ok() {
            self.last_acknowledgement = Some(progress);
            if let Some(sender) = &self.progress_sender {
                let _ = sender.send(AudiobookshelfBookProgressUpdate {
                    generation: self.generation,
                    library_item_id: self.library_item_id.clone(),
                    current_time_seconds: self.current_position,
                    duration_seconds: self.duration,
                    is_finished: self.duration > 0.0 && self.current_position >= self.duration,
                });
            }
        } else {
            log::warn!(target: "player", "Audiobookshelf book progress synchronization failed");
        }
    }

    fn close(&mut self, position_ticks: i64) {
        if self.closed {
            return;
        }
        self.closed = true;
        log::debug!(target: "player", "closing Audiobookshelf book lifecycle generation={}", self.generation.value());
        let now = std::time::Instant::now();
        self.observe(now, false);
        self.sync_final(position_ticks, now);
        let progress = crate::audiobookshelf::AudiobookshelfPlaybackProgress {
            current_time: seconds_from_ticks(position_ticks),
            time_listened: 0.0,
            duration: self.duration,
        };
        let _ = self.client.close_playback_session_bounded(
            &self.credential,
            &self.session_id,
            progress,
            crate::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
        );
    }
}

impl Drop for AudiobookshelfBookPlaybackLifecycle {
    fn drop(&mut self) {
        self.close((self.current_position * crate::api::TICKS_PER_SECOND as f64).round() as i64);
    }
}

pub(crate) enum ActiveItemLifecycle {
    Emby,
    Audiobookshelf(Box<AudiobookshelfPlaybackLifecycle>),
    AudiobookshelfBook(Box<AudiobookshelfBookPlaybackLifecycle>),
    None,
}

/// Bounded ABS session owned by a prepared source, before it is promoted to
/// the active slot. Kept separate from `ActiveItemLifecycle` because a
/// prepared source can be discarded before its projection is installed.
pub(crate) enum PreparedLifecycle {
    Episode(AudiobookshelfPlaybackLifecycle),
    Book(AudiobookshelfBookPlaybackLifecycle),
}

impl PreparedLifecycle {
    fn close(&mut self, position_ticks: i64) {
        match self {
            PreparedLifecycle::Episode(lifecycle) => lifecycle.close(position_ticks),
            PreparedLifecycle::Book(lifecycle) => lifecycle.close(position_ticks),
        }
    }
}

impl ActiveItemLifecycle {
    fn for_item(item: &QueueItem, lifecycle: Option<PreparedLifecycle>) -> Self {
        match item {
            QueueItem::Emby(_) => Self::Emby,
            QueueItem::Audiobookshelf(_) => match lifecycle {
                Some(PreparedLifecycle::Episode(lifecycle)) => {
                    Self::Audiobookshelf(Box::new(lifecycle))
                }
                _ => Self::None,
            },
            QueueItem::AudiobookshelfBook(_) => match lifecycle {
                Some(PreparedLifecycle::Book(lifecycle)) => {
                    Self::AudiobookshelfBook(Box::new(lifecycle))
                }
                _ => Self::None,
            },
            QueueItem::Feed(_) => Self::None,
        }
    }

    pub(crate) fn observe(&mut self, now: std::time::Instant, playing: bool) {
        match self {
            Self::Audiobookshelf(lifecycle) => lifecycle.observe(now, playing),
            Self::AudiobookshelfBook(lifecycle) => lifecycle.observe(now, playing),
            Self::Emby | Self::None => {}
        }
    }

    pub(crate) fn sync(&mut self, position_ticks: i64, now: std::time::Instant, force: bool) {
        match self {
            Self::Audiobookshelf(lifecycle) => lifecycle.sync(position_ticks, now, force),
            Self::AudiobookshelfBook(lifecycle) => lifecycle.sync(position_ticks, now, force),
            Self::Emby | Self::None => {}
        }
    }

    fn close(&mut self, position_ticks: i64) {
        let lifecycle = std::mem::replace(self, Self::None);
        match lifecycle {
            Self::Audiobookshelf(mut lifecycle) => lifecycle.close(position_ticks),
            Self::AudiobookshelfBook(mut lifecycle) => lifecycle.close(position_ticks),
            Self::Emby | Self::None => {}
        }
    }
}

#[cfg(test)]
mod reporting_tests {
    use super::{ActiveItemLifecycle, ListeningTime};
    use crate::api::EmbyItem;
    use crate::playback_queue::{AudiobookshelfQueueItem, FeedEntry, QueueItem};
    use std::time::{Duration, Instant};

    fn emby() -> QueueItem {
        QueueItem::Emby(Box::new(EmbyItem {
            id: "emby".into(),
            name: "Emby".into(),
            item_type: "Movie".into(),
            is_folder: false,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 1,
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
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }))
    }

    fn feed() -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: "feed".into(),
            title: "Feed".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        })
    }

    fn audiobook() -> QueueItem {
        QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: "library".into(),
            episode_id: "episode".into(),
            title: "ABS".into(),
            show_title: None,
            author: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        })
    }

    #[test]
    fn listening_time_counts_playing_wall_clock_only() {
        let start = Instant::now();
        let mut time = ListeningTime::default();
        time.observe(start, true);
        time.observe(start + Duration::from_secs(4), false);
        time.observe(start + Duration::from_secs(9), true);
        time.observe(start + Duration::from_secs(12), true);
        assert!((time.take() - 7.0).abs() < 0.001);
    }

    #[test]
    fn active_item_lifecycle_is_closed_over_item_kinds() {
        assert!(matches!(
            ActiveItemLifecycle::for_item(&emby(), None),
            ActiveItemLifecycle::Emby
        ));
        assert!(matches!(
            ActiveItemLifecycle::for_item(&feed(), None),
            ActiveItemLifecycle::None
        ));
        assert!(matches!(
            ActiveItemLifecycle::for_item(&audiobook(), None),
            ActiveItemLifecycle::None
        ));
        let lifecycle = super::AudiobookshelfPlaybackLifecycle::new(
            crate::service_runtime::SetupGeneration::new(4),
            crate::audiobookshelf::AudiobookshelfClient::new("http://127.0.0.1:1").unwrap(),
            "secret".into(),
            "session".into(),
            "library".into(),
            "episode".into(),
            0.0,
            10.0,
            None,
        );
        let mut lifecycle = lifecycle;
        lifecycle.closed = true;
        assert!(matches!(
            ActiveItemLifecycle::for_item(
                &audiobook(),
                Some(super::PreparedLifecycle::Episode(lifecycle))
            ),
            ActiveItemLifecycle::Audiobookshelf(_)
        ));
    }

    #[test]
    fn dispatched_time_is_consumed_once_and_is_not_position_scaled() {
        let start = Instant::now();
        let mut time = ListeningTime::default();
        time.observe(start, true);
        time.observe(start + Duration::from_secs(2), true);
        assert!((time.take() - 2.0).abs() < 0.001);
        assert_eq!(time.take(), 0.0);
        time.observe(start + Duration::from_secs(7), true);
        assert!((time.take() - 5.0).abs() < 0.001);
    }

    #[test]
    fn close_is_idempotent_and_clears_state_when_reporting_fails() {
        let lifecycle = super::AudiobookshelfPlaybackLifecycle::new(
            crate::service_runtime::SetupGeneration::new(9),
            crate::audiobookshelf::AudiobookshelfClient::new("http://127.0.0.1:1").unwrap(),
            "secret".into(),
            "session".into(),
            "library".into(),
            "episode".into(),
            0.0,
            10.0,
            None,
        );
        let mut active = ActiveItemLifecycle::Audiobookshelf(Box::new(lifecycle));
        active.close(42);
        assert!(matches!(active, ActiveItemLifecycle::None));
        active.close(84);
        assert!(matches!(active, ActiveItemLifecycle::None));
    }

    #[test]
    fn failed_sync_consumes_interval_without_retry_or_acknowledgement() {
        let start = Instant::now();
        let mut lifecycle = super::AudiobookshelfPlaybackLifecycle::new(
            crate::service_runtime::SetupGeneration::new(10),
            crate::audiobookshelf::AudiobookshelfClient::new("http://127.0.0.1:1").unwrap(),
            "secret".into(),
            "session".into(),
            "library".into(),
            "episode".into(),
            0.0,
            10.0,
            None,
        );
        lifecycle.observe(start, true);
        let failed_at = start + Duration::from_secs(11);
        lifecycle.sync(2 * crate::api::TICKS_PER_SECOND, failed_at, true);
        lifecycle.sync(
            3 * crate::api::TICKS_PER_SECOND,
            failed_at + Duration::from_secs(1),
            false,
        );

        assert_eq!(lifecycle.last_sync, failed_at);
        assert!(lifecycle.last_acknowledgement.is_none());
        assert_eq!(lifecycle.listening_time.take(), 0.0);
    }
}

impl Drop for AudiobookshelfPlaybackLifecycle {
    fn drop(&mut self) {
        self.close((self.current_position * crate::api::TICKS_PER_SECOND as f64).round() as i64);
    }
}
