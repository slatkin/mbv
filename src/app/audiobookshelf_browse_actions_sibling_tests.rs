#[test]
fn podcast_episode_targets_include_parent_show_identity() {
    let mut app = super::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episodes = Some(vec![
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-1".into(),
            title: "A".into(),
            published_at: None,
            duration_seconds: Some(1.0),
        },
        mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
            library_item_id: "show-b".into(),
            episode_id: "episode-1".into(),
            title: "B".into(),
            published_at: None,
            duration_seconds: Some(1.0),
        },
    ]);
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
}
