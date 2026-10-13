//! Owner play-order forwarding (queue-shuffle-play-order design D2/D4).
//!
//! The owner core picks the next slot; the Playback run obeys. Every daemon
//! path that runs an owner-core order or re-pick hook forwards the resulting
//! advance decision here, so the run and the shared snapshot never lag the
//! core's order.

use crate::SharedQueueState;
use mbv_ctrl::player::{Advance, PlayerCommand};
use mbv_player::{Player, PlayerOwnerState};
use mbv_queue::{PlaybackQueue, QueueSlotId};

/// The advance decision the Playback run obeys for the owner's order
/// (design D2). One mapping for every forwarding site: Sequential advances
/// in queue order, Shuffle advances to the picked slot, and a Shuffle order
/// with no pickable slot advances as the end of the queue.
fn advance_for_play_order(play_order: mbv_queue::PlayOrder) -> Advance {
    match play_order {
        mbv_queue::PlayOrder::Sequential => Advance::Sequential,
        mbv_queue::PlayOrder::Shuffle { upcoming } => upcoming.map_or(Advance::End, Advance::To),
    }
}

/// The slot the Next-Up card must name (queue-shuffle-play-order task 4.2):
/// the owner's advance decision resolved against the canonical queue — the
/// Shuffle pick in Shuffle, the queue neighbor in Sequential. `None` when the
/// advance is the end of the queue or the target slot is gone.
pub(crate) fn next_up_slot(owner: &PlayerOwnerState, queue: &PlaybackQueue) -> Option<QueueSlotId> {
    match advance_for_play_order(owner.play_order()) {
        Advance::Sequential => {
            let active_idx = queue.active_index().unwrap_or(0);
            queue.slots().get(active_idx + 1).map(|slot| slot.slot_id)
        }
        Advance::To(slot_id) => queue.slot(slot_id).map(|slot| slot.slot_id),
        Advance::End => None,
    }
}

/// Forward the owner's current advance decision to the Playback run. Called
/// after every owner-core hook that can change the pick or the order, when
/// the order actually changed.
pub(crate) fn forward_play_order_advance(owner: &PlayerOwnerState, player: &Player) {
    player.send_command(PlayerCommand::SetUpcoming(advance_for_play_order(
        owner.play_order(),
    )));
}

/// Reset the order for a whole-queue replacement (design D4): Sequential in
/// the owner core, in the shared snapshot, and in the Playback run. Called
/// where the daemon mints a new `QueueLineage`.
pub(crate) fn reset_play_order_for_queue_replacement(
    owner: &mut PlayerOwnerState,
    shared_queue: &SharedQueueState,
    player: &Player,
) {
    owner.note_queue_replaced();
    *shared_queue.play_order.lock().unwrap() = mbv_queue::PlayOrderMode::Sequential;
    player.send_command(PlayerCommand::SetUpcoming(Advance::Sequential));
}
