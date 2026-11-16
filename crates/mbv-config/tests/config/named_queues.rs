//! Named-queue state persistence (design D6 of `gui-named-queues`): several
//! queues round-trip with their names, sources, slot identities, and start
//! slots, and the earlier single-queue owner file loads as one current queue
//! named `Default`.

use std::path::PathBuf;

use mbv_config::{
    NamedQueue, NamedQueueSlot, NamedQueuesState, StayAliveQueueState, load_named_queues_state_at,
    load_or_migrate_named_queues_state_at, save_named_queues_state_at,
    save_stay_alive_queue_state_at,
};
use mbv_queue::{
    FeedEntry, FeedKind, NamedQueueId, QueueItem, QueueLineage, QueueSlotId, QueueSource,
    QueueState,
};

/// A unique scratch directory that removes itself on drop, so a failing test
/// leaves nothing behind. No test here reads a real state directory.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("mbv-named-queues-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn feed_item(guid: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.to_string(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.com/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".to_string()),
        duration_ticks: Some(1_800),
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    })
}

fn two_queue_state() -> NamedQueuesState {
    NamedQueuesState {
        current: NamedQueueId::from_raw(1),
        next_queue_id: NamedQueueId::from_raw(3),
        queues: vec![
            NamedQueue {
                id: NamedQueueId::from_raw(1),
                name: "Default".to_string(),
                source: QueueSource::Playlist {
                    id: None,
                    name: "Morning".to_string(),
                },
                lineage: QueueLineage(7),
                start_slot: Some(QueueSlotId::from_raw(2)),
                slots: vec![
                    NamedQueueSlot {
                        slot_id: QueueSlotId::from_raw(1),
                        item: feed_item("a"),
                    },
                    NamedQueueSlot {
                        slot_id: QueueSlotId::from_raw(2),
                        item: feed_item("b"),
                    },
                ],
            },
            NamedQueue {
                id: NamedQueueId::from_raw(2),
                name: "Kids".to_string(),
                source: QueueSource::Album,
                lineage: QueueLineage(9),
                start_slot: None,
                slots: vec![NamedQueueSlot {
                    slot_id: QueueSlotId::from_raw(3),
                    item: feed_item("c"),
                }],
            },
        ],
    }
}

fn single_queue_state() -> StayAliveQueueState {
    StayAliveQueueState {
        queue: QueueState {
            source: QueueSource::Album,
            items: vec![feed_item("a"), feed_item("b"), feed_item("c")],
            cursor: 2,
            last_played_content_id: None,
            last_played_item_id: None,
            last_played_completed: false,
            positions: std::collections::HashMap::default(),
        },
        lineage: QueueLineage(4),
    }
}

fn slot_ids(queue: &NamedQueue) -> Vec<u64> {
    queue.slots.iter().map(|slot| slot.slot_id.raw()).collect()
}

fn item_ids(queue: &NamedQueue) -> Vec<&str> {
    queue.slots.iter().map(|slot| slot.item.id()).collect()
}

#[test]
fn two_named_queues_round_trip_with_their_slot_ids_and_start_slots() {
    let scratch = ScratchDir::new("round-trip");
    let path = scratch.join("named_queues_state.json");
    let state = two_queue_state();

    save_named_queues_state_at(&path, &state).unwrap();
    let loaded = load_named_queues_state_at(&path).unwrap();

    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        serde_json::to_value(&state).unwrap()
    );
}

#[test]
fn an_earlier_single_queue_state_loads_as_one_current_default_queue() {
    let scratch = ScratchDir::new("migration");
    let stay_alive_path = scratch.join("stay_alive_queue_state.json");
    let named_queues_path = scratch.join("named_queues_state.json");
    save_stay_alive_queue_state_at(&stay_alive_path, &single_queue_state()).unwrap();
    let before = std::fs::read(&stay_alive_path).unwrap();

    let state =
        load_or_migrate_named_queues_state_at(&named_queues_path, &stay_alive_path).unwrap();

    assert_eq!(std::fs::read(&stay_alive_path).unwrap(), before);
    assert_eq!(state.queues.len(), 1);
    let queue = state.queues.first().unwrap();
    assert_eq!(queue.id, state.current);
    assert_eq!(queue.name, "Default");
    assert_eq!(queue.source, QueueSource::Album);
    assert_eq!(queue.lineage, QueueLineage(4));
    assert_eq!(slot_ids(queue), vec![1, 2, 3]);
    assert_eq!(item_ids(queue), vec!["a", "b", "c"]);
    assert_eq!(queue.start_slot, Some(QueueSlotId::from_raw(3)));
}
