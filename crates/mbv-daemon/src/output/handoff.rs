//! The hand-off of active playback between the Owner's playback outputs
//! (emby-session-owner-output design D11, task 6.1; cast-owner-output design
//! D10): one sequence for every pair of outputs. A selection captures the old
//! output's active playback at the selection instant; the new output is made
//! ready first; the old output then stops; and the captured slot continues on
//! the new output at the captured position, paused when the old output was
//! paused.
//!
//! The capture is per output kind — the local run's observed slot, the Cast
//! receiver's loaded item, or the Emby Session's loaded item while its device
//! plays it — and every continuation goes through the same helpers.

use std::time::Instant;

use mbv_ctrl::CtrlEvent;
use mbv_queue::QueueSlotId;

use crate::CtrlSender;
use crate::DaemonLoop;
use crate::cast::PlaybackOutput;
use crate::cast::{broadcast_output_change, queue_teardown_job, stop_active_output};
use crate::send_to;

/// The active playback a selection captures from the old output for the
/// hand-off (design D10 step 1, row 7.1): the slot, its position, and its
/// pause state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HandoffPlayback {
    pub(crate) slot: QueueSlotId,
    pub(crate) start_ticks: i64,
    pub(crate) paused: bool,
}

/// Captures the active playback of whichever output is current (design D10
/// step 1, row 7.1): the local run, the Cast receiver, or the Emby Session.
/// An idle output, a stopped output, and one that presents only content the
/// Owner did not load hand nothing over.
pub(crate) fn capture_handoff_for_selection(daemon: &DaemonLoop) -> Option<HandoffPlayback> {
    match &daemon.owner.output {
        PlaybackOutput::Local => capture_local_handoff(daemon),
        PlaybackOutput::Cast(_) => capture_cast_handoff(daemon),
        PlaybackOutput::EmbySession(_) => capture_emby_session_handoff(daemon),
    }
}

/// Pins the active remote output's presented status to the selection instant
/// (design D10 step 1, row 7.1 dispatch B), so the hand-off capture reads the
/// position at the selection without a fresh poll. A local output needs no
/// re-derivation: its status is the run's own observation.
pub(crate) fn refresh_active_output_presentation(daemon: &mut DaemonLoop, now: Instant) {
    match &daemon.owner.output {
        PlaybackOutput::Local => {}
        PlaybackOutput::Cast(_) => crate::cast::refresh_presentation(daemon, now),
        PlaybackOutput::EmbySession(_) => {
            crate::emby_session::status::refresh_presentation(daemon, now);
        }
    }
}

/// Captures the Local output's active playback (design D10 step 1, row 7.1):
/// the run-observed active slot — the queue's own marker when no run
/// observation exists yet — with the presented position and pause state. A
/// slot counts as active only while the output plays or pauses it, so a
/// stopped or idle output hands nothing over.
fn capture_local_handoff(daemon: &DaemonLoop) -> Option<HandoffPlayback> {
    let (active, position_ticks, paused) = {
        let status = daemon.player.status.lock().unwrap();
        (status.active, status.position_ticks, status.paused)
    };
    if !active {
        return None;
    }
    let slot = daemon
        .owner
        .core
        .observed_active_slot()
        .or_else(|| daemon.owner.core.queue.active_slot_id())?;
    Some(HandoffPlayback {
        slot,
        start_ticks: position_ticks,
        paused,
    })
}

/// Hands `slot` over at the presented position and pause state, while the
/// output presents it as playing. An idle or stopped output hands nothing over.
fn presented_handoff(daemon: &DaemonLoop, slot: QueueSlotId) -> Option<HandoffPlayback> {
    let status = daemon.player.status.lock().unwrap();
    status.active.then(|| HandoffPlayback {
        slot,
        start_ticks: status.position_ticks,
        paused: status.paused,
    })
}

/// Captures the Cast output's active playback (design D10 step 1, row 7.1
/// dispatch B): the loaded slot with the presented position and pause state —
/// the last receiver report extrapolated, never a fresh poll. The presented
/// status is active only while the receiver plays, buffers, or pauses the
/// loaded item (design D8, row 6.1), so an idle, stopped, or foreign-playing
/// receiver hands nothing over.
fn capture_cast_handoff(daemon: &DaemonLoop) -> Option<HandoffPlayback> {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return None;
    };
    let loaded = cast.loaded.as_ref()?;
    presented_handoff(daemon, loaded.slot)
}

/// Captures the Emby Session output's active playback (design D11): the loaded
/// slot while the device plays it, with the presented position estimate and
/// pause state. A device that plays content the Owner did not load hands
/// nothing over: its playback is left running (design D11), and only the
/// loaded item is the Owner's to continue elsewhere.
fn capture_emby_session_handoff(daemon: &DaemonLoop) -> Option<HandoffPlayback> {
    let PlaybackOutput::EmbySession(session) = &daemon.owner.output else {
        return None;
    };
    let loaded = session.loaded.as_ref()?;
    if !crate::emby_session::presents_item(session.last_observation.as_ref(), &loaded.item_id) {
        return None;
    }
    presented_handoff(daemon, loaded.slot)
}

/// Stops the old output for a successful swap (design D10 step 3): the local
/// run stops; an old receiver's media stops with its stop reports, and the
/// deliberate output change also tears its app down (design D12) — on the old
/// receiver, never the new one; an old Emby Session sends its device a stop
/// only while the device plays the loaded item (design D11). Called whether or
/// not a hand-off was captured, because every swap stops the old output.
pub(crate) fn stop_old_output_for_handoff(daemon: &mut DaemonLoop) {
    let old_output_is_cast = matches!(&daemon.owner.output, PlaybackOutput::Cast(_));
    stop_active_output(
        &daemon.player,
        &mut daemon.owner.output,
        &daemon.owner.core.queue,
        &daemon.ctrl_clients,
    );
    if old_output_is_cast {
        teardown_old_receiver_app(daemon);
    }
}

/// Queues the app teardown on the old receiver when it was the output
/// (design D12): only a deliberate output change tears the app down. The
/// worker's FIFO order runs it behind any media stop queued first.
fn teardown_old_receiver_app(daemon: &DaemonLoop) {
    if let PlaybackOutput::Cast(old) = &daemon.owner.output {
        queue_teardown_job(old);
    }
}

/// The remote→Local hand-off (design D10 steps 3-5, row 7.1 dispatch B, design
/// D11): the remote output's presented playback is captured, the old output
/// stops — a receiver's media-stop job first, then its app teardown (design
/// D12: only a deliberate output change tears the app down; a user Stop never
/// does), or the Emby Session's device stop when it plays the loaded item — the
/// output swaps to Local and broadcasts, and a captured slot continues on a
/// fresh local run at the captured position, paused when the old output was
/// paused (row 7.2). The stop reports go out before the local run starts
/// (design D9).
pub(crate) fn hand_remote_output_back_to_local(daemon: &mut DaemonLoop, reply_tx: &CtrlSender) {
    let handoff = {
        refresh_active_output_presentation(daemon, Instant::now());
        capture_handoff_for_selection(daemon)
    };
    stop_old_output_for_handoff(daemon);
    // A stop makes every awaited transition identity unobservable (the same
    // rule the ctrl stop applies), so the fresh run starts with a clear gate.
    crate::core::reset_slot_jumps(
        &mut daemon.owner.core.transitions,
        &mut daemon.owner.queued_transition_origin,
    );
    daemon.owner.output = PlaybackOutput::Local;
    broadcast_output_change(daemon);
    if let Some(handoff) = handoff {
        continue_slot_on_local(daemon, handoff, reply_tx);
    }
}

/// Continues a handed-off slot on a fresh local run (design D10 step 5, row
/// 7.1 dispatch B; row 7.2). A local cold start takes no start position of
/// its own: the resume position is the slot item's recorded progress
/// (`resume_ticks_for_slot` reads it at dispatch), so the captured remote
/// position is recorded on the slot first and the cold start resumes from
/// it — the same position path a stopped local run leaves behind. A local
/// cold start also cannot begin paused, so the run starts and an explicit
/// pause follows it; the run serves the pause after its initial load, so
/// the loaded item starts paused. A slot edited out of the queue continues
/// nothing; a submission the player refuses is reported to the selecting
/// Client.
fn continue_slot_on_local(
    daemon: &mut DaemonLoop,
    handoff: HandoffPlayback,
    reply_tx: &CtrlSender,
) {
    let Some(slot) = daemon.owner.core.queue.slot(handoff.slot) else {
        tracing::warn!(
            name: "daemon.handoff.slot_missing",
            target: "daemon",
            slot = ?handoff.slot,
            "the handed-off slot is absent; nothing was started"
        );
        return;
    };
    let played = slot.local_progress().played;
    let _ = daemon
        .owner
        .core
        .queue
        .apply_progress(handoff.slot, handoff.start_ticks, played);
    let Some(start_idx) = daemon.owner.core.queue.slot_index(handoff.slot) else {
        return;
    };
    tracing::info!(
        name: "daemon.handoff.local_start",
        target: "daemon",
        slot = ?handoff.slot,
        position_ticks = handoff.start_ticks,
        paused = handoff.paused,
        "the handed-off slot continues on a fresh local run"
    );
    if !crate::core::submit_queue_slots_for_replace(
        &daemon.player,
        daemon.owner.core.queue.slot_pairs(),
        start_idx,
        &daemon.client,
    ) {
        tracing::warn!(
            name: "daemon.handoff.local_start.rejected",
            target: "daemon",
            slot = ?handoff.slot,
            "the local cold-start submission was rejected"
        );
        send_to(
            reply_tx,
            &CtrlEvent::CommandRejected(
                "starting local playback for the handed-off slot failed".to_string(),
            ),
        );
        return;
    }
    // The queue's own active marker follows the handed-off slot and the old
    // output's observation is dropped — the fresh run's own observation
    // becomes playback truth (the same bookkeeping the cold-start slot jump
    // applies).
    let _ = daemon.owner.core.queue.set_active_slot(handoff.slot);
    daemon.owner.core.note_observed_active_slot(None);
    daemon.shared_queue.publish_observed(&daemon.owner.core);
    crate::broadcast_queue_state(
        &daemon.ctrl_clients,
        &daemon.player,
        &daemon.shared_queue,
        &daemon.owner.core.queue,
        &daemon.owner.core.source,
        &daemon.owner.core.transitions,
        None,
    );
    if handoff.paused {
        // A local cold start cannot begin paused (row 7.2): the run starts
        // and the pause follows it.
        daemon.player.set_paused(true);
    }
}
