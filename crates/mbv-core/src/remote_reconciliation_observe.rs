//! Observation-driven reconciliation logic for `ReconciliationTracker`.
//!
//! This module holds the `observe`/`reanchor` state machine and its private
//! helpers, split out of `remote_reconciliation.rs` to keep that file within
//! the repository's file-size limit.

use super::*;

impl ReconciliationTracker {
    pub fn observe(&mut self, observation: RemoteObservation) -> Vec<ReconciliationEffect> {
        let is_command_observation = self.active_command_generation == Some(observation.generation);
        if !self.active
            || observation.session_id != self.session_id
            || (!is_command_observation
                && self
                    .last_generation
                    .is_some_and(|generation| observation.generation <= generation))
        {
            return vec![];
        }
        // A poll racing a command is not authoritative for the command's
        // expected transition. Keep the expected state and observations
        // intact until the command is acknowledged and a post-command
        // observation becomes eligible.
        if self.expected.is_some()
            && !is_command_observation
            && self.active_command_generation.is_some()
        {
            return vec![];
        }
        self.last_generation = Some(
            self.last_generation
                .unwrap_or_default()
                .max(observation.generation),
        );
        if is_command_observation {
            self.active_command_generation = None;
        }
        self.previous_observation = self.current_observation.take();
        self.current_observation = Some(observation.clone());

        let was_starting = self.state == TrackingState::Starting;
        let mut effects = self.expire(observation.observed_at_ms);
        if was_starting && self.state == TrackingState::Invalid {
            return effects;
        }
        if self.state == TrackingState::Starting {
            effects.extend(self.observe_starting(observation));
            return effects;
        }
        if self.state == TrackingState::Suspended {
            effects.extend(self.observe_returning(observation));
            return effects;
        }

        if observation.stopped || observation.media_id.is_none() {
            effects.extend(self.observe_stopped(observation));
        } else {
            effects.extend(self.observe_playing(observation));
        }
        effects
    }

    pub fn reanchor(&mut self, occurrence_index: usize) -> Vec<ReconciliationEffect> {
        if !self.active || occurrence_index >= self.submitted.len() {
            return vec![];
        }
        if self.state != TrackingState::Invalid && self.state != TrackingState::Ambiguous {
            return vec![];
        }
        let Some(observed_media_id) = self
            .current_observation
            .as_ref()
            .and_then(|observation| observation.media_id.as_deref())
        else {
            return vec![];
        };
        if self.submitted[occurrence_index].media_id != observed_media_id {
            return vec![];
        }
        if self.state == TrackingState::Ambiguous && !self.candidates.contains(&occurrence_index) {
            return vec![];
        }
        for (idx, occurrence) in self.submitted.iter().enumerate() {
            if idx != occurrence_index
                && !self.completed.contains(&occurrence.occurrence_id)
                && idx < occurrence_index
            {
                self.bypassed.insert(occurrence.occurrence_id);
            }
        }
        self.epoch = self.epoch.saturating_add(1);
        self.candidates = vec![occurrence_index];
        self.expected = None;
        self.active_command_generation = None;
        self.set_state(TrackingState::Tracking, TrackingReason::Reanchored)
    }

    fn observe_starting(&mut self, observation: RemoteObservation) -> Vec<ReconciliationEffect> {
        let Some(expected) = self.expected.clone() else {
            return self.set_state(TrackingState::Invalid, TrackingReason::StartExpired);
        };
        let target = match expected.intent {
            RemoteIntent::Submit { target } => target,
            _ => return vec![],
        };
        let Some(target) = self
            .submitted
            .iter()
            .position(|occurrence| occurrence.occurrence_id == target)
        else {
            return vec![];
        };
        if observation.media_id.as_deref() == Some(self.submitted[target].media_id.as_str()) {
            self.expected = None;
            self.candidates = vec![target];
            return self.set_state(TrackingState::Tracking, TrackingReason::Confirmed);
        }
        // A stale pre-submission item is not contradictory until it shows
        // meaningful progress after the submission has been dispatched.
        if observation.media_id.is_none() || observation.position_ticks <= 0 {
            return vec![];
        }
        self.expected = None;
        self.set_state(TrackingState::Invalid, TrackingReason::StartContradicted)
    }

    fn observe_returning(&mut self, observation: RemoteObservation) -> Vec<ReconciliationEffect> {
        if observation.stopped || observation.media_id.is_none() {
            return vec![];
        }
        let media_id = observation.media_id.as_deref().unwrap_or_default();
        let mut reachable = Vec::new();
        for &candidate in &self.candidates {
            if self.submitted[candidate].media_id == media_id {
                reachable.push(candidate);
            }
            if let Some(successor) = candidate.checked_add(1) {
                if self
                    .submitted
                    .get(successor)
                    .is_some_and(|item| item.media_id == media_id)
                {
                    reachable.push(successor);
                }
            }
        }
        reachable.sort_unstable();
        reachable.dedup();
        if reachable.len() == 1 {
            let next = reachable[0];
            let same = self.candidates.contains(&next);
            if same || self.candidates.iter().any(|&idx| idx + 1 == next) {
                self.candidates = reachable;
                let mut effects =
                    self.set_state(TrackingState::Tracking, TrackingReason::Confirmed);
                if !same {
                    effects.extend(self.maybe_complete_prior());
                }
                return effects;
            }
        }
        if reachable.len() > 1 {
            self.candidates = reachable;
            return self.set_state(
                TrackingState::Ambiguous,
                TrackingReason::DuplicateCandidates,
            );
        }
        let unique = self
            .submitted
            .iter()
            .enumerate()
            .filter(|(_, item)| item.media_id == media_id)
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();
        if unique.len() == 1 {
            self.candidates = unique;
            return self.set_state(
                TrackingState::Invalid,
                TrackingReason::ReturningStateRequiresReanchor,
            );
        }
        self.set_state(
            TrackingState::Invalid,
            TrackingReason::ReturningStateIncompatible,
        )
    }

    fn observe_stopped(&mut self, _observation: RemoteObservation) -> Vec<ReconciliationEffect> {
        let mut effects = Vec::new();
        if let Some(expected) = self.expected.take() {
            if matches!(expected.intent, RemoteIntent::Stop) {
                effects.push(ReconciliationEffect::ExpectedConfirmed);
                return effects;
            }
            self.expected = Some(expected);
        }
        if self.state == TrackingState::Tracking && self.candidates.len() == 1 {
            let idx = self.candidates[0];
            if idx + 1 == self.submitted.len()
                && self.previous_observation.as_ref().is_some_and(|previous| {
                    Self::near_end(previous.position_ticks, previous.runtime_ticks)
                })
            {
                effects.extend(self.complete(idx));
            }
        }
        effects
    }

    fn observe_playing(&mut self, observation: RemoteObservation) -> Vec<ReconciliationEffect> {
        let media_id = observation.media_id.as_deref().unwrap_or_default();
        let expected = self.expected.clone();
        if let Some(expected) = expected {
            let mut target_ambiguity_retained = false;
            if let Some(target) = Self::intent_target(&expected.intent) {
                let Some(target) = self
                    .submitted
                    .iter()
                    .position(|occurrence| occurrence.occurrence_id == target)
                else {
                    return vec![];
                };
                if self.submitted[target].media_id == media_id {
                    if self.expected_move_provable(&expected, target) {
                        self.expected = None;
                        let non_adjacent =
                            self.candidates.len() == 1 && target.abs_diff(self.candidates[0]) > 1;
                        self.candidates = vec![target];
                        if non_adjacent {
                            self.epoch = self.epoch.saturating_add(1);
                        }
                        let mut effects = vec![ReconciliationEffect::ExpectedConfirmed];
                        effects.extend(
                            self.set_state(TrackingState::Tracking, TrackingReason::Confirmed),
                        );
                        return effects;
                    }
                    // A target that is a duplicate of the source media cannot
                    // be proven from an observation alone. Keep the Expected
                    // transition pending so a stale observation cannot confirm
                    // or contradict it, and never complete an occurrence that
                    // was not uniquely observed.
                    target_ambiguity_retained = true;
                }
            }
            if !target_ambiguity_retained {
                if matches!(expected.intent, RemoteIntent::Restart | RemoteIntent::Seek)
                    && self
                        .candidates
                        .iter()
                        .any(|&idx| self.submitted[idx].media_id == media_id)
                {
                    self.expected = None;
                    return vec![ReconciliationEffect::ExpectedConfirmed];
                }
                // An incompatible live command is evidence of contradiction, not
                // proof that playback is invalid. Reconcile it as unprompted.
                self.expected = None;
            }
        }

        let mut next_candidates = Vec::new();
        for &candidate in &self.candidates {
            let current = &self.submitted[candidate];
            if current.media_id == media_id {
                if self.same_occurrence_reset(candidate, &observation) {
                    if self
                        .submitted
                        .get(candidate + 1)
                        .is_some_and(|next| next.media_id == media_id)
                    {
                        next_candidates.push(candidate);
                        next_candidates.push(candidate + 1);
                    } else {
                        return self
                            .set_state(TrackingState::Invalid, TrackingReason::MaterialReset);
                    }
                } else {
                    next_candidates.push(candidate);
                }
            }
            if self
                .submitted
                .get(candidate + 1)
                .is_some_and(|next| next.media_id == media_id)
            {
                next_candidates.push(candidate + 1);
            }
        }
        next_candidates.sort_unstable();
        next_candidates.dedup();
        if next_candidates.is_empty() {
            let reason = if self
                .current_observation
                .as_ref()
                .zip(self.previous_observation.as_ref())
                .is_some_and(|(current, previous)| {
                    current
                        .position_ticks
                        .saturating_sub(previous.position_ticks)
                        < -POSITION_JITTER_TICKS
                }) {
                TrackingReason::BackwardTransition
            } else {
                TrackingReason::NonAdjacentTransition
            };
            return self.set_state(TrackingState::Invalid, reason);
        }
        let advanced = self
            .candidates
            .iter()
            .any(|candidate| next_candidates.contains(&(*candidate + 1)));
        let mut effects = Vec::new();
        if advanced && self.candidates.len() == 1 {
            effects.extend(self.maybe_complete_prior());
        }
        self.candidates = next_candidates;
        if self.candidates.len() > 1 {
            effects.extend(self.set_state(
                TrackingState::Ambiguous,
                TrackingReason::DuplicateCandidates,
            ));
        } else {
            effects.extend(self.set_state(TrackingState::Tracking, TrackingReason::Confirmed));
        }
        effects
    }

    fn same_occurrence_reset(&self, candidate: usize, observation: &RemoteObservation) -> bool {
        let Some(previous) = self.previous_observation.as_ref() else {
            return false;
        };
        if previous.media_id.as_deref() != Some(self.submitted[candidate].media_id.as_str())
            || observation.position_ticks >= previous.position_ticks
        {
            return false;
        }
        previous
            .position_ticks
            .saturating_sub(observation.position_ticks)
            > POSITION_JITTER_TICKS
    }

    /// Whether an Expected move onto `target` can be confirmed from the
    /// observed media alone. Duplicate source/target media cannot prove an
    /// occurrence transition from observation alone: a target that shares the
    /// source's media identity must stay pending rather than be confirmed (or
    /// later completed) for an occurrence that was never uniquely observed.
    fn expected_move_provable(&self, expected: &ExpectedTransition, target: usize) -> bool {
        match expected.source {
            Some(source) => {
                source == target
                    || self.submitted[source].media_id != self.submitted[target].media_id
            }
            None => !self.candidates.iter().any(|&candidate| {
                candidate != target
                    && self.submitted[candidate].media_id == self.submitted[target].media_id
            }),
        }
    }

    fn maybe_complete_prior(&mut self) -> Vec<ReconciliationEffect> {
        let Some(previous) = self.previous_observation.as_ref() else {
            return vec![];
        };
        if self.candidates.len() != 1 {
            return vec![];
        }
        let prior = self.candidates[0];
        if prior + 1 >= self.submitted.len()
            || !Self::near_end(previous.position_ticks, previous.runtime_ticks)
        {
            return vec![];
        }
        self.complete(prior)
    }

    fn complete(&mut self, idx: usize) -> Vec<ReconciliationEffect> {
        let occurrence = self.submitted[idx].clone();
        if self.completed.insert(occurrence.occurrence_id) {
            vec![ReconciliationEffect::Completion(occurrence)]
        } else {
            vec![]
        }
    }

    pub(super) fn resolved_index(&self) -> Option<usize> {
        (self.candidates.len() == 1).then_some(self.candidates[0])
    }

    fn intent_target(intent: &RemoteIntent) -> Option<OccurrenceId> {
        match intent {
            RemoteIntent::Submit { target }
            | RemoteIntent::Next { target }
            | RemoteIntent::Previous { target }
            | RemoteIntent::Select { target } => Some(*target),
            RemoteIntent::Restart | RemoteIntent::Seek | RemoteIntent::Stop => None,
        }
    }

    pub(super) fn set_state(
        &mut self,
        state: TrackingState,
        reason: TrackingReason,
    ) -> Vec<ReconciliationEffect> {
        self.state = state;
        self.reason = reason.clone();
        vec![ReconciliationEffect::StateChanged { state, reason }]
    }

    /// Inclusive 95 percent comparison in a widened integer domain.
    pub fn near_end(position_ticks: i64, runtime_ticks: i64) -> bool {
        position_ticks > 0
            && runtime_ticks > 0
            && (position_ticks as u128) * 20 >= (runtime_ticks as u128) * 19
    }
}
