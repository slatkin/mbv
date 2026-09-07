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

/// Outcome of [`OwnerTransitionState::accept`] (design D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchDecision {
    /// No transition was in flight: the accepted transition became `in_flight`
    /// and the caller sends it to the Playback run now.
    DispatchNow(Transition),
    /// A transition is already in flight: the accepted transition became
    /// `queued_latest`. Any previously queued transition it displaced is
    /// returned so the caller reports `Superseded` for it.
    Queued { superseded: Option<Transition> },
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

    /// One-in-flight dispatch (design D4). With nothing in flight, the accepted
    /// transition becomes `in_flight` and the caller dispatches it now. With a
    /// transition already in flight, the accepted transition becomes
    /// `queued_latest`, displacing (and superseding) any previously queued one;
    /// it is not sent to the Playback run until the in-flight request settles.
    pub fn accept(&mut self, transition: Transition) -> DispatchDecision {
        if self.in_flight.is_none() {
            self.in_flight = Some(transition);
            DispatchDecision::DispatchNow(transition)
        } else {
            let superseded = self.queued_latest.replace(transition);
            DispatchDecision::Queued { superseded }
        }
    }

    /// Clear the settled in-flight transition and promote `queued_latest` into
    /// its place, returning the promoted transition for the caller to dispatch.
    ///
    // task 3.3: real settlement must first confirm a Playback-run observation
    // matching the in-flight request identity AND its target slot; this stub
    // clears unconditionally and does not yet re-dispatch the promoted value.
    pub fn settle_in_flight(&mut self) -> Option<Transition> {
        self.in_flight = self.queued_latest.take();
        self.in_flight
    }

    /// Replace the queued-latest transition, returning the displaced one (its
    /// caller reports `Superseded` per design D4).
    pub fn replace_queued_latest(&mut self, transition: Transition) -> Option<Transition> {
        self.queued_latest.replace(transition)
    }

    /// In-flight / queued-latest transition summaries for the owner snapshot
    /// (design D5).
    pub fn summaries(
        &self,
    ) -> (
        Option<crate::ctrl::TransitionSummary>,
        Option<crate::ctrl::TransitionSummary>,
    ) {
        (
            self.in_flight.map(transition_summary),
            self.queued_latest.map(transition_summary),
        )
    }
}

fn transition_summary(t: Transition) -> crate::ctrl::TransitionSummary {
    crate::ctrl::TransitionSummary {
        request_id: t.request_id,
        generation: t.generation,
        target_slot: t.target.raw(),
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

        // Both A-transitions target the same slot: a settling observation must
        // correlate by `request_id`, never by `target`. An observation still
        // tagged with first-A's `request_id` (10) cannot be accepted as
        // settling a transition minted for second-A (12), even though both
        // point at `slot_a`.
        assert_eq!(state.in_flight().unwrap().target, slot_a);
        assert_eq!(state.queued_latest().unwrap().target, slot_a);
        assert_ne!(first_a.request_id, second_a.request_id);
    }

    // Three rapid requests: only the first is dispatched to the Playback run;
    // the queue retains just the newest, superseding the middle one (design D4).
    #[test]
    fn three_rapid_accepts_dispatch_one_and_retain_newest() {
        let slot = QueueSlotId::from_raw(1);
        let first = Transition::new(1, 1, slot);
        let second = Transition::new(2, 1, slot);
        let third = Transition::new(3, 1, slot);

        let mut state = OwnerTransitionState::default();
        assert_eq!(state.accept(first), DispatchDecision::DispatchNow(first));
        assert_eq!(
            state.accept(second),
            DispatchDecision::Queued { superseded: None }
        );
        assert_eq!(
            state.accept(third),
            DispatchDecision::Queued {
                superseded: Some(second)
            }
        );

        assert_eq!(state.in_flight(), Some(first));
        assert_eq!(state.queued_latest(), Some(third));
    }

    #[test]
    fn settle_in_flight_promotes_queued_latest() {
        let mut state = OwnerTransitionState::default();
        let a = Transition::new(1, 1, QueueSlotId::from_raw(1));
        let b = Transition::new(2, 1, QueueSlotId::from_raw(2));
        state.accept(a);
        state.accept(b);

        assert_eq!(state.settle_in_flight(), Some(b));
        assert_eq!(state.in_flight(), Some(b));
        assert_eq!(state.queued_latest(), None);
    }
}
