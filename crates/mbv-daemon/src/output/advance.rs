//! The shared queue advance every remote output steps with
//! (cast-owner-output design D7, emby-session-owner-output design D6). A
//! natural end records the ended slot's completion through the same
//! observation funnel `TrackCompleted` records through (progress, consume
//! rules) and steps the queue with the same `relative_step_target(Next)`
//! funnel the next key uses; a step target that cannot play on the output is
//! reported and stepped over, with the queue length bounding the attempts, and
//! at the edge the output stays idle.
//!
//! The state machine is shared: each remote variant of `PlaybackOutput` owns
//! one `OutputAdvance`, and the code here reaches the active one through the
//! output's accessors. Only the end report that starts an advance differs per
//! output: the Cast receiver's `CastStatus` end table stays in `cast/advance`.

use mbv_ctrl::{Direction, PlaybackRequestId};
use mbv_player::owner_state::StepTarget;
use mbv_queue::QueueSlotId;

use crate::DaemonLoop;

/// The advance one natural end started (design D6): the step this advance has
/// in resolve, the slot of the last failed step it is stepping over, and the
/// step attempts the queue length still allows. It lives on the output so a
/// stop, an output change, or the loaded item's replacement ends it with the
/// output.
#[derive(Debug)]
pub(crate) struct OutputAdvance {
    /// The request id of the step the advance dispatched and whose resolve is
    /// still due, so a failed resolve can tell its own step from a user
    /// transition that resolved in between.
    step_request: Option<PlaybackRequestId>,
    /// The slot the last failed step targeted; the next step starts after it.
    failed_slot: Option<QueueSlotId>,
    /// The step attempts the queue length still allows (design D6).
    remaining: usize,
}

/// Starts the advance a natural end began (design D6): the ended slot is
/// recorded as the observed active slot ahead of the step, its completion is
/// recorded through the `TrackCompleted` observation funnel, the queue length
/// bounds the step attempts, and the steps go through the shared step funnel.
pub(crate) fn begin_advance(daemon: &mut DaemonLoop) {
    // The completion observation design D6's end row requires ahead of the
    // step: a load whose in-flight transition expired before a settling
    // observation never recorded its slot, and the shared funnel would
    // otherwise step from the stale predecessor and reload the item that just
    // finished.
    let Some(ended) = daemon.owner.output.loaded_slot() else {
        return;
    };
    observe_slot(daemon, ended);
    // The step target resolves while the ended slot is still in the queue:
    // the completion record's consume rules can remove it, and the shared
    // step funnel has no base to step from once the slot it observes is gone.
    let target = daemon.owner.core.relative_step_target(Direction::Next);
    record_natural_end(daemon, ended, matches!(target, StepTarget::Jump(_)));
    let remaining = daemon.owner.core.queue.len();
    daemon.owner.output.set_advance(Some(OutputAdvance {
        step_request: None,
        failed_slot: None,
        remaining,
    }));
    step_target_advance(daemon, target);
}

/// Records the ended slot's completion through the same observation funnel
/// `TrackCompleted` records through (design D6's end row): progress into the
/// slot's canonical state and the consume rules. The payload is the outcome a
/// local natural end decides (`FinishReason::Natural`): the position resets,
/// `played` stays video-only, and a finished track is consumable. `consume`
/// follows the resolved step target — at the queue edge the local outcome is
/// the `Stopped` observation, which consumes nothing.
fn record_natural_end(daemon: &mut DaemonLoop, ended: QueueSlotId, consume: bool) {
    let played = daemon
        .owner
        .core
        .queue
        .slot(ended)
        .is_none_or(|slot| !slot.item.is_audio());
    // The Feed completion event's kind gate is read before the record: a
    // consumed slot is gone from the queue by the time the event goes out.
    let is_feed = crate::cast::slot_is_feed(&daemon.owner.core.queue, ended);
    let consume_policy = (daemon.owner_settings)().consume;
    crate::record_completed_slot(
        &mut daemon.owner,
        &daemon.player,
        &daemon.shared_queue,
        ended,
        0,
        played,
        consume,
        consume_policy,
    );
    // The ended Feed slot's `TrackCompleted` goes out beside the record: the
    // payload is the recorded outcome, the same one a local natural end's
    // `TrackCompleted` carries, so the Client's Feed lifecycle decision is the
    // same one local playback drives. Feed slots never pass Emby Session
    // admission, so the event stays on the Cast side (design D6).
    if is_feed {
        crate::cast::emit_natural_end_track_completed(daemon, ended, played, consume);
    }
}

/// One advance step attempt (design D6). The step target is resolved through
/// the shared step funnel, then dispatched with the advance's bookkeeping.
fn step_advance(daemon: &mut DaemonLoop) {
    rebase_on_failed_step(daemon);
    // The queue length bounds the attempts (design D6): an advance that has
    // spent them ends with the output idle.
    if daemon
        .owner
        .output
        .advance()
        .is_none_or(|advance| advance.remaining == 0)
    {
        clear_advance(daemon);
        return;
    }
    let target = daemon.owner.core.relative_step_target(Direction::Next);
    step_target_advance(daemon, target);
}

/// The advance bookkeeping and dispatch one resolved step target performs
/// (design D6) — shared by the advance's own steps and the natural end's
/// first step, whose target the completion record may have consumed the ended
/// slot out from under. A jump dispatches the load whose resolve outcome
/// continues or ends the advance; the edge and a coalesced step leave the
/// same-direction step already loading the next slot in charge.
fn step_target_advance(daemon: &mut DaemonLoop, target: StepTarget) {
    let dispatched = match target {
        StepTarget::Jump(slot_id) => {
            let mut ctx = crate::cast::owner_context(daemon);
            Some(crate::play_order::dispatch_step_target(
                &mut ctx,
                slot_id,
                Direction::Next,
            ))
        }
        StepTarget::Coalesced | StepTarget::AtEdge => None,
    };
    if let Some(request_id) = dispatched {
        if let Some(advance) = daemon.owner.output.advance_mut() {
            advance.remaining -= 1;
            advance.step_request = Some(request_id);
        }
        return;
    }
    // The edge or a coalesced step ended the advance: the output stays idle
    // (design D6), or the step already loading the next slot takes over.
    clear_advance(daemon);
}

/// Ends the advance without a step: the queue has no target for it, or a
/// queued user transition the failed step's expiry promoted took the queue
/// over.
pub(crate) fn clear_advance(daemon: &mut DaemonLoop) {
    daemon.owner.output.set_advance(None);
}

/// Records `slot` as the owner's observed active slot (the only path for that
/// record), forwarding the play order when the note re-picked the Shuffle
/// pick — the settle path's pattern.
fn observe_slot(daemon: &mut DaemonLoop, slot: QueueSlotId) {
    let crate::core::DaemonPlayerOwner { core, .. } = &mut daemon.owner;
    let order_before = core.play_order();
    core.note_observed_active_slot(Some(slot));
    if core.play_order() != order_before {
        crate::forward_play_order_advance(core, &daemon.player);
    }
}

/// Re-bases the shared step funnel on the slot the failed step targeted
/// (design D6's step-over): that slot becomes the desired active slot, so the
/// funnel steps from after it and the Shuffle pick re-picks past it. A failed
/// slot an edit removed ends the advance: the edited queue has no order past
/// it.
fn rebase_on_failed_step(daemon: &mut DaemonLoop) {
    let Some(failed) = daemon
        .owner
        .output
        .advance()
        .and_then(|advance| advance.failed_slot)
    else {
        return;
    };
    if daemon.owner.core.queue.slot(failed).is_none() {
        daemon.owner.output.set_advance(None);
        return;
    }
    observe_slot(daemon, failed);
}

/// Whether a failed resolve belonged to the advance's in-flight step (design
/// D6). Consumes the step record when it does, so the caller reports the
/// failure and either steps over or abandons the advance.
pub(crate) fn step_failed(daemon: &mut DaemonLoop, request_id: PlaybackRequestId) -> bool {
    match daemon.owner.output.advance_mut() {
        Some(advance) if advance.step_request == Some(request_id) => {
            advance.step_request = None;
            true
        }
        _ => false,
    }
}

/// Continues the advance past the failed step: the next step starts after the
/// slot the failed step targeted (design D6).
pub(crate) fn continue_after_failed_step(daemon: &mut DaemonLoop, failed_slot: QueueSlotId) {
    match daemon.owner.output.advance_mut() {
        Some(advance) => advance.failed_slot = Some(failed_slot),
        None => return,
    }
    step_advance(daemon);
}
