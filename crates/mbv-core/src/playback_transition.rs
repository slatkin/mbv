//! Owner-side desired-transition state (design D3/D4).
//!
//! The Player owner tracks the transition it has dispatched to the Playback
//! run (`in_flight`) and, separately, the newest transition held back behind
//! it (`queued_latest`). This module only defines that state; the dispatch
//! and settlement logic that consumes it lands in Section 3.

use crate::ctrl::{PlaybackGeneration, PlaybackRequestId};
use crate::playback_queue::QueueSlotId;

/// A desired playback transition: the correlated request identity plus the
/// canonical slot it targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub target: QueueSlotId,
}

impl Transition {
    pub fn new(
        request_id: PlaybackRequestId,
        generation: PlaybackGeneration,
        target: QueueSlotId,
    ) -> Self {
        Self {
            request_id,
            generation,
            target,
        }
    }
}

/// Owner holder for desired-transition state. At most one transition is in
/// flight to the Playback run; at most one newer transition is queued behind
/// it (design D4).
#[derive(Debug, Clone, Default)]
pub struct OwnerTransitionState {
    in_flight: Option<Transition>,
    queued_latest: Option<Transition>,
}

impl OwnerTransitionState {
    pub fn in_flight(&self) -> Option<Transition> {
        self.in_flight
    }

    pub fn queued_latest(&self) -> Option<Transition> {
        self.queued_latest
    }

    pub fn set_in_flight(&mut self, transition: Option<Transition>) {
        self.in_flight = transition;
    }

    /// Replace the queued-latest transition, returning the displaced one (its
    /// caller reports `Superseded` per design D4).
    pub fn replace_queued_latest(&mut self, transition: Transition) -> Option<Transition> {
        self.queued_latest.replace(transition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A -> B -> A: the first and last request target the same slot but are
    // distinct requests. Neither the in-flight nor the queued-latest holder
    // may conflate their `PlaybackRequestId`s.
    #[test]
    fn a_b_a_sequence_retains_distinct_request_identities() {
        let slot_a = QueueSlotId::from_raw(1);
        let slot_b = QueueSlotId::from_raw(2);

        let first_a = Transition::new(10, 1, slot_a);
        let to_b = Transition::new(11, 1, slot_b);
        let second_a = Transition::new(12, 1, slot_a);

        let mut state = OwnerTransitionState::default();
        state.set_in_flight(Some(first_a));
        assert!(state.replace_queued_latest(to_b).is_none());

        let displaced = state.replace_queued_latest(second_a);
        assert_eq!(displaced.map(|t| t.request_id), Some(11));

        assert_eq!(state.in_flight().unwrap().request_id, 10);
        assert_eq!(state.in_flight().unwrap().target, slot_a);
        assert_eq!(state.queued_latest().unwrap().request_id, 12);
        assert_eq!(state.queued_latest().unwrap().target, slot_a);
        assert_ne!(
            state.in_flight().unwrap().request_id,
            state.queued_latest().unwrap().request_id
        );
    }
}
