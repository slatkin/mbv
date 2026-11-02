use super::*;

pub(super) fn twitch_item(url: &str) -> QueueItem {
    QueueItem::Twitch(
        TwitchItem::from_url(url).unwrap_or_else(|_| panic!("test URL must parse: {url}")),
    )
}

/// Contract: spec *The prompt accepts only Twitch hosts* — the scheme must be
/// http or https, the host must be twitch.tv or a subdomain of it, the path is
/// never examined, and the title derives from the URL.
#[rstest::rstest]
#[case::live_channel(
    "https://www.twitch.tv/somechannel",
    "https://www.twitch.tv/somechannel",
    "somechannel"
)]
#[case::vod(
    "https://www.twitch.tv/videos/123456",
    "https://www.twitch.tv/videos/123456",
    "videos/123456"
)]
#[case::clip_on_subdomain(
    "https://clips.twitch.tv/SomeSlug",
    "https://clips.twitch.tv/SomeSlug",
    "SomeSlug"
)]
#[case::bare_host_no_path("https://twitch.tv", "https://twitch.tv", "twitch.tv")]
fn from_url_accepts_twitch_hosts_and_derives_title(
    #[case] input: &str,
    #[case] expected_url: &str,
    #[case] expected_title: &str,
) {
    let item = TwitchItem::from_url(input).unwrap_or_else(|err| panic!("expected accept: {err}"));
    assert_eq!(item.url, expected_url);
    assert_eq!(item.title, expected_title);
}

#[rstest::rstest]
#[case::other_host("https://www.youtube.com/watch?v=abc")]
#[case::look_alike_host("https://nottwitch.tv/somechannel")]
#[case::non_http_scheme("ftp://twitch.tv/somechannel")]
fn from_url_refuses_non_twitch_urls(#[case] input: &str) {
    assert!(
        TwitchItem::from_url(input).is_err(),
        "expected refusal for {input}"
    );
}

#[test]
fn twitch_item_is_video_and_admissible_for_a_video_owner() {
    let item = twitch_item("https://www.twitch.tv/somechannel");
    assert!(!item.is_audio());
    assert!(item.is_video());
    assert_eq!(item.media_kind(), "Video");
    assert!(item.admissible_for_owner(false, |_| true));
}

#[test]
fn twitch_item_round_trips_through_a_queue_slot() {
    let mut queue = PlaybackQueue::from_queue_items(vec![], None, revision_mint());
    let slot_id = queue.append(twitch_item("https://www.twitch.tv/somechannel"));

    assert_eq!(queue.slot(slot_id).unwrap().item.title(), "somechannel");
    assert_eq!(queue.slot(slot_id).unwrap().item.duration(), None);
    assert_eq!(
        queue.slot(slot_id).unwrap().item.content_id(),
        QueueItemContentId::Twitch("https://www.twitch.tv/somechannel".to_string())
    );
}
