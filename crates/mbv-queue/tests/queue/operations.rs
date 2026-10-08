use super::*;

// PlaybackQueue operation tests (task 1.2)

#[test]
fn replace_clears_queue_and_sets_new_items() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let initial = queue.revision();
    let old_active = queue.replace(vec![
        QueueItem::Emby(Box::new(item("x"))),
        QueueItem::Feed(feed("f1")),
        QueueItem::Emby(Box::new(item("y"))),
    ]);

    assert_eq!(old_active, Some(0));
    assert_eq!(queue.len(), 3);
    assert_eq!(queue.slots()[0].item.id(), "x");
    assert_eq!(queue.slots()[1].item.id(), "f1");
    assert_eq!(queue.slots()[2].item.id(), "y");
    assert_eq!(queue.active_slot_id(), None);
    assert!(queue.revision() > initial);
}

#[test]
fn replace_with_empty_vec_clears() {
    let mut queue = queue_from_items(vec![item("a")], Some(0));
    let old_active = queue.replace(vec![]);

    assert_eq!(old_active, Some(0));
    assert!(queue.is_empty());
    assert_eq!(queue.active_slot_id(), None);
}

#[test]
fn clear_removes_all_slots_and_bumps_revision() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(1));
    let initial = queue.revision();
    queue.clear();

    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.active_slot_id(), None);
    assert!(queue.revision() > initial);
}

#[test]
fn clear_on_empty_queue_is_noop() {
    let mut queue = empty_queue();
    let before = queue.revision();
    queue.clear();

    assert!(queue.is_empty());
    assert_eq!(queue.revision(), before);
}

#[test]
fn len_and_active_index_reflect_queue_state() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(1));

    assert_eq!(queue.len(), 3);
    assert_eq!(queue.active_index(), Some(1));

    let target = queue.slots()[2].slot_id;
    queue.set_active_slot(target);
    assert_eq!(queue.active_index(), Some(2));

    queue.clear_active_slot();
    assert_eq!(queue.active_index(), None);
}

#[test]
fn mixed_queue_replace_preserves_item_variants() {
    let mut queue = empty_queue();
    queue.replace(vec![
        QueueItem::Feed(feed("f1")),
        QueueItem::Emby(Box::new(item("e1"))),
        QueueItem::Feed(feed("f2")),
    ]);

    assert!(matches!(queue.slots()[0].item, QueueItem::Feed(_)));
    assert!(matches!(queue.slots()[1].item, QueueItem::Emby(_)));
    assert!(matches!(queue.slots()[2].item, QueueItem::Feed(_)));
}

// is_music — the fire-and-forget predicate row visuals and resume share

#[rstest::rstest]
#[case::audio_track("Audio", "Audio", true)]
#[case::music_album("MusicAlbum", "Audio", true)]
#[case::music_artist("MusicArtist", "Audio", true)]
#[case::movie("Movie", "Video", false)]
#[case::episode("Episode", "Video", false)]
fn is_music_covers_emby_music_types(
    #[case] item_type: &str,
    #[case] media_type: &str,
    #[case] expected: bool,
) {
    let item = emby_item_of_type("m", item_type, media_type, "M");
    assert_eq!(QueueItem::Emby(Box::new(item)).is_music(), expected);
}

#[test]
fn is_music_excludes_feeds_and_audiobookshelf_however_audio_they_are() {
    assert!(!QueueItem::Feed(feed("f1")).is_music());
    assert!(
        !QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(audiobookshelf_episode(
            "lib1", "ep1"
        )))
        .is_music()
    );
    assert!(
        !QueueItem::Audiobookshelf(AudiobookshelfItem::Book(audiobookshelf_book("lib1")))
            .is_music()
    );
}

#[rstest::rstest]
#[case::completed_below_floor(ProgressObservation::Completed { position_ticks: 29 * TICKS_PER_SECOND, played: false }, "Video", 7 * TICKS_PER_SECOND, 7 * TICKS_PER_SECOND, false)]
#[case::completed_at_floor(ProgressObservation::Completed { position_ticks: 30 * TICKS_PER_SECOND, played: false }, "Video", 7 * TICKS_PER_SECOND, 30 * TICKS_PER_SECOND, false)]
#[case::completed_audio(ProgressObservation::Completed { position_ticks: 40 * TICKS_PER_SECOND, played: false }, "Audio", 7 * TICKS_PER_SECOND, 7 * TICKS_PER_SECOND, false)]
#[case::completed_played(ProgressObservation::Completed { position_ticks: 40 * TICKS_PER_SECOND, played: true }, "Video", 7 * TICKS_PER_SECOND, 0, true)]
#[case::stopped_positive(ProgressObservation::Stopped { position_ticks: 12 * TICKS_PER_SECOND, played: false }, "Video", 7 * TICKS_PER_SECOND, 12 * TICKS_PER_SECOND, false)]
#[case::stopped_zero(ProgressObservation::Stopped { position_ticks: 0, played: false }, "Video", 7 * TICKS_PER_SECOND, 7 * TICKS_PER_SECOND, false)]
#[case::stopped_audio(ProgressObservation::Stopped { position_ticks: 12 * TICKS_PER_SECOND, played: false }, "Audio", 7 * TICKS_PER_SECOND, 7 * TICKS_PER_SECOND, false)]
#[case::stopped_played(ProgressObservation::Stopped { position_ticks: 12 * TICKS_PER_SECOND, played: true }, "Video", 7 * TICKS_PER_SECOND, 0, true)]
fn progress_observation_records_expected_position(
    #[case] observation: ProgressObservation,
    #[case] media_type: &str,
    #[case] previous: i64,
    #[case] expected_position: i64,
    #[case] expected_played: bool,
) {
    let mut emby = item("progress");
    emby.media_type = media_type.to_owned();
    emby.playback_position_ticks = previous;
    let queue_item = QueueItem::Emby(Box::new(emby));
    assert_eq!(
        observation.position_to_record(&queue_item),
        expected_position
    );
    assert_eq!(observation.played(), expected_played);
}
