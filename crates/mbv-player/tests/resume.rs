//! The public resume-decision contract for a queue replacement's start slot
//! (queue-handoff-and-take design D4): the carried start position is a
//! one-time override for that slot, ungated by the one-percent rule, and
//! every later start keeps its per-medium source.

use mbv_emby_model::{TICKS_PER_SECOND, test_support};
use mbv_player::resume_ticks_for_replace_start;
use mbv_queue::{FeedEntry, PlaybackQueue, QueueItem, QueueRevisionMint};

/// An Emby audio track: its per-medium source is the beginning, so only the
/// carried override can start it mid-way.
fn audio_track() -> QueueItem {
    let mut item = test_support::make_item("Song", "Audio");
    item.media_type = "Audio".into();
    item.runtime_ticks = 180 * TICKS_PER_SECOND;
    QueueItem::Emby(Box::new(item))
}

/// A feed entry parked mid-way: its per-medium source is its locally recorded
/// position, which qualifies under the one-percent rule.
fn feed_entry() -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: "feed-1".into(),
        title: "Episode".into(),
        enclosure_url: Some("https://feed/a.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(u64::try_from(3600 * TICKS_PER_SECOND).expect("positive ticks")),
        pub_date_secs: None,
        feed_kind: None,
        feed_id: Some("feed".into()),
        position_ticks: 1800 * TICKS_PER_SECOND,
        played: false,
    })
}

/// Contract: the start slot of a replacement that carries a start position
/// starts at the carried position — for an Emby audio track too, whose
/// per-medium source would always start from the beginning — and the next
/// slot still starts from its per-medium source (design D4).
#[test]
fn the_carried_position_starts_the_handoff_slot_and_the_next_slot_keeps_its_own_source() {
    let mut queue = PlaybackQueue::from_queue_items(
        Vec::new(),
        None,
        std::sync::Arc::new(QueueRevisionMint::default()),
    );
    let audio_slot = queue.append(audio_track());
    let feed_slot = queue.append(feed_entry());

    assert_eq!(
        resume_ticks_for_replace_start(&queue, audio_slot, Some(90.0)),
        Some(90 * TICKS_PER_SECOND),
        "the carried position starts the start slot, ungated"
    );
    assert_eq!(
        resume_ticks_for_replace_start(&queue, feed_slot, None),
        Some(1800 * TICKS_PER_SECOND),
        "the next slot uses its per-medium source"
    );
}
