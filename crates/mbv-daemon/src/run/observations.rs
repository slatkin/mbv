//! The run observations the Owner applies to its canonical queue: a natural
//! completion, a stop, and the queue's start slot that follows both
//! (gui-named-queues design D4). Every output records through these: the local
//! Playback run, a Cast receiver's natural end, and an Emby Session's end.

use crate::{DaemonPlayerOwner, SharedQueueState};
use mbv_ctrl::PlaybackGeneration;
use mbv_player::Player;
use mbv_queue::{ProgressObservation, QueueSlotId, StartSlotObservation};

pub(crate) fn playback_run_identity_is_current(
    run_identity: PlaybackGeneration,
    player: &Player,
) -> bool {
    run_identity == player.status.lock().unwrap().sequence_generation
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConsumePolicy {
    pub videos: bool,
    pub audio: bool,
}

pub(crate) fn apply_track_completed_observation(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    run_identity: PlaybackGeneration,
    slot_id: QueueSlotId,
    position_ticks: i64,
    was_played: bool,
    consume: bool,
    consume_policy: ConsumePolicy,
) -> bool {
    if !playback_run_identity_is_current(run_identity, player) {
        return false;
    }
    record_completed_slot(
        owner,
        shared_queue,
        slot_id,
        position_ticks,
        was_played,
        consume,
        consume_policy,
    )
}

/// The completion recording every natural end shares (cast-owner-output row
/// 6.3, design D7's Finished row): progress into the slot's canonical state,
/// the consume rules, and the shared snapshot publish. Local playback reaches it through
/// [`apply_track_completed_observation`]'s run-identity guard; a Cast natural
/// end records through it directly, because a receiver's end report carries no
/// local run identity to guard.
///
/// It also moves the queue's start slot past the completed one (gui-named-queues
/// design D4, spec "Each queue has a start slot"): a completion that advances
/// makes the next slot the start slot, after any consume, and a completion at
/// the end of the queue leaves the queue with none.
pub(crate) fn record_completed_slot(
    owner: &mut DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
    slot_id: QueueSlotId,
    position_ticks: i64,
    was_played: bool,
    consume: bool,
    consume_policy: ConsumePolicy,
) -> bool {
    // The completed slot's position, read before a consume removes it: the
    // slot that follows it is what a completion makes the start slot.
    let completed_index = owner.core.queue.slot_index(slot_id);
    if let Some(slot) = owner.core.queue.slot(slot_id) {
        let observation = ProgressObservation::Completed {
            position_ticks,
            played: was_played,
        };
        owner.core.apply_completion_progress(
            slot_id,
            observation.position_to_record(&slot.item),
            observation.played(),
        );
    }
    if owner.core.consume_completed_slot(
        slot_id,
        consume,
        consume_policy.videos,
        consume_policy.audio,
    ) {
        tracing::info!(name: "daemon.queue_slot.consumed", target: "consume", slot = ?slot_id, "completed track consumed");
    }
    if let Some(index) = completed_index {
        // After a consume the next slot sits at the completed one's position;
        // otherwise it follows it. Either way the queue's end leaves `None`.
        let next_index = if owner.core.queue.slot(slot_id).is_some() {
            index.saturating_add(1)
        } else {
            index
        };
        let next = owner
            .core
            .queue
            .slots()
            .get(next_index)
            .map(|slot| slot.slot_id);
        shared_queue.note_start_slot(&owner.core.queue, StartSlotObservation::Completed { next });
    }
    shared_queue.publish_observed(&owner.core);
    true
}

pub(crate) fn apply_stopped_observation(
    owner: &mut DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
    player: &Player,
    run_identity: PlaybackGeneration,
    slot_id: Option<QueueSlotId>,
    position_ticks: i64,
    was_played: bool,
) -> Option<bool> {
    if !playback_run_identity_is_current(run_identity, player) {
        return None;
    }
    let Some(slot_id) = slot_id else {
        return Some(false);
    };
    let Some(slot) = owner.core.queue.slot(slot_id) else {
        return Some(false);
    };
    let observation = ProgressObservation::Stopped {
        position_ticks,
        played: was_played,
    };
    owner.core.apply_completion_progress(
        slot_id,
        observation.position_to_record(&slot.item),
        observation.played(),
    );
    // A stop keeps the last observed active slot as the start slot
    // (gui-named-queues design D4, spec "Each queue has a start slot").
    shared_queue.note_start_slot(&owner.core.queue, StartSlotObservation::Stopped);
    Some(true)
}
