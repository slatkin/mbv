//! Process-local reconciliation for playback submitted to a generic Emby client.
//!
//! The model deliberately knows nothing about HTTP, playlist mutation, or the
//! terminal UI. It turns an immutable submitted sequence, ordered remote
//! observations, and mbv-issued intents into bounded, occurrence-aware effects.

use std::collections::HashSet;

pub type OccurrenceId = u64;

const POSITION_JITTER_TICKS: i64 = 3 * crate::api::TICKS_PER_SECOND;
const EXPECTED_TRANSITION_TTL_MS: u64 = 15_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SequenceSource {
    Playlist { id: String },
    Other { label: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmittedOccurrence {
    pub occurrence_id: OccurrenceId,
    pub media_id: String,
    pub playlist_item_id: Option<String>,
    pub source: Option<SequenceSource>,
    pub runtime_ticks: i64,
}

impl SubmittedOccurrence {
    pub fn new(occurrence_id: OccurrenceId, media_id: impl Into<String>) -> Self {
        Self {
            occurrence_id,
            media_id: media_id.into(),
            playlist_item_id: None,
            source: None,
            runtime_ticks: 0,
        }
    }

    pub fn playlist_entry(mut self, entry_id: impl Into<String>) -> Self {
        self.playlist_item_id = Some(entry_id.into());
        self
    }

    pub fn runtime_ticks(mut self, runtime_ticks: i64) -> Self {
        self.runtime_ticks = runtime_ticks;
        self
    }

    pub fn source(mut self, source: SequenceSource) -> Self {
        self.source = Some(source);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackingState {
    Starting,
    Tracking,
    Ambiguous,
    Invalid,
    Suspended,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackingReason {
    WaitingForStart,
    Confirmed,
    DuplicateCandidates,
    NonAdjacentTransition,
    BackwardTransition,
    MaterialReset,
    StartExpired,
    StartContradicted,
    SessionUnavailable,
    ReturningStateRequiresReanchor,
    ReturningStateIncompatible,
    CommandFailed,
    Reanchored,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteObservation {
    pub generation: u64,
    pub session_id: String,
    pub media_id: Option<String>,
    pub position_ticks: i64,
    pub runtime_ticks: i64,
    pub stopped: bool,
    pub observed_at_ms: u64,
}

impl RemoteObservation {
    pub fn playing(
        generation: u64,
        session_id: impl Into<String>,
        media_id: impl Into<String>,
        position_ticks: i64,
        runtime_ticks: i64,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            generation,
            session_id: session_id.into(),
            media_id: Some(media_id.into()),
            position_ticks,
            runtime_ticks,
            stopped: false,
            observed_at_ms,
        }
    }

    pub fn stopped(generation: u64, session_id: impl Into<String>, observed_at_ms: u64) -> Self {
        Self {
            generation,
            session_id: session_id.into(),
            media_id: None,
            position_ticks: 0,
            runtime_ticks: 0,
            stopped: true,
            observed_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteIntent {
    Submit { target: usize },
    Next { target: usize },
    Previous { target: usize },
    Select { target: usize },
    Restart,
    Seek,
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedTransition {
    pub intent: RemoteIntent,
    pub source: Option<usize>,
    pub epoch: u64,
    pub deadline_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconciliationEffect {
    StateChanged {
        state: TrackingState,
        reason: TrackingReason,
    },
    Completion(SubmittedOccurrence),
    ExpectedConfirmed,
    ExpectedContradicted,
    ExpectedExpired,
}

#[derive(Clone, Debug)]
pub struct ReconciliationTracker {
    session_id: String,
    submitted: Vec<SubmittedOccurrence>,
    state: TrackingState,
    reason: TrackingReason,
    candidates: Vec<usize>,
    epoch: u64,
    next_generation: u64,
    command_generation: Option<u64>,
    last_generation: Option<u64>,
    previous_observation: Option<RemoteObservation>,
    current_observation: Option<RemoteObservation>,
    expected: Option<ExpectedTransition>,
    completed: HashSet<OccurrenceId>,
    consumed: HashSet<OccurrenceId>,
    bypassed: HashSet<OccurrenceId>,
    active: bool,
}

impl ReconciliationTracker {
    pub fn new(
        session_id: impl Into<String>,
        submitted: Vec<SubmittedOccurrence>,
        start: usize,
        now_ms: u64,
    ) -> Option<Self> {
        if submitted.len() < 2 || start >= submitted.len() {
            return None;
        }
        Some(Self {
            session_id: session_id.into(),
            submitted,
            state: TrackingState::Starting,
            reason: TrackingReason::WaitingForStart,
            candidates: vec![start],
            epoch: 0,
            next_generation: 1,
            command_generation: None,
            last_generation: None,
            previous_observation: None,
            current_observation: None,
            expected: Some(ExpectedTransition {
                intent: RemoteIntent::Submit { target: start },
                source: None,
                epoch: 0,
                deadline_ms: now_ms.saturating_add(EXPECTED_TRANSITION_TTL_MS),
            }),
            completed: HashSet::new(),
            consumed: HashSet::new(),
            bypassed: HashSet::new(),
            active: true,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn submitted(&self) -> &[SubmittedOccurrence] {
        &self.submitted
    }

    pub fn state(&self) -> TrackingState {
        self.state
    }

    pub fn reason(&self) -> &TrackingReason {
        &self.reason
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn candidates(&self) -> Vec<OccurrenceId> {
        self.candidates
            .iter()
            .filter_map(|&idx| self.submitted.get(idx).map(|o| o.occurrence_id))
            .collect()
    }

    pub fn current_occurrence(&self) -> Option<&SubmittedOccurrence> {
        (self.candidates.len() == 1)
            .then(|| self.submitted.get(self.candidates[0]))
            .flatten()
    }

    pub fn current_index(&self) -> Option<usize> {
        self.resolved_index()
    }

    /// Occurrences that the current remote item can be explicitly re-anchored
    /// to. Ambiguous tracking is limited to its retained candidates; an
    /// invalid session may recover at any submitted occurrence with the
    /// observed media identity.
    pub fn reanchor_targets(&self) -> Vec<(usize, String)> {
        if !self.active
            || !matches!(
                self.state,
                TrackingState::Invalid | TrackingState::Ambiguous
            )
        {
            return vec![];
        }
        let Some(media_id) = self
            .current_observation
            .as_ref()
            .and_then(|observation| observation.media_id.as_deref())
        else {
            return vec![];
        };
        let indices = if self.state == TrackingState::Ambiguous {
            self.candidates.clone()
        } else {
            self.submitted
                .iter()
                .enumerate()
                .filter_map(|(index, occurrence)| {
                    (occurrence.media_id == media_id).then_some(index)
                })
                .collect()
        };
        indices
            .into_iter()
            .filter_map(|index| {
                self.submitted
                    .get(index)
                    .filter(|occurrence| occurrence.media_id == media_id)
                    .map(|occurrence| (index, occurrence.media_id.clone()))
            })
            .collect()
    }

    pub fn expected(&self) -> Option<&ExpectedTransition> {
        self.expected.as_ref()
    }

    pub fn completed(&self) -> impl Iterator<Item = OccurrenceId> + '_ {
        self.completed.iter().copied()
    }

    pub fn consumed(&self) -> impl Iterator<Item = OccurrenceId> + '_ {
        self.consumed.iter().copied()
    }

    pub fn bypassed(&self) -> impl Iterator<Item = OccurrenceId> + '_ {
        self.bypassed.iter().copied()
    }

    pub fn mark_consumed(&mut self, occurrence_id: OccurrenceId) -> bool {
        self.completed.contains(&occurrence_id) && self.consumed.insert(occurrence_id)
    }

    pub fn consume_pending(&self, epoch: u64, occurrence_id: OccurrenceId) -> bool {
        self.active
            && self.state == TrackingState::Tracking
            && self.epoch == epoch
            && self.completed.contains(&occurrence_id)
            && self.consumed.contains(&occurrence_id)
    }

    pub fn next_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        generation
    }

    /// Associate the current reconciliation-affecting command with this tracker.
    pub fn track_command_generation(&mut self, generation: u64) {
        self.command_generation = Some(generation);
    }

    pub fn command_generation_matches(&self, generation: u64) -> bool {
        self.command_generation == Some(generation)
    }

    /// Reject observations from polls issued before this causal boundary.
    pub fn accept_observations_from(&mut self, generation: u64) {
        self.last_generation = generation.checked_sub(1);
    }

    pub fn issue_intent(&mut self, intent: RemoteIntent, now_ms: u64) {
        if !self.active {
            return;
        }
        let source = self.resolved_index();
        let target = match intent {
            RemoteIntent::Submit { target }
            | RemoteIntent::Next { target }
            | RemoteIntent::Previous { target }
            | RemoteIntent::Select { target } => Some(target),
            RemoteIntent::Restart | RemoteIntent::Seek | RemoteIntent::Stop => None,
        };
        if target.is_some_and(|idx| idx >= self.submitted.len()) {
            return;
        }
        self.expected = Some(ExpectedTransition {
            intent,
            source,
            epoch: self.epoch,
            deadline_ms: now_ms.saturating_add(EXPECTED_TRANSITION_TTL_MS),
        });
    }

    pub fn command_failed(&mut self) -> Vec<ReconciliationEffect> {
        if !self.active {
            return vec![];
        }
        self.active = false;
        self.expected = None;
        self.set_state(TrackingState::Invalid, TrackingReason::CommandFailed)
    }

    pub fn stop_tracking(&mut self) {
        self.active = false;
        self.expected = None;
    }

    pub fn poll_failed(&self) -> Vec<ReconciliationEffect> {
        vec![]
    }

    pub fn session_disappeared(&mut self) -> Vec<ReconciliationEffect> {
        if !self.active {
            return vec![];
        }
        self.set_state(TrackingState::Suspended, TrackingReason::SessionUnavailable)
    }

    pub fn expire(&mut self, now_ms: u64) -> Vec<ReconciliationEffect> {
        let Some(expected) = self.expected.as_ref() else {
            return vec![];
        };
        if now_ms < expected.deadline_ms {
            return vec![];
        }
        self.expected = None;
        let mut effects = vec![ReconciliationEffect::ExpectedExpired];
        if self.state == TrackingState::Starting {
            effects.extend(self.set_state(TrackingState::Invalid, TrackingReason::StartExpired));
        }
        effects
    }

    pub fn observe(&mut self, observation: RemoteObservation) -> Vec<ReconciliationEffect> {
        if !self.active
            || observation.session_id != self.session_id
            || self
                .last_generation
                .is_some_and(|generation| observation.generation <= generation)
        {
            return vec![];
        }
        self.last_generation = Some(observation.generation);
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
            if let Some(target) = Self::intent_target(&expected.intent) {
                if self.submitted[target].media_id == media_id {
                    self.expected = None;
                    let non_adjacent =
                        self.candidates.len() == 1 && target.abs_diff(self.candidates[0]) > 1;
                    self.candidates = vec![target];
                    if non_adjacent {
                        self.epoch = self.epoch.saturating_add(1);
                    }
                    let mut effects = vec![ReconciliationEffect::ExpectedConfirmed];
                    effects
                        .extend(self.set_state(TrackingState::Tracking, TrackingReason::Confirmed));
                    return effects;
                }
            }
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

    fn resolved_index(&self) -> Option<usize> {
        (self.candidates.len() == 1).then_some(self.candidates[0])
    }

    fn intent_target(intent: &RemoteIntent) -> Option<usize> {
        match intent {
            RemoteIntent::Submit { target }
            | RemoteIntent::Next { target }
            | RemoteIntent::Previous { target }
            | RemoteIntent::Select { target } => Some(*target),
            RemoteIntent::Restart | RemoteIntent::Seek | RemoteIntent::Stop => None,
        }
    }

    fn set_state(
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

#[cfg(test)]
#[path = "remote_reconciliation_tests.rs"]
mod tests;
