// ---------------------------------------------------------------------------
// Now-playing title parts — the shared media-type mapping on `QueueItem`
// (tasks 1.1, 1.2): one `#[case]` per media type for the two-part rows, plus
// the degradation cases that must produce exactly one part and no context
// part.
// ---------------------------------------------------------------------------

use super::*;

pub(super) fn emby_item_of_type(
    id: &str,
    item_type: &str,
    media_type: &str,
    name: &str,
) -> EmbyItem {
    let mut e = item(id);
    e.item_type = item_type.to_string();
    e.media_type = media_type.to_string();
    e.name = name.to_string();
    e.series_name = String::new();
    e.artist = String::new();
    e
}

fn title_parts_queue_item(kind: &str) -> QueueItem {
    match kind {
        "emby_movie" => QueueItem::Emby(Box::new(emby_item_of_type(
            "m1", "Movie", "Video", "The Film",
        ))),
        "emby_episode" => {
            let mut e = emby_item_of_type("e1", "Episode", "Video", "Pilot");
            e.series_name = "Series Name".to_string();
            QueueItem::Emby(Box::new(e))
        }
        "emby_audio_track" => {
            let mut a = emby_item_of_type("a1", "Audio", "Audio", "Track Name");
            a.artist = "Artist Name".to_string();
            QueueItem::Emby(Box::new(a))
        }
        "emby_home_video" => QueueItem::Emby(Box::new(emby_item_of_type(
            "v1",
            "Video",
            "Video",
            "Home Clip",
        ))),
        "emby_audio_track_without_artist" => QueueItem::Emby(Box::new(emby_item_of_type(
            "a1",
            "Audio",
            "Audio",
            "Track Name",
        ))),
        "abs_podcast_episode" => {
            QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
                title: "Episode Five".into(),
                show_title: Some("Show Title".into()),
                ..audiobookshelf_episode("lib1", "ep1")
            }))
        }
        "abs_book" => {
            QueueItem::Audiobookshelf(AudiobookshelfItem::Book(audiobookshelf_book("lib1")))
        }
        "feed_entry" => QueueItem::Feed(FeedEntry {
            guid: "g1".into(),
            title: "Entry Title".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: Some("https://example.com/feed.xml".into()),
            position_ticks: 0,
            played: false,
        }),
        other => panic!("unknown case kind: {other}"),
    }
}

#[rstest::rstest]
#[case::emby_movie("emby_movie", Some("Sub Name"), "The Film", None)]
#[case::emby_episode("emby_episode", Some("Sub Name"), "Pilot", Some("Series Name"))]
#[case::emby_audio_track(
    "emby_audio_track",
    Some("Sub Name"),
    "Track Name",
    Some("Artist Name")
)]
#[case::emby_home_video("emby_home_video", Some("Sub Name"), "Home Clip", None)]
#[case::audiobookshelf_podcast_episode(
    "abs_podcast_episode",
    Some("Sub Name"),
    "Episode Five",
    Some("Show Title")
)]
#[case::feed_entry_with_subscription(
    "feed_entry",
    Some("Sub Name"),
    "Entry Title",
    Some("Sub Name")
)]
// Degradation cases: exactly one part, no context part.
#[case::abs_podcast_episode_without_show(
    "abs_podcast_episode_without_show",
    Some("Sub Name"),
    "Episode Five",
    None
)]
#[case::feed_entry_without_matching_subscription("feed_entry", None, "Entry Title", None)]
#[case::feed_entry_without_feed_id("feed_entry_without_feed_id", None, "Entry Title", None)]
#[case::abs_book("abs_book", Some("Sub Name"), "ABS book", None)]
#[case::emby_audio_track_without_artist(
    "emby_audio_track_without_artist",
    Some("Sub Name"),
    "Track Name",
    None
)]
fn queue_item_playback_title_parts(
    #[case] kind: &str,
    #[case] feed_subscription_name: Option<&str>,
    #[case] expected_title: &str,
    #[case] expected_context: Option<&str>,
) {
    let queue_item = match kind {
        "abs_podcast_episode_without_show" => {
            let ep = AudiobookshelfQueueItem {
                title: "Episode Five".into(),
                show_title: None,
                ..audiobookshelf_episode("lib1", "ep1")
            };
            QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(ep))
        }
        "feed_entry_without_feed_id" => QueueItem::Feed(FeedEntry {
            guid: "g1".into(),
            title: "Entry Title".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        }),
        other => title_parts_queue_item(other),
    };

    let parts = queue_item.playback_title_parts(feed_subscription_name);

    assert_eq!(parts.title.text, expected_title);
    assert_eq!(parts.title.role, PlaybackTitlePartRole::Title);
    match (expected_context, parts.context) {
        (None, None) => {}
        (Some(expected), Some(part)) => {
            assert_eq!(part.text, expected);
            assert_eq!(part.role, PlaybackTitlePartRole::Context);
        }
        (expected, actual) => {
            panic!("context part mismatch: expected {expected:?}, got {actual:?}")
        }
    }
}

#[test]
fn non_feed_items_ignore_the_feed_subscription_name() {
    // The explicit optional input is read only for feed items; a movie must
    // not pick it up as a context part.
    let movie = QueueItem::Emby(Box::new(emby_item_of_type(
        "m1", "Movie", "Video", "The Film",
    )));
    let parts = movie.playback_title_parts(Some("Sub Name"));
    assert!(parts.context.is_none());
    assert_eq!(parts.title.text, "The Film");
}
