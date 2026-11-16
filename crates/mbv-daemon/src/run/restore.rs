// The queue state a Local Owner starts from (gui-named-queues design D6): the
// current Named queue with its saved slot identities, source, lineage, and start
// slot, and every resting queue beside it. An earlier single-queue owner file
// loads through the `mbv-config` migration as one current queue named `Default`.

use crate::DaemonRole;
use crate::core::{NamedQueueStore, RestingNamedQueue};
use mbv_config::NamedQueuesState;
use mbv_queue::{
    NamedQueueId, PlaybackQueue, QueueItem, QueueLineage, QueueRevisionMint, QueueSlotId,
    QueueSource,
};
use std::sync::Arc;

/// The queues the Owner holds when it starts serving its first Client.
pub(crate) struct StartupQueues {
    /// The current queue, with its saved slot identities.
    pub queue: PlaybackQueue,
    /// The current queue's source.
    pub source: QueueSource,
    /// The current queue's lineage.
    pub lineage: QueueLineage,
    /// The Named queues the Owner holds: the current queue's identity and name,
    /// the next identity to mint, and the resting queues.
    pub store: NamedQueueStore,
}

impl StartupQueues {
    /// An empty current queue named `Default`, with no resting queue: a fresh
    /// Local Owner, and every packaged `mbvd`.
    pub(crate) fn fresh(mint: &Arc<QueueRevisionMint>) -> Self {
        Self {
            queue: PlaybackQueue::from_queue_items(Vec::new(), None, Arc::clone(mint)),
            source: QueueSource::Unknown,
            lineage: QueueLineage::default(),
            store: NamedQueueStore::first_start(),
        }
    }

    /// The state the persisted Named queues restore (design D6). The current
    /// queue keeps its saved slot identities, source, lineage, and start slot,
    /// which also becomes its active slot so the player seeds its playlist
    /// there; every other queue comes back as a resting queue with its own.
    /// `None` when the state holds no queue at all.
    pub(crate) fn restored(
        state: &NamedQueuesState,
        mint: &Arc<QueueRevisionMint>,
    ) -> Option<Self> {
        // A file whose current identity names no queue falls back to the first
        // queue, so a corrupt `current` field cannot drop every queue.
        let current = state
            .queues
            .iter()
            .find(|queue| queue.id == state.current)
            .or_else(|| state.queues.iter().min_by_key(|queue| queue.id))?;
        let queue = PlaybackQueue::from_slot_items(
            slot_items(current),
            current.start_slot,
            Arc::clone(mint),
        );
        let mut resting = state
            .queues
            .iter()
            .filter(|held| held.id != current.id)
            .map(|held| resting_queue(held, mint))
            .collect::<Vec<_>>();
        resting.sort_by_key(|held| held.id);
        Some(Self {
            queue,
            source: current.source.clone(),
            lineage: current.lineage,
            store: NamedQueueStore {
                current: current.id,
                current_name: current.name.clone(),
                next_queue_id: next_queue_id(state),
                resting,
            },
        })
    }
}

/// The queues the Owner starts from: a Local Owner restores its Named queues
/// from their state file (design D6), and every other role starts empty.
pub(crate) fn startup_queues(role: DaemonRole, mint: &Arc<QueueRevisionMint>) -> StartupQueues {
    if role != DaemonRole::Local {
        return StartupQueues::fresh(mint);
    }
    restored_state()
        .and_then(|state| StartupQueues::restored(&state, mint))
        .unwrap_or_else(|| StartupQueues::fresh(mint))
}

/// The Named-queue state to restore: the new state file, else the earlier
/// single-queue owner file converted to one current `Default` queue, else the
/// oldest queue state file when no owner file exists yet. Every conversion reads
/// only; the Owner's next save writes the new file, so the earlier files stay as
/// they are (design D6).
fn restored_state() -> Option<NamedQueuesState> {
    let owner_path = mbv_config::stay_alive_queue_state_path();
    mbv_config::load_or_migrate_named_queues_state_at(
        &mbv_config::named_queues_state_path(),
        &owner_path,
    )
    .or_else(|| {
        (!owner_path.exists())
            .then(mbv_config::load_queue_state)
            .flatten()
            .and_then(|queue| {
                mbv_config::legacy_queue_for_owner_if_absent(&owner_path, Some(queue))
            })
            .map(mbv_config::converted_named_queues_state)
    })
}

/// The identity the next new queue takes: the saved one, raised above every
/// identity the file holds, so a hand-edited file cannot mint a duplicate.
fn next_queue_id(state: &NamedQueuesState) -> NamedQueueId {
    state
        .queues
        .iter()
        .map(|queue| queue.id.raw().saturating_add(1))
        .max()
        .map_or(state.next_queue_id, |lowest_free| {
            state.next_queue_id.max(NamedQueueId::from_raw(lowest_free))
        })
}

/// One persisted queue as a resting queue. Its start slot is kept only when it
/// still names a slot the queue holds.
fn resting_queue(
    queue: &mbv_config::NamedQueue,
    mint: &Arc<QueueRevisionMint>,
) -> RestingNamedQueue {
    let restored = PlaybackQueue::from_slot_items(slot_items(queue), None, Arc::clone(mint));
    RestingNamedQueue {
        id: queue.id,
        name: queue.name.clone(),
        start_slot: queue
            .start_slot
            .filter(|slot_id| restored.slot(*slot_id).is_some()),
        queue: restored,
        source: queue.source.clone(),
        lineage: queue.lineage,
    }
}

/// The slots of one persisted queue, with their saved identities.
fn slot_items(queue: &mbv_config::NamedQueue) -> Vec<(QueueSlotId, QueueItem)> {
    queue
        .slots
        .iter()
        .map(|slot| (slot.slot_id, slot.item.clone()))
        .collect()
}
