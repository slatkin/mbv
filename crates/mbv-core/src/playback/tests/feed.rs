use super::*;

#[rstest::rstest]
#[case::returns_enclosure(
    "g1",
    Some("https://enc.mp3"),
    Some("https://link.html"),
    Some("https://enc.mp3")
)]
#[case::falls_back_to_link("g2", None, Some("https://link.html"), Some("https://link.html"))]
#[case::none_when_empty("g3", None, None, None)]
fn feed_entry_primary_source(
    #[case] guid: &str,
    #[case] enclosure_url: Option<&str>,
    #[case] link: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let entry = FeedEntry {
        guid: guid.into(),
        title: "T".into(),
        enclosure_url: enclosure_url.map(str::to_owned),
        link: link.map(str::to_owned),
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    assert_eq!(entry.primary_source(), expected);
}

// ---------------------------------------------------------------------------
// Feed media-kind classification (task 1.1)
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::uses_mime_when_present(
    "g1",
    Some("audio/mpeg"),
    crate::config::FeedKind::Video,
    "Audio",
    true,
    false
)]
#[case::falls_back_to_feed_kind_when_mime_absent(
    "g2",
    None,
    crate::config::FeedKind::Video,
    "video",
    false,
    true
)]
#[case::falls_back_to_feed_kind_for_unrecognized_mime(
    "g3",
    Some("application/octet-stream"),
    crate::config::FeedKind::Audio,
    "audio",
    true,
    false
)]
fn feed_media_kind(
    #[case] guid: &str,
    #[case] mime_type: Option<&str>,
    #[case] feed_kind: crate::config::FeedKind,
    #[case] expected_kind: &str,
    #[case] expected_audio: bool,
    #[case] expected_video: bool,
) {
    let entry = FeedEntry {
        guid: guid.into(),
        title: "T".into(),
        enclosure_url: None,
        link: None,
        mime_type: mime_type.map(str::to_owned),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(feed_kind),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let qi = QueueItem::Feed(entry);
    assert_eq!(qi.media_kind(), expected_kind);
    assert_eq!(qi.is_audio(), expected_audio);
    assert_eq!(qi.is_video(), expected_video);
}

#[test]
fn feed_legacy_entry_without_feed_kind_is_neither_audio_nor_video() {
    // Simulates a legacy serialized FeedEntry that lacks feed_kind
    // (serde default = None, i.e. unknown).
    let json = r#"{"kind":"Feed","guid":"g4","title":"T","enclosure_url":null,"link":null,"mime_type":null,"duration_ticks":null}"#;
    let qi: QueueItem = serde_json::from_str(json).unwrap();
    assert_eq!(qi.media_kind(), "Video");
    assert!(!qi.is_audio());
    assert!(!qi.is_video());
}
