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

    /// Replace the Bare owner's canonical queue after a local queue mutation.
    /// Daemon owners mutate this state directly; Bare keeps the existing shell
    /// queue API and synchronizes the same canonical value at the boundary.
    pub fn sync_canonical_queue(&mut self, queue: PlaybackQueue) {
        let observed = self
            .observed_active_slot
            .filter(|slot_id| queue.slot(*slot_id).is_some());
        self.queue = queue;
        self.observed_active_slot = observed;
        if let Some(slot_id) = observed {
            let _ = self.queue.set_active_slot(slot_id);
        } else {
            self.queue.clear_active_slot();
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

    /// Consume a completed slot in the owner's canonical queue when the
    /// completion event and the configured per-kind policy both allow it.
    /// Returns `true` only when a slot was actually removed.
    pub fn consume_completed_slot(
        &mut self,
        slot_id: QueueSlotId,
        consume: bool,
        consume_videos: bool,
        consume_audio: bool,
    ) -> bool {
        let Some(slot) = self.queue.slot(slot_id) else {
            return false;
        };
        let allowed = consume
            && ((slot.item.is_video() && consume_videos)
                || (slot.item.is_audio() && consume_audio));
        allowed
            && matches!(
                self.queue.consume_slot(slot_id),
                crate::playback_queue::QueueMutationResult::Applied(_)
            )
    }

    /// The last Playback-run-observed active slot (design D3).
    pub fn observed_active_slot(&self) -> Option<QueueSlotId> {
        self.observed_active_slot
    }

    /// Mint an owner-local transition identity for Bare-mode playback.
    pub fn mint_local_transition(
        &mut self,
    ) -> (
        crate::ctrl::PlaybackRequestId,
        crate::ctrl::PlaybackGeneration,
    ) {
        self.transitions.mint_local_id()
    }

    pub fn accept_local_transition(
        &mut self,
        transition: crate::playback_transition::Transition,
    ) -> crate::playback_transition::DispatchDecision {
        self.transitions.accept(transition)
    }

    pub fn settle_local_transition(
        &mut self,
        request_id: crate::ctrl::PlaybackRequestId,
        slot_id: QueueSlotId,
    ) -> crate::playback_transition::SettleOutcome {
        self.transitions.settle(request_id, slot_id)
    }

    pub fn expire_local_transition(
        &mut self,
        now: std::time::Instant,
    ) -> crate::playback_transition::ExpireOutcome {
        self.transitions.expire(now)
    }

    pub fn reset_local_transitions(&mut self) {
        self.transitions.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback_transition::{DispatchDecision, ExpireOutcome, Transition};
    use std::time::{Duration, Instant};

    #[test]
    fn bare_transition_expiry_promotes_queued_jump() {
        let mut owner = PlayerOwnerState::default();
        let slot_a = QueueSlotId::from_raw(1);
        let slot_b = QueueSlotId::from_raw(2);
        let first = Transition::new(1, 1, slot_a);
        let queued = Transition::new(2, 2, slot_b);

        assert_eq!(
            owner.accept_local_transition(first),
            DispatchDecision::DispatchNow(first)
        );
        assert_eq!(
            owner.accept_local_transition(queued),
            DispatchDecision::Queued { superseded: None }
        );
        let start = Instant::now();
        assert_eq!(owner.expire_local_transition(start), ExpireOutcome::Pending);
        assert_eq!(
            owner.expire_local_transition(start + Duration::from_secs(6)),
            ExpireOutcome::Expired {
                expired: first,
                dispatch_next: Some(queued),
            }
        );
    }
}
