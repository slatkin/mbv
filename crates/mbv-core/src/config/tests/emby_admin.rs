#[test]
fn emby_setup_normalizes_server_and_starts_at_revision_one() {
    let setup = EmbySetup::new("  https://emby.example/// ", " user-1 ");
    assert_eq!(setup.server_url, "https://emby.example");
    assert_eq!(setup.user_id, "user-1");
    assert_eq!(setup.revision, 1);
}

#[test]
fn different_server_replacement_clears_only_emby_owned_state() {
    let _guard = TestStateDirGuard::new();
    let mut config = Config {
        emby_setup: Some(EmbySetup::new("https://old.example", "old-user")),
        ..Config::default()
    };
    config.feeds.push(FeedSubscription {
        name: "News".into(),
        url: "https://feeds.example/news".into(),
        kind: FeedKind::Audio,
    });
    save_config_settings(&config).unwrap();
    save_service_secret(ServiceKind::Emby, "old-token").unwrap();
    save_queue_state(&QueueState {
        source: QueueSource::Unknown,
        items: vec![crate::playback_queue::QueueItem::Feed(
            crate::playback_queue::FeedEntry {
                guid: "feed-1".into(),
                title: "Episode".into(),
                enclosure_url: Some("https://feeds.example/episode.mp3".into()),
                link: None,
                mime_type: Some("audio/mpeg".into()),
                duration_ticks: None,
                pub_date_secs: None,
                feed_kind: Some(FeedKind::Audio),
                feed_id: None,
                position_ticks: 0,
                played: false,
            },
        )],
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    })
    .unwrap();

    let mut replacement = EmbySetup::new("https://new.example/", "new-user");
    replacement.revision = 2;
    replace_emby_setup_and_secret(&replacement, "new-token").unwrap();

    let state = load_queue_state().unwrap();
    assert_eq!(state.items.len(), 1);
    assert!(matches!(
        state.items[0],
        crate::playback_queue::QueueItem::Feed(_)
    ));
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("new-token")
    );
    assert_eq!(load_config().unwrap().feeds.len(), 1);
}

#[test]
fn persisted_setup_revision_round_trips_without_persisting_password() {
    let _guard = TestStateDirGuard::new();
    let mut setup = EmbySetup::new("https://emby.example", "user-1");
    setup.revision = 9;
    persist_emby_setup_and_secret(&setup, "long-lived-token").unwrap();
    let text = std::fs::read_to_string(config_path()).unwrap();
    assert!(text.contains("revision = 9"));
    assert!(!text.contains("long-lived-token"));
    assert!(!text.contains("password"));
    assert_eq!(load_config().unwrap().emby_setup.unwrap().revision, 9);
}
