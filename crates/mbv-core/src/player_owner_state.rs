//! The Player owner's reusable authority for the Bound queue (design D1/D3).
//!
//! A single owner (the daemon event loop today; the Bare-mode shell in a later
//! unit) holds exactly one of these. Queue mutations go through the canonical
//! [`PlaybackQueue`] held here; `observed_active_slot` is private and changes
//! only via [`PlayerOwnerState::note_observed_active_slot`] from a Playback-run
//! observation, never from accepting a command.

use crate::playback_queue::{PlaybackQueue, QueueSlotId};
use crate::playback_transition::OwnerTransitionState;

#[derive(Default)]
pub struct PlayerOwnerState {
    pub(crate) queue: PlaybackQueue,
    pub(crate) source: crate::config::QueueSource,
    observed_active_slot: Option<QueueSlotId>,
    pub(crate) transitions: OwnerTransitionState,
}

impl PlayerOwnerState {
    /// Seed an owner core with an existing canonical queue and its source
    /// (queue adoption on startup / handoff).
    pub fn new(queue: PlaybackQueue, source: crate::config::QueueSource) -> Self {
        Self {
            queue,
            source,
            observed_active_slot: None,
            transitions: OwnerTransitionState::default(),
        }
    }

    /// Record a Playback-run observation of the active file. The only path
    /// permitted to change the observed active slot (design D3).
    pub fn note_observed_active_slot(&mut self, slot_id: Option<QueueSlotId>) {
        self.observed_active_slot = slot_id;
        if let Some(slot_id) = slot_id {
            self.queue.set_active_slot(slot_id);
        }
    }

    /// Resolve a Playback-run `TrackChanged` observation against the canonical
    /// queue. `Some((index, slot_id))` when the reported slot is still present,
    /// and the observed active slot is advanced to it. `None` means the report
    /// is stale — the caller discards it and leaves the canonical queue and
    /// observed active slot unchanged; a stale slot is never repaired by
    /// position (design D6).
    pub fn observe_track_change(&mut self, slot_id: QueueSlotId) -> Option<(usize, QueueSlotId)> {
        let index = self.queue.slot_index(slot_id)?;
        let resolved = self.queue.slots().get(index)?.slot_id;
        self.note_observed_active_slot(Some(resolved));
        Some((index, resolved))
    }

    /// The last Playback-run-observed active slot (design D3).
    pub fn observed_active_slot(&self) -> Option<QueueSlotId> {
        self.observed_active_slot
    }

    /// In-flight / queued-latest transition summaries for the owner snapshot
    /// (design D5).
    pub fn transition_summaries(
        &self,
    ) -> (
        Option<crate::ctrl::TransitionSummary>,
        Option<crate::ctrl::TransitionSummary>,
    ) {
        self.transitions.summaries()
    }
}
