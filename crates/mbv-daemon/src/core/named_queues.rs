//! The Local Owner's Named queues (gui-named-queues design D1).
//!
//! The Owner keeps its existing Bound queue, run, transitions, consume, and
//! lineage machinery for the **current queue** without change. Every other
//! Named queue rests in this store: it keeps its slots, its name, its source,
//! its lineage, and its start slot, and it has no run, so no completion,
//! consume, or progress observation reaches it (spec `named-queues` "Only the
//! current queue plays").

use mbv_queue::{NamedQueueId, PlaybackQueue, QueueLineage, QueueSlotId, QueueSource};

/// The name a fresh Owner gives its first queue (spec `named-queues` "A first
/// start makes one queue"). The `mbv-config` conversion of an earlier
/// single-queue state file gives its queue the same name.
pub(crate) const FIRST_QUEUE_NAME: &str = mbv_config::DEFAULT_NAMED_QUEUE_NAME;

/// The raw identity a fresh Owner mints for its first queue. The `mbv-config`
/// conversion of an earlier single-queue state file mints the same identity for
/// the `Default` queue it produces, so both paths start the persisted identity
/// sequence at the same value.
pub(crate) const FIRST_QUEUE_RAW_ID: u64 = 1;

/// One Named queue that rests while another queue is current.
#[derive(Debug, Clone)]
pub struct RestingNamedQueue {
    /// The queue's stable identity, unchanged by a rename or a restart. Slot
    /// identities are unique within the queue (spec `named-queues` "The Owner
    /// holds named queues").
    pub id: NamedQueueId,
    /// The user-visible name. Two queues may share one.
    pub name: String,
    /// The queue's ordered slots, with their stable identities.
    pub queue: PlaybackQueue,
    /// The queue's own Queue source.
    pub source: QueueSource,
    /// The queue's own lineage.
    pub lineage: QueueLineage,
    /// The slot a Play of this queue starts, or `None` when it starts at its
    /// first slot. A start slot names a slot and never stores a position: the
    /// item that plays from it takes its position from its Resume source
    /// (design D4).
    pub start_slot: Option<QueueSlotId>,
}

/// The Named queues the Owner holds besides the current queue's content:
/// which queue is current, which identities are taken, and the resting queues
/// in queue order.
///
/// The current queue's slots, source, lineage, and run stay where they already
/// live — the Owner's Bound queue (`DaemonPlayerOwner::core`). The store holds
/// the current queue's identity and name only, so no queue has two owners
/// (design D1).
#[derive(Debug, Clone)]
pub struct NamedQueueStore {
    /// The current queue's identity: a stored identity, never a position in
    /// `resting`.
    pub current: NamedQueueId,
    /// The current queue's user-visible name.
    pub current_name: String,
    /// The identity the next new queue takes. Identities are never reused
    /// within one state file (design D3).
    pub next_queue_id: NamedQueueId,
    /// The resting queues, in queue order (creation order).
    pub resting: Vec<RestingNamedQueue>,
}

impl NamedQueueStore {
    /// The store a fresh Local Owner starts with: one current queue named
    /// `Default`, no resting queue, and the next identity above the first
    /// (spec `named-queues` "A first start makes one queue").
    #[must_use]
    pub fn first_start() -> Self {
        Self {
            current: NamedQueueId::from_raw(FIRST_QUEUE_RAW_ID),
            current_name: FIRST_QUEUE_NAME.to_string(),
            next_queue_id: NamedQueueId::from_raw(FIRST_QUEUE_RAW_ID.saturating_add(1)),
            resting: Vec::new(),
        }
    }
}
