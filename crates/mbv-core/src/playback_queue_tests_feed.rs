#[test]
fn feed_entry_primary_source_returns_enclosure() {
    let entry = FeedEntry {
        guid: "g1".into(),
        title: "T".into(),
        enclosure_url: Some("https://enc.mp3".into()),
        link: Some("https://link.html".into()),
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    assert_eq!(entry.primary_source(), Some("https://enc.mp3"));
}

#[test]
fn feed_entry_primary_source_falls_back_to_link() {
    let entry = FeedEntry {
        guid: "g2".into(),
        title: "T".into(),
        enclosure_url: None,
        link: Some("https://link.html".into()),
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    assert_eq!(entry.primary_source(), Some("https://link.html"));
}

#[test]
fn feed_entry_primary_source_none_when_empty() {
    let entry = FeedEntry {
        guid: "g3".into(),
        title: "T".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    assert_eq!(entry.primary_source(), None);
}

// ---------------------------------------------------------------------------
// Feed media-kind classification (task 1.1)
// ---------------------------------------------------------------------------

#[test]
fn feed_media_kind_uses_mime_when_present() {
    let entry = FeedEntry {
        guid: "g1".into(),
        title: "T".into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Video),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let qi = QueueItem::Feed(entry);
    assert_eq!(qi.media_kind(), "Audio");
    assert!(qi.is_audio());
    assert!(!qi.is_video());
}

#[test]
fn feed_media_kind_falls_back_to_feed_kind_when_mime_absent() {
    let entry = FeedEntry {
        guid: "g2".into(),
        title: "T".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Video),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let qi = QueueItem::Feed(entry);
    assert_eq!(qi.media_kind(), "video");
    assert!(!qi.is_audio());
    assert!(qi.is_video());
}

#[test]
fn feed_media_kind_falls_back_to_feed_kind_for_unrecognized_mime() {
    let entry = FeedEntry {
        guid: "g3".into(),
        title: "T".into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("application/octet-stream".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let qi = QueueItem::Feed(entry);
    assert_eq!(qi.media_kind(), "audio");
    assert!(qi.is_audio());
    assert!(!qi.is_video());
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
