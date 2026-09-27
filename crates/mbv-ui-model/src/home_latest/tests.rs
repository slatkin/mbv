use super::*;
use mbv_emby_model::{EmbyArtistRef, EmbyImageTags, EmbyItem, EmbyLink, EmbyPerson};
use mbv_queue::FeedKind;
use mbv_queue::{AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry};
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
