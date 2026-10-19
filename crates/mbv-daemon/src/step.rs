//! Owner step dispatch and Next-Up resolution (cast-owner-output design D7,
//! queue-content-shuffle 5.3): a relative step resolves the queue neighbor
//! and dispatches a jump transition through the one-in-flight gate, and the
//! Next-Up card names that same queue neighbor.

use mbv_ctrl::{Direction, PlaybackRequestId};
use mbv_queue::{PlaybackQueue, QueueSlotId};

use crate::core::DaemonOwnerContext;

/// The slot the Next-Up card must name: the queue neighbor after the active
/// slot. `None` when the queue ends at the active slot.
pub(crate) fn next_up_slot(queue: &PlaybackQueue) -> Option<QueueSlotId> {
    let active_idx = queue.active_index().unwrap_or(0);
    queue.slots().get(active_idx + 1).map(|slot| slot.slot_id)
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
    // Content the Owner did not load is the device's own playback
    // (emby-session-owner-output design D9): next and previous go to the
    // Session as its own commands, and the Owner loads nothing.
    if let crate::cast::PlaybackOutput::EmbySession(session) = &mut ctx.owner.output
        && crate::emby_session::command::route_step(session, direction)
    {
        return None;
    }
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
