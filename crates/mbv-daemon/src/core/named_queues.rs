//! The Local Owner's Named queues (gui-named-queues design D1).
//!
//! The Owner keeps its existing Bound queue, run, transitions, consume, and
//! lineage machinery for the **current queue** without change. Every other
//! Named queue rests in this store: it keeps its slots, its name, its source,
//! its lineage, and its start slot, and it has no run, so no completion,
//! consume, or progress observation reaches it (spec `named-queues` "Only the
//! current queue plays").

use mbv_queue::{
    NamedQueueId, PlaybackQueue, QueueLineage, QueueRevisionMint, QueueSlotId, QueueSource,
    StartSlotObservation,
};
use std::sync::Arc;

use super::{DaemonPlayerOwner, SharedQueueState};

/// The name a fresh Owner gives its first queue (spec `named-queues` "A first
/// start makes one queue"). The `mbv-config` conversion of an earlier
/// single-queue state file gives its queue the same name.
pub(crate) const FIRST_QUEUE_NAME: &str = mbv_config::DEFAULT_NAMED_QUEUE_NAME;

/// The raw identity a fresh Owner mints for its first queue. The `mbv-config`
/// conversion of an earlier single-queue state file mints the same identity for
/// the `Default` queue it produces, so both paths start the persisted identity
/// sequence at the same value.
pub(crate) const FIRST_QUEUE_RAW_ID: u64 = mbv_config::FIRST_QUEUE_RAW_ID;

/// The lowest number a new queue's name draws from: the first new queue beside
/// `Default` is `Queue 2` (design D6).
pub(crate) const FIRST_NEW_QUEUE_NUMBER: u64 = 2;

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

impl RestingNamedQueue {
    /// The one write site for a resting queue's start slot (design D4), the
    /// mirror of `SharedQueueState::note_start_slot` for the current queue. A
    /// resting queue sees no run, so the only observation that reaches it is
    /// the removal of the slot it names.
    pub fn note_start_slot(&mut self, observation: StartSlotObservation) {
        self.start_slot = mbv_queue::start_slot_after(&self.queue, self.start_slot, observation);
    }
}

/// The queue one addressed command reaches (design D5 of `gui-named-queues`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedQueueTarget {
    /// The current queue, which names no identity or the current identity.
    /// Its slots live on the Owner core, not in this store.
    Current,
    /// A resting queue, at this index in the store's queue order.
    Resting(usize),
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
    /// The resting queues in store order: a new queue and a queue parked by a
    /// swap both append, and a restore sorts by identity, so the order is
    /// creation order only until a queue is parked. The persisted file sorts
    /// by identity instead (`named_queues_state`).
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

    /// The queue `queue` addresses (design D5): `None` and the current queue's
    /// own identity reach the current queue, an identity the store holds
    /// reaches that resting queue, and anything else is `None` — the Owner no
    /// longer holds it, so the caller rejects the command.
    #[must_use]
    pub fn target(&self, queue: Option<NamedQueueId>) -> Option<NamedQueueTarget> {
        match queue {
            None => Some(NamedQueueTarget::Current),
            Some(id) if id == self.current => Some(NamedQueueTarget::Current),
            Some(id) => self
                .resting
                .iter()
                .position(|resting| resting.id == id)
                .map(NamedQueueTarget::Resting),
        }
    }

    /// Whether `name` is the name of any Named queue, the current queue
    /// included (design D6).
    fn uses_name(&self, name: &str) -> bool {
        self.current_name == name || self.resting.iter().any(|resting| resting.name == name)
    }

    /// The name a new queue takes: `Queue N` for the lowest `N` from 2 up that
    /// no queue uses, so a freed number is used again (design D6, spec
    /// `named-queues` "A Client creates and renames queues").
    #[must_use]
    pub fn next_queue_name(&self) -> String {
        let mut number = FIRST_NEW_QUEUE_NUMBER;
        loop {
            let name = format!("Queue {number}");
            if !self.uses_name(&name) {
                return name;
            }
            number = number.saturating_add(1);
        }
    }

    /// Adds one new, empty, resting queue to queue order: the next identity
    /// (never a reused one, design D3), the name `next_queue_name` gives it,
    /// no source, no start slot, and an empty queue drawing its revisions from
    /// `mint`. Returns the index it takes in queue order.
    pub fn push_new_queue(&mut self, mint: Arc<QueueRevisionMint>) -> usize {
        let id = self.next_queue_id;
        self.next_queue_id = NamedQueueId::from_raw(id.raw().saturating_add(1));
        let name = self.next_queue_name();
        self.resting.push(RestingNamedQueue {
            id,
            name,
            queue: PlaybackQueue::from_queue_items(Vec::new(), None, mint),
            source: QueueSource::Unknown,
            lineage: QueueLineage::default(),
            start_slot: None,
        });
        self.resting.len().saturating_sub(1)
    }

    /// Takes the resting queue at `index` out of queue order, or `None` when
    /// `index` names no queue.
    #[must_use]
    pub fn take_resting(&mut self, index: usize) -> Option<RestingNamedQueue> {
        (index < self.resting.len()).then(|| self.resting.remove(index))
    }

    /// The number of Named queues the Owner holds, the current queue included.
    /// Never zero: the current queue is always one queue, and the Owner refuses
    /// to close its last one (spec `named-queues` "The last queue does not
    /// close").
    #[must_use]
    pub fn queue_count(&self) -> usize {
        self.resting.len().saturating_add(1)
    }
}

/// The Owner's whole Named-queue state as `named_queues_state.json` holds it
/// (design D6): the current queue from the Owner core, the resting queues from
/// the store, and the next identity to mint. Queues are in identity order: a
/// new queue's identity is the highest so far and a parked queue's is not, so
/// each entry is inserted at its identity-ordered place instead of sorted.
pub(crate) fn named_queues_state(
    owner: &DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
) -> mbv_config::NamedQueuesState {
    let mut queues = Vec::with_capacity(owner.named_queues.resting.len().saturating_add(1));
    queues.push(current_queue_entry(owner, shared_queue));
    for resting in &owner.named_queues.resting {
        let entry = mbv_config::NamedQueue {
            id: resting.id,
            name: resting.name.clone(),
            source: resting.source.clone(),
            lineage: resting.lineage,
            start_slot: resting.start_slot,
            slots: resting.queue.slot_pairs(),
        };
        let at = queues.partition_point(|queue| queue.id < entry.id);
        queues.insert(at, entry);
    }
    mbv_config::NamedQueuesState {
        current: owner.named_queues.current,
        next_queue_id: owner.named_queues.next_queue_id,
        queues,
    }
}

/// The current queue's persisted entry: its name, source, lineage, start slot,
/// and slots with their identities, cloned from the Owner core.
fn current_queue_entry(
    owner: &DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
) -> mbv_config::NamedQueue {
    mbv_config::NamedQueue {
        id: owner.named_queues.current,
        name: owner.named_queues.current_name.clone(),
        source: owner.core.source.clone(),
        lineage: *shared_queue.lineage.lock().unwrap(),
        start_slot: shared_queue.start_slot(),
        slots: owner.core.queue.slot_pairs(),
    }
}

/// What the Named-queue file holds after the event loop's last write (design
/// D6): the current queue's cleared entry as the file holds it, the next
/// identity, and every resting queue's persisted fields with its queue
/// revision. The loop keeps the last capture, so a dirty event that still
/// matches it changed no persisted content and writes the file again only
/// once its content changes.
pub(crate) struct NamedQueuesSaved {
    /// The current queue's persisted entry, cleared of Service positions, as
    /// its serialized bytes: the persisted shape is the JSON, and `QueueItem`
    /// has no `PartialEq` to compare the entries with.
    current_entry_json: Vec<u8>,
    next_queue_id: NamedQueueId,
    resting: Vec<RestingSaved>,
}

/// One resting queue's persisted fields, with the queue revision that changes
/// whenever its slots do: a resting queue sees no progress observation, so
/// these fields carry its whole persisted content.
#[derive(Debug, PartialEq)]
struct RestingSaved {
    id: NamedQueueId,
    name: String,
    source: QueueSource,
    lineage: QueueLineage,
    start_slot: Option<QueueSlotId>,
    revision: u64,
}

impl RestingSaved {
    fn of(resting: &RestingNamedQueue) -> Self {
        Self {
            id: resting.id,
            name: resting.name.clone(),
            source: resting.source.clone(),
            lineage: resting.lineage,
            start_slot: resting.start_slot,
            revision: resting.queue.revision().raw(),
        }
    }
}

impl NamedQueuesSaved {
    /// Captures what a write of the Owner's Named-queue state puts in the
    /// file, so the next dirty event can compare against it.
    pub(crate) fn capture(owner: &DaemonPlayerOwner, shared_queue: &SharedQueueState) -> Self {
        Self {
            current_entry_json: cleared_current_entry_json(owner, shared_queue),
            next_queue_id: owner.named_queues.next_queue_id,
            resting: owner
                .named_queues
                .resting
                .iter()
                .map(RestingSaved::of)
                .collect(),
        }
    }

    /// Whether `owner` still holds exactly the state this capture describes:
    /// the current queue's persisted entry byte for byte, and every resting
    /// queue's persisted fields with its revision. Reads the resting queues'
    /// metadata only, so a match costs no resting slot clone.
    pub(crate) fn matches(
        &self,
        owner: &DaemonPlayerOwner,
        shared_queue: &SharedQueueState,
    ) -> bool {
        self.next_queue_id == owner.named_queues.next_queue_id
            && self.resting
                == owner
                    .named_queues
                    .resting
                    .iter()
                    .map(RestingSaved::of)
                    .collect::<Vec<_>>()
            && cleared_current_entry_json(owner, shared_queue) == self.current_entry_json
    }
}

/// The current queue's persisted entry as the file holds it: Service
/// positions cleared away, then serialized. An empty result compares unequal
/// to every capture, so a serialization failure only ever writes again.
fn cleared_current_entry_json(
    owner: &DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
) -> Vec<u8> {
    let entry = current_queue_entry(owner, shared_queue).into_without_service_positions();
    serde_json::to_vec(&entry).unwrap_or_default()
}
