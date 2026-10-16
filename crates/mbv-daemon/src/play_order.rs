//! Owner play-order forwarding (queue-shuffle-play-order design D2/D4).
//!
//! The owner core picks the next slot; the Playback run obeys. Every daemon
//! path that runs an owner-core order or re-pick hook forwards the resulting
//! advance decision here, so the run and the shared snapshot never lag the
//! core's order.

use crate::SharedQueueState;
use mbv_ctrl::player::PlayerCommand;
use mbv_ctrl::{Direction, PlaybackRequestId};
use mbv_player::{Player, PlayerOwnerState};
use mbv_queue::{PlayOrder, PlaybackQueue, QueueSlotId};

use crate::core::DaemonOwnerContext;

/// The slot the Next-Up card must name (queue-shuffle-play-order task 4.2):
/// the owner's order resolved against the canonical queue — the Shuffle pick
/// in Shuffle, the queue neighbor in Sequential. `None` when the order is the
/// end of the queue or the target slot is gone.
pub(crate) fn next_up_slot(owner: &PlayerOwnerState, queue: &PlaybackQueue) -> Option<QueueSlotId> {
    match owner.play_order() {
        PlayOrder::Sequential => {
            let active_idx = queue.active_index().unwrap_or(0);
            queue.slots().get(active_idx + 1).map(|slot| slot.slot_id)
        }
        PlayOrder::Shuffle {
            upcoming: Some(slot_id),
        } => queue.slot(slot_id).map(|slot| slot.slot_id),
        PlayOrder::Shuffle { upcoming: None } => None,
    }
}

/// The owner's one step rule (cast-owner-output design D7): a relative step —
/// a next or previous key, and the advance a natural end starts — resolves
/// its target through `relative_step_target` and dispatches a jump transition
/// through the one-in-flight gate. A jump returns the dispatched transition's
/// request id; the edge and a coalesced same-direction step return `None` and
/// leave the playback state alone.
pub(crate) fn dispatch_relative_step(
    ctx: &mut DaemonOwnerContext<'_>,
    direction: Direction,
) -> Option<PlaybackRequestId> {
    let mbv_player::owner_state::StepTarget::Jump(slot_id) =
        ctx.owner.core.relative_step_target(direction)
    else {
        return None;
    };
    Some(dispatch_step_target(ctx, slot_id, direction))
}

/// Dispatches the jump one resolved step target names (the dispatch half of
/// [`dispatch_relative_step`], cast-owner-output row 6.3): the transport jump
/// goes out under a freshly minted request id, and the id comes back so the
/// caller can track the step it dispatched.
pub(crate) fn dispatch_step_target(
    ctx: &mut DaemonOwnerContext<'_>,
    slot_id: QueueSlotId,
    direction: Direction,
) -> PlaybackRequestId {
    let (request_id, generation) = ctx.owner.core.transitions.mint_local_id();
    crate::core::dispatch_slot_jump(
        ctx,
        crate::core::JumpOrigin::Transport,
        mbv_player::transition::Transition::with_cause(
            request_id,
            generation,
            slot_id,
            mbv_player::transition::TransitionCause::Step(direction),
        ),
        None,
    );
    request_id
}

/// Forward the owner's current play order to the Playback run. Called after
/// every owner-core hook that can change the pick or the order.
pub(crate) fn forward_play_order_advance(owner: &PlayerOwnerState, player: &Player) {
    player.send_command(PlayerCommand::SetUpcoming(owner.play_order()));
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
    forward_play_order_advance(owner, player);
}
