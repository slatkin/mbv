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
/// the store, and the next identity to mint.
pub(crate) fn named_queues_state(
    owner: &DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
) -> mbv_config::NamedQueuesState {
    let mut queues = vec![mbv_config::NamedQueue {
        id: owner.named_queues.current,
        name: owner.named_queues.current_name.clone(),
        source: owner.core.source.clone(),
        lineage: *shared_queue.lineage.lock().unwrap(),
        start_slot: shared_queue.start_slot(),
        slots: slots_of(&owner.core.queue),
    }];
    for resting in &owner.named_queues.resting {
        queues.push(mbv_config::NamedQueue {
            id: resting.id,
            name: resting.name.clone(),
            source: resting.source.clone(),
            lineage: resting.lineage,
            start_slot: resting.start_slot,
            slots: slots_of(&resting.queue),
        });
    }
    queues.sort_by_key(|queue| queue.id);
    mbv_config::NamedQueuesState {
        current: owner.named_queues.current,
        next_queue_id: owner.named_queues.next_queue_id,
        queues,
    }
}

/// The persisted slots of one queue, with their owner-assigned identities.
fn slots_of(queue: &PlaybackQueue) -> Vec<mbv_config::NamedQueueSlot> {
    queue
        .slots()
        .iter()
        .map(|slot| mbv_config::NamedQueueSlot {
            slot_id: slot.slot_id,
            item: slot.item.clone(),
        })
        .collect()
}
