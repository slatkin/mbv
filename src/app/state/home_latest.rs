use mbv_core::playback_queue::QueueItem;
use std::time::{SystemTime, UNIX_EPOCH};

/// The launch-relative interval used by destination Latest markers.
/// `previous` is intentionally immutable and separate from exit-only UI state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HomeLatestLaunchWindow {
    pub(in crate::app) previous: Option<u64>,
    pub(in crate::app) current: u64,
}

pub(crate) fn current_launch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn capture_launch_window(current: u64) -> HomeLatestLaunchWindow {
    if current == 0 {
        log::warn!(target: "home_latest", "invalid non-positive launch cutoff; markers disabled");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }

    let previous = mbv_core::config::load_home_latest_launch();
    if let Err(error) = mbv_core::config::save_home_latest_launch(current) {
        log::warn!(target: "home_latest", "could not save launch cutoff: {error}");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    HomeLatestLaunchWindow { previous, current }
}

/// Normalize the provider timestamp carried by a destination Latest item.
pub(in crate::app) fn provider_timestamp_secs(item: &QueueItem) -> Option<u64> {
    match item {
        QueueItem::Emby(item) => {
            crate::app::infra::feed_parse::parse_pub_date_secs(&item.date_added)
        }
        QueueItem::Feed(entry) => entry.pub_date_secs,
        QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Episode(
            episode,
        )) => episode.pub_date_secs,
        QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Book(_)) => None,
    }
}

pub(in crate::app) fn is_new_in_launch_window(
    item: &QueueItem,
    window: HomeLatestLaunchWindow,
) -> bool {
    let Some(previous) = window.previous else {
        return false;
    };
    provider_timestamp_secs(item)
        .is_some_and(|timestamp| previous < timestamp && timestamp <= window.current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::api::{EmbyArtistRef, EmbyImageTags, EmbyItem, EmbyLink, EmbyPerson};
    use mbv_core::config::FeedKind;
    use mbv_core::playback_queue::{AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry};
    use rstest::rstest;

    fn emby(date_added: &str) -> QueueItem {
        QueueItem::Emby(Box::new(EmbyItem {
            id: "emby".into(),
            name: "Emby".into(),
            item_type: "Movie".into(),
            is_folder: false,
            child_count: None,
            media_type: "Video".into(),
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
            artist_items: Vec::<EmbyArtistRef>::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: date_added.into(),
            total_count: 0,
            container: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genres: Vec::new(),
            people: Vec::<EmbyPerson>::new(),
            external_urls: Vec::<EmbyLink>::new(),
            playlist_item_id: String::new(),
            image_tags: EmbyImageTags::default(),
        }))
    }

    fn episode(pub_date_secs: Option<u64>) -> QueueItem {
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
            library_item_id: "library".into(),
            episode_id: "episode".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs,
            is_finished: false,
            cover_path: None,
        }))
    }

    fn feed(pub_date_secs: Option<u64>) -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: "feed".into(),
            title: "Feed".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs,
            feed_kind: Some(FeedKind::Audio),
            feed_id: None,
            position_ticks: 0,
            played: false,
        })
    }

    #[rstest]
    #[case::emby(emby("1970-01-01T00:02:30Z"), Some(150))]
    #[case::emby_date_only(emby("1970-01-01"), Some(0))]
    #[case::feed(feed(Some(150)), Some(150))]
    #[case::missing(episode(None), None)]
    #[case::cutoff_equal(feed(Some(100)), Some(100))]
    fn provider_timestamps_are_normalized_across_services(
        #[case] item: QueueItem,
        #[case] expected: Option<u64>,
    ) {
        assert_eq!(provider_timestamp_secs(&item), expected);
    }

    #[rstest]
    #[case::qualifies(emby("1970-01-01T00:02:30Z"), true)]
    #[case::cutoff_equal(feed(Some(100)), false)]
    #[case::missing(episode(None), false)]
    #[case::invalid(emby("not-a-date"), false)]
    fn launch_window_uses_a_closed_upper_bound(#[case] item: QueueItem, #[case] expected: bool) {
        let window = HomeLatestLaunchWindow {
            previous: Some(100),
            current: 200,
        };
        assert_eq!(is_new_in_launch_window(&item, window), expected);
    }
}
