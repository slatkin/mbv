#[test]
fn podcast_episode_targets_include_parent_show_identity() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].detail_cache.insert(
        "show-a".into(),
        vec![mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-1".into(),
            title: "A".into(),
            description: None,
            published_at: None,
            duration_seconds: Some(1.0),
        }],
    );
    app.audiobookshelf_browse[0].detail_cache.insert(
        "show-b".into(),
        vec![mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-b".into(),
            episode_id: "episode-1".into(),
            title: "B".into(),
            description: None,
            published_at: None,
            duration_seconds: Some(1.0),
        }],
    );
    let first = crate::app::components::msg::PodcastEpisodeTarget::new(
        "show-a".into(),
        "episode-1".into(),
    );
    let second = crate::app::components::msg::PodcastEpisodeTarget::new(
        "show-b".into(),
        "episode-1".into(),
    );
    let first_item = app.selected_audiobookshelf_queue_item_target(0, &first).unwrap();
    let second_item = app.selected_audiobookshelf_queue_item_target(0, &second).unwrap();
    assert_eq!(first_item.as_audiobookshelf().unwrap().library_item_id, "show-a");
    assert_eq!(second_item.as_audiobookshelf().unwrap().library_item_id, "show-b");
    // The parent-show metadata follows each episode's own `library_item_id`,
    // never the tab's current selection (task 2.3): the tab's selection is
    // "show-a", yet the show-b episode does not inherit its metadata.
    assert_eq!(
        first_item.as_audiobookshelf().unwrap().show_title.as_deref(),
        Some("Show A")
    );
    assert_eq!(second_item.as_audiobookshelf().unwrap().show_title, None);
}

#[test]
fn podcast_shelf_queue_target_uses_latest_browse_progress() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_shelf_cache.insert(
        "abs-podcasts".into(),
        vec![mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
                title: "Episode A".into(),
                show_title: Some("Show A".into()),
                author: None,
                description: None,
                duration_ticks: Some(100 * mbv_core::api::TICKS_PER_SECOND as u64),
                position_ticks: 0,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: None,
            },
        )],
    );
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_core::audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 42.5,
            is_finished: false,
        },
    );
    let target = crate::app::components::msg::PodcastEpisodeTarget::new(
        "show-a".into(),
        "episode-a".into(),
    );

    let item = app
        .selected_audiobookshelf_queue_item_target(0, &target)
        .expect("shelf target resolves");
    let episode = item.as_audiobookshelf().unwrap();
    assert_eq!(
        episode.position_ticks,
        (42.5 * mbv_core::api::TICKS_PER_SECOND as f64).round() as i64
    );
    assert!(!episode.played);
    assert!(!episode.is_finished);
}
