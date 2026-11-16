// Every Named queue the local Owner holds, in one file: each queue's name,
// source, lineage, start slot, and slots with their stable identities (design
// D6 of `gui-named-queues`). An earlier single-queue owner file loads as one
// current queue named `Default`, and the earlier file is left as it was.

use std::path::Path;

use mbv_queue::{NamedQueueId, QueueItem, QueueLineage, QueueSlotId, QueueSource, QueueState};

use crate::state::{load_json, load_stay_alive_queue_state_at, save_json_atomic};
use crate::{
    ConfigError, StayAliveQueueState, named_queues_state_path, stay_alive_queue_state_path,
};

/// The name the queue converted from the earlier single-queue file takes, and
/// the name of the first queue a fresh Owner holds.
pub const DEFAULT_NAMED_QUEUE_NAME: &str = "Default";

/// The raw identity the converted `Default` queue takes. A fresh Owner mints
/// the same first identity, so both paths name their first queue alike.
pub const FIRST_QUEUE_RAW_ID: u64 = 1;

/// One Named queue as it is persisted: its slots with their stable identities,
/// its name, its source, its lineage, and its start slot.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NamedQueue {
    pub id: NamedQueueId,
    pub name: String,
    pub source: QueueSource,
    pub lineage: QueueLineage,
    /// Absent when the queue has no start slot: Play starts its first slot.
    #[serde(default)]
    pub start_slot: Option<QueueSlotId>,
    #[serde(default)]
    pub slots: Vec<NamedQueueSlot>,
}

/// One persisted slot: its owner-assigned identity and its media snapshot.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NamedQueueSlot {
    pub slot_id: QueueSlotId,
    pub item: QueueItem,
}

/// Every Named queue the local Owner holds, and which one is current.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NamedQueuesState {
    pub current: NamedQueueId,
    pub next_queue_id: NamedQueueId,
    pub queues: Vec<NamedQueue>,
}

impl NamedQueuesState {
    /// This state with every Service-owned resume position cleared, as
    /// `save_stay_alive_queue_state_at` clears the owner queue.
    #[must_use]
    pub fn without_service_positions(&self) -> Self {
        Self {
            current: self.current,
            next_queue_id: self.next_queue_id,
            queues: self
                .queues
                .iter()
                .map(NamedQueue::without_service_positions)
                .collect(),
        }
    }
}

impl NamedQueue {
    fn without_service_positions(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            source: self.source.clone(),
            lineage: self.lineage,
            start_slot: self.start_slot,
            slots: self
                .slots
                .iter()
                .map(|slot| {
                    let mut item = slot.item.clone();
                    item.clear_service_positions();
                    NamedQueueSlot {
                        slot_id: slot.slot_id,
                        item,
                    }
                })
                .collect(),
        }
    }
}

pub fn save_named_queues_state_at(
    path: &Path,
    state: &NamedQueuesState,
) -> Result<(), ConfigError> {
    save_json_atomic(path, &state.without_service_positions(), "named queues")
}

pub fn save_named_queues_state(state: &NamedQueuesState) -> Result<(), ConfigError> {
    save_named_queues_state_at(&named_queues_state_path(), state)
}

#[must_use]
pub fn load_named_queues_state_at(path: &Path) -> Option<NamedQueuesState> {
    let state: NamedQueuesState = load_json(path, "named queues state")?;
    Some(state.without_service_positions())
}

#[must_use]
pub fn load_named_queues_state() -> Option<NamedQueuesState> {
    load_named_queues_state_at(&named_queues_state_path())
}

/// The Named-queue state to restore at startup: the new state file when it
/// exists, else one current queue named `Default` converted from the earlier
/// single-queue file.
///
/// The conversion reads only. It leaves the earlier file as it was, and the
/// Owner's next save writes the new one.
#[must_use]
pub fn load_or_migrate_named_queues_state_at(
    named_queues_path: &Path,
    stay_alive_path: &Path,
) -> Option<NamedQueuesState> {
    if named_queues_path.exists() {
        return load_named_queues_state_at(named_queues_path);
    }
    load_stay_alive_queue_state_at(stay_alive_path).map(converted_named_queues_state)
}

#[must_use]
pub fn load_or_migrate_named_queues_state() -> Option<NamedQueuesState> {
    load_or_migrate_named_queues_state_at(
        &named_queues_state_path(),
        &stay_alive_queue_state_path(),
    )
}

/// An earlier single-queue state as one current queue named `Default`. Its
/// slots take fresh identities in queue order, and the position it saved
/// becomes the start slot, so Play resumes where the earlier Owner left off.
#[must_use]
pub fn converted_named_queues_state(stay_alive: StayAliveQueueState) -> NamedQueuesState {
    let StayAliveQueueState { queue, lineage } = stay_alive;
    let QueueState {
        source,
        items,
        cursor,
        ..
    } = queue;
    let mut next_slot_id = 1_u64;
    let mut slots = Vec::with_capacity(items.len());
    for item in items {
        slots.push(NamedQueueSlot {
            slot_id: QueueSlotId::from_raw(next_slot_id),
            item,
        });
        next_slot_id = next_slot_id.saturating_add(1);
    }
    let start_slot = slots.get(cursor).map(|slot| slot.slot_id);
    NamedQueuesState {
        current: NamedQueueId::from_raw(FIRST_QUEUE_RAW_ID),
        next_queue_id: NamedQueueId::from_raw(FIRST_QUEUE_RAW_ID.saturating_add(1)),
        queues: vec![NamedQueue {
            id: NamedQueueId::from_raw(FIRST_QUEUE_RAW_ID),
            name: DEFAULT_NAMED_QUEUE_NAME.to_string(),
            source,
            lineage,
            start_slot,
            slots,
        }],
    }
}
