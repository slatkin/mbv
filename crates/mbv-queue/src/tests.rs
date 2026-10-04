use super::*;
use mbv_emby_model::{EmbyImageTags, EmbyItem, TICKS_PER_SECOND};
use title_parts::emby_item_of_type;

fn item(id: &str) -> EmbyItem {
    EmbyItem {
        id: id.to_string(),
        name: format!("Item {id}"),
        item_type: "Episode".to_string(),
        is_folder: false,
        child_count: None,
        media_type: "Video".to_string(),
        collection_type: String::new(),
        runtime_ticks: 30 * TICKS_PER_SECOND,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        studios: Vec::new(),
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: EmbyImageTags::default(),
    }
}

fn revision_mint() -> std::sync::Arc<QueueRevisionMint> {
    std::sync::Arc::new(QueueRevisionMint::default())
}

fn queue_from_items(items: Vec<EmbyItem>, active: Option<usize>) -> PlaybackQueue {
    PlaybackQueue::from_items(items, active, revision_mint())
}

fn queue_from_queue_items(items: Vec<QueueItem>, active: Option<usize>) -> PlaybackQueue {
    PlaybackQueue::from_queue_items(items, active, revision_mint())
}

fn queue_from_slot_items(
    slots: Vec<(QueueSlotId, QueueItem)>,
    active: Option<QueueSlotId>,
) -> PlaybackQueue {
    PlaybackQueue::from_slot_items(slots, active, revision_mint())
}

fn empty_queue() -> PlaybackQueue {
    PlaybackQueue::from_queue_items(Vec::new(), None, revision_mint())
}

#[rstest::rstest]
#[case::not_started(EpisodeResume::NOT_STARTED, 0, false)]
#[case::resumed(EpisodeResume::from_seconds(90.0, true), 90 * TICKS_PER_SECOND, true)]
fn from_catalog_applies_resume_state(
    #[case] resume: EpisodeResume,
    #[case] position_ticks: i64,
    #[case] finished: bool,
) {
    let item = AudiobookshelfQueueItem::from_catalog(
        AudiobookshelfEpisodeCatalog {
            library_item_id: "show".into(),
            episode_id: "episode".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            pub_date_secs: None,
            cover_path: None,
        },
        resume,
    );

    assert_eq!(item.position_ticks, position_ticks);
    assert_eq!(item.played, finished);
    assert_eq!(item.is_finished, finished);
}

fn audiobookshelf_episode(library_item_id: &str, episode_id: &str) -> AudiobookshelfQueueItem {
    AudiobookshelfQueueItem {
        library_item_id: library_item_id.into(),
        episode_id: episode_id.into(),
        title: "ABS episode".into(),
        show_title: Some("Show".into()),
        author: Some("Author".into()),
        description: None,
        duration_ticks: Some(120 * TICKS_PER_SECOND as u64),
        position_ticks: 30 * TICKS_PER_SECOND,
        played: false,
        pub_date_secs: Some(1_700_000_000),
        is_finished: false,
        cover_path: Some("/covers/show.jpg".into()),
    }
}

fn audiobookshelf_book(library_item_id: &str) -> AudiobookshelfBookQueueItem {
    AudiobookshelfBookQueueItem {
        library_item_id: library_item_id.into(),
        title: "ABS book".into(),
        author: Some("Author".into()),
        duration_ticks: Some(3600 * TICKS_PER_SECOND as u64),
        position_ticks: 900 * TICKS_PER_SECOND,
        played: false,
        is_finished: false,
        cover_path: Some("/covers/book.jpg".into()),
    }
}

#[test]
fn mpv_url_source_covers_each_queue_item_shape() {
    let emby = QueueItem::Emby(Box::new(item("emby1")));
    let feed = QueueItem::Feed(FeedEntry {
        guid: "feed1".into(),
        title: "Feed entry".into(),
        enclosure_url: Some("https://example.com/audio.mp3".into()),
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    });
    let episode = QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(audiobookshelf_episode(
        "lib1", "ep1",
    )));
    let book = QueueItem::Audiobookshelf(AudiobookshelfItem::Book(audiobookshelf_book("lib1")));

    assert!(emby.mpv_url_source().is_some());
    assert!(feed.mpv_url_source().is_some());
    assert!(episode.mpv_url_source().is_none());
    assert!(book.mpv_url_source().is_none());
}

fn item_with_progress(id: &str, position_seconds: i64, played: bool) -> EmbyItem {
    let mut item = item(id);
    item.playback_position_ticks = position_seconds * TICKS_PER_SECOND;
    item.played = played;
    item
}

fn slot_ids(queue: &PlaybackQueue) -> Vec<QueueSlotId> {
    queue.slots().iter().map(|slot| slot.slot_id).collect()
}

#[test]
fn duplicate_item_ids_receive_distinct_queue_slot_ids() {
    let queue = queue_from_items(vec![item("same"), item("same")], Some(0));

    assert_ne!(queue.slots()[0].slot_id, queue.slots()[1].slot_id);
    assert_eq!(queue.slots()[0].item.id(), queue.slots()[1].item.id());
}

#[test]
fn owner_assigned_dup_items_keep_distinct_slot_ids_through_submit_and_append() {
    // Submission hands the run owner-assigned (id, item) pairs; two copies of
    // the same content must land in two independently addressable slots.
    let submitted = vec![
        (
            QueueSlotId::from_raw(10),
            QueueItem::Emby(Box::new(item("dup"))),
        ),
        (
            QueueSlotId::from_raw(11),
            QueueItem::Emby(Box::new(item("dup"))),
        ),
    ];
    let mut queue = queue_from_slot_items(submitted, Some(QueueSlotId::from_raw(10)));

    // Then append the same content twice more with fresh owner ids.
    queue.append_with_id(
        QueueSlotId::from_raw(12),
        QueueItem::Emby(Box::new(item("dup"))),
    );
    queue.append_with_id(
        QueueSlotId::from_raw(13),
        QueueItem::Emby(Box::new(item("dup"))),
    );

    let ids = slot_ids(&queue);
    assert_eq!(
        ids,
        vec![
            QueueSlotId::from_raw(10),
            QueueSlotId::from_raw(11),
            QueueSlotId::from_raw(12),
            QueueSlotId::from_raw(13),
        ]
    );
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 4, "every occurrence keeps a distinct slot id");
    assert!(queue.slots().iter().all(|s| s.item.id() == "dup"));

    // A subsequent local allocation must not reuse an adopted id.
    let minted = queue.append(QueueItem::Emby(Box::new(item("dup"))));
    assert!(minted.raw() > 13);
}

#[test]
fn from_queue_items_next_local_allocation_is_len_plus_one() {
    // A newly constructed canonical queue allocates slot ids starting at one;
    // this locks the assumption that its first three slots use ids 1..=3.
    let mut queue = queue_from_queue_items(
        vec![item("a"), item("b"), item("c")]
            .into_iter()
            .map(|i| QueueItem::Emby(Box::new(i)))
            .collect(),
        Some(0),
    );
    assert_eq!(queue.append(QueueItem::Emby(Box::new(item("d")))).raw(), 4);
}

#[test]
fn removing_before_active_slot_preserves_active_identity() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(2));
    let active = queue.active_slot_id().unwrap();
    let before_active = queue.slots()[0].slot_id;

    assert!(matches!(
        queue.remove_slot(before_active),
        RemoveSlotResult::Removed(_)
    ));

    assert_eq!(queue.active_slot_id(), Some(active));
    assert_eq!(queue.slot_index(active), Some(1));
}

#[test]
fn moving_slots_around_active_slot_preserves_active_identity() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(1));
    let ids = slot_ids(&queue);
    let active = queue.active_slot_id().unwrap();

    assert!(matches!(
        queue.move_slot(ids[0], 2),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.active_slot_id(), Some(active));
    assert_eq!(queue.slot_index(active), Some(0));

    assert!(matches!(
        queue.move_slot(ids[2], 0),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.active_slot_id(), Some(active));
    assert_eq!(queue.slot_index(active), Some(1));
}

#[test]
fn moving_active_slot_keeps_active_identity_on_that_slot() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(1));
    let active = queue.active_slot_id().unwrap();

    assert!(matches!(
        queue.move_slot(active, 0),
        QueueMutationResult::Applied(())
    ));

    assert_eq!(queue.active_slot_id(), Some(active));
    assert_eq!(queue.slot_index(active), Some(0));
}

#[test]
fn set_active_slot_targets_slot_after_reorder() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let target = queue.slots()[2].slot_id;

    assert!(matches!(
        queue.move_slot(target, 0),
        QueueMutationResult::Applied(())
    ));
    assert!(matches!(
        queue.set_active_slot(target),
        QueueMutationResult::Applied(())
    ));

    assert_eq!(queue.active_slot_id(), Some(target));
    assert_eq!(queue.slot_index(target), Some(0));
}

#[test]
fn consume_removes_intended_slot_occurrence() {
    let mut queue = queue_from_items(vec![item("same"), item("same"), item("c")], Some(2));
    let consumed = queue.slots()[1].slot_id;

    let QueueMutationResult::Applied(slot) = queue.consume_slot(consumed) else {
        panic!("expected consume to remove the slot");
    };

    assert_eq!(slot.slot_id, consumed);
    assert!(queue.slot(consumed).is_none());
    assert_eq!(queue.slots().len(), 2);
    assert_eq!(queue.slots()[0].item.id(), "same");
}

#[test]
fn structural_mutations_bump_revision() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let initial = queue.revision();

    let inserted = queue.append(QueueItem::Emby(Box::new(item("c"))));
    assert!(queue.revision() > initial);
    let after_insert = queue.revision();

    assert!(matches!(
        queue.move_slot(inserted, 0),
        QueueMutationResult::Applied(())
    ));
    assert!(queue.revision() > after_insert);
    let after_move = queue.revision();

    assert!(matches!(
        queue.consume_slot(inserted),
        QueueMutationResult::Applied(_)
    ));
    assert!(queue.revision() > after_move);
}

#[test]
fn active_slot_removal_requires_confirmation_decision() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let active = queue.active_slot_id().unwrap();

    assert!(matches!(
        queue.remove_slot(active),
        RemoveSlotResult::RequiresActiveConfirmation(slot_id) if slot_id == active
    ));
    assert!(queue.slot(active).is_some());
}

#[test]
fn confirmed_active_slot_removal_clears_active_identity() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let active = queue.active_slot_id().unwrap();

    assert!(matches!(
        queue.remove_active_slot_confirmed(active),
        RemoveSlotResult::Removed(_)
    ));

    assert!(queue.slot(active).is_none());
    assert_eq!(queue.active_slot_id(), None);
}

#[test]
fn projected_row_mutation_matrix_tracks_revision_without_noop_bumps() {
    let mut queue = queue_from_queue_items(
        vec![
            QueueItem::Emby(Box::new(item("a"))),
            QueueItem::Emby(Box::new(item("b"))),
        ],
        Some(0),
    );
    let first = queue.slots()[0].slot_id;
    let second = queue.slots()[1].slot_id;

    let before = queue.revision();
    assert!(matches!(
        queue.apply_progress(first, 0, false),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), before);

    assert!(matches!(
        queue.apply_progress(first, TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    let after_progress = queue.revision();
    assert!(after_progress > before);

    assert!(matches!(
        queue.update_slot_item(
            first,
            QueueItem::Emby(Box::new(item_with_progress("a", 1, false,)))
        ),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), after_progress);

    assert!(matches!(
        queue.set_active_slot(second),
        QueueMutationResult::Applied(())
    ));
    let after_active = queue.revision();
    assert!(after_active > after_progress);
    assert!(matches!(
        queue.set_active_slot(second),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), after_active);

    assert!(matches!(
        queue.apply_progress(first, TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), after_active);

    let before_refresh = queue.revision();
    let result = queue.merge_refresh(vec![item_with_progress("a", 1, false), item("b")]);
    assert_eq!(result.pruned_slots, [] as [QueueSlotId; 0]);
    assert_eq!(queue.revision(), before_refresh);

    queue.clear_active_slot();
    assert!(queue.revision() > before_refresh);
}

mod persistence;
mod progress;
mod revision;
fn feed(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.to_string(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.com/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(60 * TICKS_PER_SECOND as u64),
        pub_date_secs: None,
        feed_kind: Some(crate::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

#[test]
fn feed_slot_participates_in_queue_ordering_and_survives_refresh() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let feed_slot = queue.append(QueueItem::Feed(feed("f1")));

    // The Feed slot holds its own identity alongside the Emby slots.
    assert_eq!(queue.slots().last().unwrap().slot_id, feed_slot);
    assert_eq!(queue.slots().last().unwrap().item.id(), "f1");

    assert!(matches!(
        queue.set_active_slot(feed_slot),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.active_slot_id(), Some(feed_slot));

    assert!(matches!(
        queue.move_slot(feed_slot, 0),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.slots()[0].slot_id, feed_slot);

    // Feed slots have no server-side counterpart; a refresh must leave
    // them in place rather than pruning them.
    let result = queue.merge_refresh(vec![item("a"), item("b")]);
    assert_eq!(result.pruned_slots, [] as [QueueSlotId; 0]);
    assert!(queue.slot(feed_slot).is_some());
    assert!(matches!(
        queue.slot(feed_slot).unwrap().item,
        QueueItem::Feed(_)
    ));
}

mod feed;
mod operations;
mod title_parts;
