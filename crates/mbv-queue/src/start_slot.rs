//! The start-slot rule of the `named-queues` spec, "Each queue has a start
//! slot": the one place that decides where a Play starts a queue, from the run
//! observations the Owner takes (design D4 of `gui-named-queues`).

use crate::{PlaybackQueue, QueueSlotId};

/// One run or queue-edit observation that the start-slot rule reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartSlotObservation {
    /// The playback output reports this slot as the active one. While a queue
    /// plays, its active slot is its start slot.
    Active(QueueSlotId),
    /// Playback stopped on the item it was playing: the last observed active
    /// slot stays the start slot.
    Stopped,
    /// A natural completion advanced to `next`, or ended the queue when it is
    /// `None`.
    Completed { next: Option<QueueSlotId> },
    /// The slot that occupied `index` was removed. `queue` is the queue as it
    /// stands after the removal, so the slot now at that index becomes the
    /// start slot.
    StartSlotRemoved { index: usize },
    /// `queue` is a Named queue that has just become the current queue. Its
    /// own stored start slot becomes the current queue's start slot, or
    /// `None` when it had none (gui-named-queues design D4).
    Installed { start_slot: Option<QueueSlotId> },
}

/// The start slot `queue` holds after `observation`, given the start slot it
/// held before it.
///
/// The rule never stores a position: a start slot names a slot, and the item
/// that plays from it takes its position from its Resume source.
#[must_use]
pub fn start_slot_after(
    queue: &PlaybackQueue,
    start_slot: Option<QueueSlotId>,
    observation: StartSlotObservation,
) -> Option<QueueSlotId> {
    match observation {
        StartSlotObservation::Active(slot_id) => Some(slot_id),
        StartSlotObservation::Stopped => start_slot,
        StartSlotObservation::Completed { next } => next,
        StartSlotObservation::StartSlotRemoved { index } => {
            queue.slots().get(index).map(|slot| slot.slot_id)
        }
        StartSlotObservation::Installed { start_slot } => start_slot,
    }
}
