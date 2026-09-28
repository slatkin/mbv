//! The Player owner's reusable authority for the Bound queue (design D1/D3).
//!
//! A single owner (the daemon event loop today; the Bare-mode shell in a later
//! unit) holds exactly one of these. Queue mutations go through the canonical
//! [`PlaybackQueue`] held here; `observed_active_slot` is private and changes
//! only via [`PlayerOwnerState::note_observed_active_slot`] from a Playback-run
//! observation, never from accepting a command.

use crate::transition::OwnerTransitionState;
use crate::transition::TransitionCause;
use mbv_ctrl::Direction;
use mbv_queue::{PlaybackQueue, QueueRevisionMint, QueueSlotId};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepTarget {
    Jump(QueueSlotId),
    Coalesced,
    AtEdge,
}

#[derive(Debug)]
pub struct PlayerOwnerState {
    pub queue: PlaybackQueue,
    pub revision_mint: Arc<QueueRevisionMint>,
    pub source: mbv_queue::QueueSource,
    observed_active_slot: Option<QueueSlotId>,
    pub transitions: OwnerTransitionState,
}

impl Default for PlayerOwnerState {
    fn default() -> Self {
        let revision_mint = Arc::new(QueueRevisionMint::default());
        Self::new(
            PlaybackQueue::from_queue_items(Vec::new(), None, Arc::clone(&revision_mint)),
            mbv_queue::QueueSource::default(),
        )
    }
}

impl PlayerOwnerState {
    /// Seed an owner core with an existing canonical queue and its source
    /// (queue adoption on startup / handoff).
    #[must_use]
    pub fn new(queue: PlaybackQueue, source: mbv_queue::QueueSource) -> Self {
        let revision_mint = queue.revision_mint();
        Self {
            queue,
            revision_mint,
            source,
            observed_active_slot: None,
            transitions: OwnerTransitionState::default(),
        }
    }

    /// Replace the Bare owner's canonical queue after a local queue mutation.
    /// Daemon owners mutate this state directly; Bare keeps the existing shell
    /// queue API and synchronizes the same canonical value at the boundary.
    pub fn sync_canonical_queue(&mut self, queue: PlaybackQueue) {
        self.revision_mint = queue.revision_mint();
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

    /// Apply a completed/stopped occurrence's resolved position to the
    /// canonical queue, so the owner's own queue — and every broadcast built
    /// from it — reflects real progress instead of the slot's submission-time
    /// position. Callers resolve the meaningful-progress gate before calling.
    pub fn apply_completion_progress(
        &mut self,
        slot_id: QueueSlotId,
        position_ticks: i64,
        played: bool,
    ) {
        let _ = self.queue.apply_progress(slot_id, position_ticks, played);
        let _ = self.queue.mark_progress_sync_pending(slot_id);
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
                mbv_queue::QueueMutationResult::Applied(_)
            )
    }

    /// Resolve a relative step from the latest desired/observed queue position (design D3).
    #[must_use]
    pub fn relative_step_target(&self, direction: Direction) -> StepTarget {
        let latest = self
            .transitions
            .queued_latest()
            .or_else(|| self.transitions.in_flight());
        if latest.is_some_and(|t| t.cause == TransitionCause::Step(direction)) {
            return StepTarget::Coalesced;
        }
        let base = latest
            .map(|t| t.target)
            .or(self.observed_active_slot)
            .or_else(|| self.queue.active_slot_id());
        let Some(index) = base.and_then(|slot| self.queue.slot_index(slot)) else {
            return StepTarget::AtEdge;
        };
        let neighbor = match direction {
            Direction::Next => index.checked_add(1).filter(|&i| i < self.queue.len()),
            Direction::Previous => index.checked_sub(1),
        };
        neighbor
            .and_then(|i| {
                self.queue
                    .slots()
                    .get(i)
                    .map(|s| StepTarget::Jump(s.slot_id))
            })
            .unwrap_or(StepTarget::AtEdge)
    }

    /// The last Playback-run-observed active slot (design D3).
    #[must_use]
    pub fn observed_active_slot(&self) -> Option<QueueSlotId> {
        self.observed_active_slot
    }

    /// Mint an owner-local transition identity for Bare-mode playback.
    pub fn mint_local_transition(
        &mut self,
    ) -> (mbv_ctrl::PlaybackRequestId, mbv_ctrl::PlaybackGeneration) {
        self.transitions.mint_local_id()
    }

    pub fn accept_local_transition(
        &mut self,
        transition: crate::transition::Transition,
    ) -> crate::transition::DispatchDecision {
        self.transitions.accept(transition)
    }

    pub fn settle_local_transition(
        &mut self,
        request_id: mbv_ctrl::PlaybackRequestId,
        slot_id: QueueSlotId,
    ) -> crate::transition::SettleOutcome {
        self.transitions.settle(request_id, slot_id)
    }

    pub fn expire_local_transition(
        &mut self,
        now: std::time::Instant,
    ) -> crate::transition::ExpireOutcome {
        self.transitions.expire(now)
    }

    pub fn clear_unconfirmable_transition(&mut self) {
        self.transitions.clear_unconfirmable();
    }

    pub fn reset_local_transitions(&mut self) {
        self.transitions.reset();
    }

    /// The slot of the owner's in-flight desired transition, if any: the local
    /// owner's equivalent of the daemon's published in-flight transition, so a
    /// Bare-mode shell can paint the selected slot as now-playing before the
    /// Playback run confirms it (design D3/D4).
    #[must_use]
    pub fn in_flight_transition_slot(&self) -> Option<QueueSlotId> {
        self.transitions
            .in_flight()
            .map(|transition| transition.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::{DispatchDecision, ExpireOutcome, Transition};
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

    fn item(id: &str) -> mbv_emby_model::EmbyItem {
        mbv_emby_model::EmbyItem {
            id: id.to_string(),
            name: format!("Item {id}"),
            item_type: "Episode".to_string(),
            is_folder: false,
            child_count: None,
            media_type: "Video".to_string(),
            collection_type: String::new(),
            runtime_ticks: 100 * mbv_emby_model::TICKS_PER_SECOND,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            artist_items: Vec::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genres: Vec::new(),
            people: Vec::new(),
            external_urls: Vec::new(),
            playlist_item_id: String::new(),
            image_tags: mbv_emby_model::EmbyImageTags::default(),
        }
    }

    // Regression: the daemon used to drop TrackCompleted/Stopped position and
    // played state on the floor instead of applying it to the canonical
    // queue, so the very next broadcast reverted a stopped item's progress
    // back to its submission-time position (0, for a freshly queued item).
    #[test]
    fn relative_steps_use_desired_target_edges_and_coalesce_matching_direction() {
        use crate::transition::{Transition, TransitionCause};
        let queue = PlaybackQueue::from_items(
            vec![item("a"), item("b"), item("c")],
            Some(0),
            Arc::new(QueueRevisionMint::default()),
        );
        let slots: Vec<_> = queue.slots().iter().map(|slot| slot.slot_id).collect();
        let mut owner = PlayerOwnerState::new(queue, mbv_queue::QueueSource::default());
        owner.accept_local_transition(Transition::with_cause(
            1,
            1,
            slots[1],
            TransitionCause::Step(Direction::Next),
        ));
        assert_eq!(
            owner.relative_step_target(Direction::Next),
            StepTarget::Coalesced
        );
        assert_eq!(
            owner.relative_step_target(Direction::Previous),
            StepTarget::Jump(slots[0])
        );
        owner.reset_local_transitions();
        owner.note_observed_active_slot(Some(slots[2]));
        assert_eq!(
            owner.relative_step_target(Direction::Next),
            StepTarget::AtEdge
        );
    }

    #[test]
    fn step_and_direct_jump_dispatch_the_same_canonical_resume_issue_824() {
        use crate::transition::{Transition, TransitionCause};
        use mbv_queue::ProgressObservation;

        let mut audio = item("audio");
        audio.media_type = "Audio".to_string();
        audio.playback_position_ticks = 8 * mbv_emby_model::TICKS_PER_SECOND;
        let mut video = item("video");
        video.playback_position_ticks = 20 * 60 * mbv_emby_model::TICKS_PER_SECOND;

        for (target_item, observed_ticks) in [
            (audio, 8 * mbv_emby_model::TICKS_PER_SECOND),
            (video, 12 * mbv_emby_model::TICKS_PER_SECOND),
        ] {
            let mut queue = PlaybackQueue::from_items(
                vec![item("current"), target_item],
                Some(0),
                Arc::new(QueueRevisionMint::default()),
            );
            let target = queue.slots()[1].slot_id;
            let slot_item = &queue.slot(target).expect("target exists").item;
            let recorded = ProgressObservation::Completed {
                position_ticks: observed_ticks,
                played: false,
            }
            .position_to_record(slot_item);
            queue.apply_progress(target, recorded, false);

            let step_jump = {
                let mut owner =
                    PlayerOwnerState::new(queue.clone(), mbv_queue::QueueSource::default());
                let StepTarget::Jump(target) = owner.relative_step_target(Direction::Next) else {
                    panic!("next step must resolve target");
                };
                let transition =
                    Transition::with_cause(1, 1, target, TransitionCause::Step(Direction::Next));
                let crate::transition::DispatchDecision::DispatchNow(transition) =
                    owner.accept_local_transition(transition)
                else {
                    panic!("step transition must dispatch immediately");
                };
                transition.into_jump(crate::resume_ticks_for_slot(&owner.queue, target))
            };
            let direct_jump = {
                let mut owner = PlayerOwnerState::new(queue, mbv_queue::QueueSource::default());
                let transition = Transition::new(2, 2, target);
                let crate::transition::DispatchDecision::DispatchNow(transition) =
                    owner.accept_local_transition(transition)
                else {
                    panic!("direct jump must dispatch immediately");
                };
                transition.into_jump(crate::resume_ticks_for_slot(&owner.queue, target))
            };
            let resume = |command| match command {
                mbv_ctrl::player::PlayerCommand::JumpTo { resume_ticks, .. } => resume_ticks,
                _ => panic!("transition dispatch must produce JumpTo"),
            };
            assert_eq!(resume(step_jump), resume(direct_jump));
        }
    }

    #[test]
    fn apply_completion_progress_advances_canonical_queue_and_revision() {
        let queue = PlaybackQueue::from_items(
            vec![item("a")],
            Some(0),
            Arc::new(QueueRevisionMint::default()),
        );
        let slot_id = queue.slots()[0].slot_id;
        let before_revision = queue.revision();
        let mut owner = PlayerOwnerState::new(queue, mbv_queue::QueueSource::default());

        let watched_ticks = 86 * mbv_emby_model::TICKS_PER_SECOND;
        owner.apply_completion_progress(slot_id, watched_ticks, false);

        let slot = owner.queue.slot(slot_id).expect("slot still present");
        assert_eq!(slot.item.playback_position_ticks(), watched_ticks);
        assert_ne!(owner.queue.revision(), before_revision);
    }
}
