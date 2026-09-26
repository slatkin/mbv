#[test]
fn podcast_episode_targets_include_parent_show_identity() {
    let mut app = super::podcast::audiobookshelf_app();
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
    let first =
        crate::app::components::msg::PodcastEpisodeTarget::new("show-a".into(), "episode-1".into());
    let second =
        crate::app::components::msg::PodcastEpisodeTarget::new("show-b".into(), "episode-1".into());
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
