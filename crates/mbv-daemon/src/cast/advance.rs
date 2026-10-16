//! The Owner's queue advance from a receiver's reported end (cast-owner-output
//! design D7, row 6.2). The advance table runs on every fresh status report:
//! a `Finished` end of the loaded item marks the end handled and steps the
//! queue with the same `relative_step_target(Next)` funnel the next key uses;
//! an `Error` end is reported once and advances nothing; a `Cancelled` or
//! `Interrupted` end and an idle for foreign content only change what the
//! presentation shows (row 6.1). An uncastable step target is reported and
//! stepped over, with the queue length bounding the attempts; at the edge the
//! output stays idle.

use mbv_cast::client::{CastIdleReason, CastPlaybackState};
use mbv_ctrl::{CtrlEvent, Direction, PlaybackRequestId};
use mbv_queue::QueueSlotId;

use super::super::DaemonLoop;
use super::PlaybackOutput;

/// The advance one natural end started (design D7): the step this advance has
/// in resolve, the slot of the last failed step it is stepping over, and the
/// step attempts the queue length still allows. It lives on the output so a
/// stop, an output change, or the loaded item's replacement ends it with the
/// output.
#[derive(Debug)]
pub(crate) struct CastAdvance {
    /// The request id of the step the advance dispatched and whose resolve is
    /// still due, so a failed resolve can tell its own step from a user
    /// transition that resolved in between.
    step_request: Option<PlaybackRequestId>,
    /// The slot the last failed step targeted; the next step starts after it.
    failed_slot: Option<QueueSlotId>,
    /// The step attempts the queue length still allows (design D7).
    remaining: usize,
}

/// What the freshly stored status's end report asks the Owner to do (design
/// D7's advance table).
enum EndReport {
    Step,
    ReportError,
}

/// Applies the advance table to the output's freshly stored status (row 6.2).
/// Only an idle report that still names the loaded content id and an
/// end this output has not handled yet reaches the table: the end-handled
/// flag makes the second poll of the same `Finished` — and the same `Error` —
/// a no-op, so a receiver that lingers on one end report advances exactly
/// once.
pub(super) fn apply_end_report(daemon: &mut DaemonLoop) {
    let report = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        let (Some(status), Some(loaded)) = (cast.last_status.as_ref(), cast.loaded.as_mut()) else {
            return;
        };
        if status.state != CastPlaybackState::Idle
            || status.playing_content_id.as_deref() != Some(loaded.content_id.as_str())
            || loaded.end_handled
        {
            return;
        }
        match status.idle_reason {
            Some(CastIdleReason::Finished) => {
                loaded.end_handled = true;
                Some(EndReport::Step)
            }
            Some(CastIdleReason::Error) => {
                loaded.end_handled = true;
                Some(EndReport::ReportError)
            }
            // A Cancelled end presents no active playback and advances
            // nothing; an Interrupted end is the Owner's own replacing load.
            // Both are presentation-only (row 6.1).
            _ => None,
        }
    };
    match report {
        Some(EndReport::Step) => begin_advance(daemon),
        Some(EndReport::ReportError) => report_receiver_error(daemon),
        None => {}
    }
}

/// Reports the receiver's failure to play the loaded item (design D7, spec
/// "Receiver fails to play the item"): the failure is named to every Client
/// and nothing is advanced or loaded.
fn report_receiver_error(daemon: &mut DaemonLoop) {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return;
    };
    let item = cast
        .loaded
        .as_ref()
        .and_then(|loaded| daemon.owner.core.queue.slot(loaded.slot))
        .map(|slot| slot.item.display_name());
    let report = match item {
        Some(name) => format!("\"{name}\" failed to play on the receiver"),
        None => "the receiver failed to play the loaded item".to_string(),
    };
    tracing::warn!(
        name: "cast.playback.failed",
        target: "cast",
        report = %report,
        "the receiver reported an error for the loaded item"
    );
    crate::core::broadcast(&daemon.ctrl_clients, &CtrlEvent::CommandRejected(report));
}

/// Starts the advance a natural end began (design D7): the ended slot is
/// recorded as the observed active slot ahead of the step, the queue length
/// bounds the step attempts, and the steps go through the shared step funnel.
fn begin_advance(daemon: &mut DaemonLoop) {
    // The completion observation design D7's Finished row requires ahead of
    // the step: a load whose in-flight transition expired before a settling
    // status never recorded its slot, and the shared funnel would otherwise
    // step from the stale predecessor and reload the item that just finished.
    let ended = {
        let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
            return;
        };
        match cast.loaded.as_ref() {
            Some(loaded) => loaded.slot,
            None => return,
        }
    };
    observe_slot(daemon, ended);
    let remaining = daemon.owner.core.queue.len();
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    cast.advance = Some(CastAdvance {
        step_request: None,
        failed_slot: None,
        remaining,
    });
    step_advance(daemon);
}

/// One advance step attempt (design D7). The shared step funnel resolves the
/// target: a jump dispatches the load whose resolve outcome continues or ends
/// the advance, the edge leaves the output idle, and a coalesced step leaves
/// the same-direction step already loading the next slot in charge.
fn step_advance(daemon: &mut DaemonLoop) {
    rebase_on_failed_step(daemon);
    // The queue length bounds the attempts (design D7): an advance that has
    // spent them ends with the output idle.
    let spent = {
        let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
            return;
        };
        cast.advance
            .as_ref()
            .is_none_or(|advance| advance.remaining == 0)
    };
    if spent {
        clear_advance(daemon);
        return;
    }
    let dispatched = {
        let mut ctx = super::owner_context(daemon);
        crate::play_order::dispatch_relative_step(&mut ctx, Direction::Next)
    };
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    match (dispatched, cast.advance.as_mut()) {
        (Some(request_id), Some(advance)) => {
            advance.remaining -= 1;
            advance.step_request = Some(request_id);
        }
        // The edge or a coalesced step ended the advance: the output stays
        // idle (design D7), or the step already loading the next slot takes
        // over.
        (None, _) => cast.advance = None,
        (Some(_), None) => {}
    }
}

/// Ends the advance without a step: the queue has no target for it.
fn clear_advance(daemon: &mut DaemonLoop) {
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    cast.advance = None;
}

/// Records `slot` as the owner's observed active slot (design D3's only path
/// for that record), forwarding the play order when the note re-picked the
/// Shuffle pick — the settle path's pattern.
fn observe_slot(daemon: &mut DaemonLoop, slot: QueueSlotId) {
    let crate::core::DaemonPlayerOwner { core, .. } = &mut daemon.owner;
    let order_before = core.play_order();
    core.note_observed_active_slot(Some(slot));
    if core.play_order() != order_before {
        crate::forward_play_order_advance(core, &daemon.player);
    }
}

/// Re-bases the shared step funnel on the slot the failed step targeted
/// (design D7's step-over): that slot becomes the desired active slot, so the
/// funnel steps from after it and the Shuffle pick re-picks past it. A failed
/// slot an edit removed ends the advance: the edited queue has no order past
/// it.
fn rebase_on_failed_step(daemon: &mut DaemonLoop) {
    let failed = {
        let crate::core::DaemonPlayerOwner { core, output, .. } = &mut daemon.owner;
        let PlaybackOutput::Cast(cast) = output else {
            return;
        };
        let Some(failed) = cast
            .advance
            .as_ref()
            .and_then(|advance| advance.failed_slot)
        else {
            return;
        };
        if core.queue.slot(failed).is_none() {
            cast.advance = None;
            return;
        }
        failed
    };
    observe_slot(daemon, failed);
}

/// Whether a failed resolve belonged to the advance's in-flight step (design
/// D7). Consumes the step record when it does, so the caller reports the
/// failure and either steps over or abandons the advance.
pub(crate) fn step_failed(daemon: &mut DaemonLoop, request_id: PlaybackRequestId) -> bool {
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return false;
    };
    match cast.advance.as_mut() {
        Some(advance) if advance.step_request == Some(request_id) => {
            advance.step_request = None;
            true
        }
        _ => false,
    }
}

/// Continues the advance past the failed step: the next step starts after the
/// slot the failed step targeted (design D7).
pub(crate) fn continue_after_failed_step(daemon: &mut DaemonLoop, failed_slot: QueueSlotId) {
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    match cast.advance.as_mut() {
        Some(advance) => advance.failed_slot = Some(failed_slot),
        None => return,
    }
    step_advance(daemon);
}

/// Ends the advance: a queued user transition the failed step's expiry
/// promoted took the queue over, so the advance stops stepping.
pub(crate) fn abandon(daemon: &mut DaemonLoop) {
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    cast.advance = None;
}
