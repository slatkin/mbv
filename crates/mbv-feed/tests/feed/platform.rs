//! Contract: a URL shows its Feed platform.

use mbv_feed::FeedPlatform;
use rstest::rstest;

/// One case per feed form in design D1, plus the `PeerTube` forms that need the
/// path rule and the generic forms.
#[rstest]
#[case::youtube(
    "https://www.youtube.com/feeds/videos.xml?channel_id=UCabc123",
    Some(FeedPlatform::YouTube)
)]
#[case::soundcloud(
    "https://feeds.soundcloud.com/users/soundcloud:users:123456/sounds.rss",
    Some(FeedPlatform::SoundCloud)
)]
#[case::spreaker(
    "https://www.spreaker.com/show/1234567/episodes/feed",
    Some(FeedPlatform::Spreaker)
)]
#[case::audioboom(
    "https://audioboom.com/channels/1234567.rss",
    Some(FeedPlatform::Audioboom)
)]
#[case::patreon(
    "https://www.patreon.com/rss/creator?auth=abc123",
    Some(FeedPlatform::Patreon)
)]
#[case::substack("https://name.substack.com/feed", Some(FeedPlatform::Substack))]
#[case::spotify_for_creators(
    "https://anchor.fm/s/1234567/podcast/rss",
    Some(FeedPlatform::Spotify)
)]
#[case::odysee("https://odysee.com/$/rss/@channel:1", Some(FeedPlatform::Odysee))]
#[case::internet_archive(
    "https://archive.org/services/collection-rss.php?collection=id",
    Some(FeedPlatform::InternetArchive)
)]
#[case::peer_tube_channel(
    "https://video.example.com/feeds/videos.xml?videoChannelName=tech",
    Some(FeedPlatform::PeerTube)
)]
#[case::peer_tube_playlist_podcast(
    "https://video.example.com/feeds/podcast/videos.xml?playlistId=42",
    Some(FeedPlatform::PeerTube)
)]
#[case::peer_tube_instance_wide("https://video.example.com/feeds/videos.xml", None)]
#[case::youtube_lookalike("https://notyoutube.com/feed", None)]
#[case::userinfo_naming_youtube("https://youtube.com@evil.example/feed", None)]
#[case::query_naming_youtube("https://evil.example?@youtube.com/feed", None)]
#[case::no_path("https://www.youtube.com?si=x", None)]
#[case::userinfo_before_youtube(
    "https://user:pass@www.youtube.com/feeds/videos.xml?channel_id=UCabc",
    Some(FeedPlatform::YouTube)
)]
#[case::generic_podcast("https://podcast.example.com/feed.xml", None)]
fn a_url_shows_its_feed_platform(#[case] url: &str, #[case] platform: Option<FeedPlatform>) {
    assert_eq!(FeedPlatform::of_url(url), platform);
}
