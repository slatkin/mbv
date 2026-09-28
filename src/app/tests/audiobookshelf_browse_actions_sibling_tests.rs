#[test]
fn shelf_cache_hit_carries_browse_progress() {
    // Regression for 50c4d12d / issue #844: cached shelf content must not erase
    // the current browse progress when it becomes a submitted QueueItem.
    let mut app = super::podcast::audiobookshelf_app();
    app.audiobookshelf_shelf_cache.insert(
        "abs-podcasts".into(),
        vec![mbv_queue::AudiobookshelfEpisodeCatalog {
            library_item_id: "show-a".into(),
            episode_id: "episode-1".into(),
            title: "Episode".into(),
            show_title: Some("Show A".into()),
            author: None,
            description: None,
            duration_ticks: None,
            pub_date_secs: None,
            cover_path: None,
        }],
    );
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-1".into()),
        mbv_audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-1".into(),
            current_time_seconds: 90.0,
            is_finished: true,
        },
    );

    let item = app
        .selected_audiobookshelf_queue_item_target(
            0,
            &mbv_ui_msg::PodcastEpisodeTarget::new("show-a".into(), "episode-1".into()),
        )
        .unwrap();
    let item = item.as_audiobookshelf().unwrap();

    assert_eq!(item.position_ticks, 90 * mbv_emby_model::TICKS_PER_SECOND);
    assert!(item.played);
}

#[test]
fn podcast_episode_targets_include_parent_show_identity() {
    let mut app = super::podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].detail_cache.insert(
        "show-a".into(),
        vec![mbv_audiobookshelf::AudiobookshelfDownloadedEpisode {
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
        vec![mbv_audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-b".into(),
            episode_id: "episode-1".into(),
            title: "B".into(),
            description: None,
            published_at: None,
            duration_seconds: Some(1.0),
        }],
    );
    let first = mbv_ui_msg::PodcastEpisodeTarget::new("show-a".into(), "episode-1".into());
    let second = mbv_ui_msg::PodcastEpisodeTarget::new("show-b".into(), "episode-1".into());
    let first_item = app
        .selected_audiobookshelf_queue_item_target(0, &first)
        .unwrap();
    let second_item = app
        .selected_audiobookshelf_queue_item_target(0, &second)
        .unwrap();
    assert_eq!(
        first_item.as_audiobookshelf().unwrap().library_item_id,
        "show-a"
    );
    assert_eq!(
        second_item.as_audiobookshelf().unwrap().library_item_id,
        "show-b"
    );
    // The parent-show metadata follows each episode's own `library_item_id`,
    // never the tab's current selection (task 2.3): the tab's selection is
    // "show-a", yet the show-b episode does not inherit its metadata.
    assert_eq!(
        first_item
            .as_audiobookshelf()
            .unwrap()
            .show_title
            .as_deref(),
        Some("Show A")
    );
    assert_eq!(second_item.as_audiobookshelf().unwrap().show_title, None);
}
