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
//!
//! An Emby Session's device advance (emby-session-owner-output design D14) is
//! not a step: the device already moved to the queued slot, so
//! [`apply_device_advance`] follows it to that slot, records the old slot's
//! completion when the item ended naturally, and appends the slot after the
//! new active one with a `play_last`. A slot a queue edit made stale is never
//! adopted: [`correct_stale_device_advance`] records the same completion and
//! loads the queue's correct next slot, or stops the device when the queue has
//! none.

use std::time::Instant;

use mbv_ctrl::{Direction, PlaybackRequestId};
use mbv_ids::{EmbySessionId, ItemId};
use mbv_player::owner_state::StepTarget;
use mbv_queue::QueueSlotId;

use super::PlaybackOutput;
use crate::DaemonLoop;
use crate::emby_session::EmbySessionLoadedItem;
use crate::emby_session::end::DeviceAdvance;
use crate::emby_session::load::SessionAdmission;

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

/// Applies the device advance of design D14 (task 9.3): the Session moved from
/// the loaded item to the queued next one by itself, so the Owner follows it to
/// that slot. The four steps run in the design's order, and none of them loads
/// the active item again: the Owner sends no `PlayNow`, and it does not call
/// [`begin_advance`], which steps with one.
///
/// A queued slot a queue edit made stale is never adopted (task 9.4): the Owner
/// records the old slot's completion as step 2 does and corrects the device
/// instead ([`correct_stale_device_advance`]).
pub(crate) fn apply_device_advance(daemon: &mut DaemonLoop, advance: DeviceAdvance, now: Instant) {
    let Some(ended) = daemon.owner.output.loaded_slot() else {
        return;
    };
    if advance.stale {
        correct_stale_device_advance(daemon, advance, ended);
        return;
    }
    // 1. The observed active slot: the queued slot the device started, with the
    // Shuffle re-pick and the forward play order that follow it.
    observe_slot(daemon, advance.queued);
    // 2. A natural end records the old slot's completion and its consume rules
    // (design D6's end row); a device next before the end margin records none.
    if advance.completes {
        record_natural_end(daemon, ended, true);
    }
    // 3. The queued slot becomes the loaded item, so the end rule and the
    // position estimate hold the item the device plays now. The replaced item's
    // advance and its owed pause belong to the item the device left.
    reset_loaded_item(daemon, now);
    daemon.shared_queue.publish_observed(&daemon.owner.core);
    // 4. The device holds the active item alone now: append the next admissible
    // slot, so it never reaches its own next-episode autoplay (design D14).
    append_next_slot(daemon, advance.queued);
}

/// Corrects the device after it moved to a queued slot a queue edit made stale
/// (design D14 "the queue corrects the device", task 9.4): the Owner does not
/// adopt the stale slot. It records the old slot's completion as step 2 of the
/// device advance does, then loads the queue's correct next admissible slot
/// under the Load rule — that slot and the slot after it, with the new item's
/// resume position — or stops the device when the queue has no next admissible
/// slot, which leaves the queue on the old slot.
///
/// The correction registers no in-flight transition, because the Owner issued
/// it: nothing about it can coalesce with a user request. So the observation
/// that settles an ordinary load's transition never runs for it, and the
/// corrected slot is recorded as the observed active slot here, with the
/// published slot the non-stale device advance publishes (design D14 step 1,
/// 3). Without it a Next would step from the item the device left and Clients
/// would highlight the wrong row.
fn correct_stale_device_advance(
    daemon: &mut DaemonLoop,
    advance: DeviceAdvance,
    ended: QueueSlotId,
) {
    // The window is resolved while the old slot is still in the queue: the
    // completion record's consume rules can remove it, and the search has no
    // base to step from once the slot it names is gone.
    let (correct, stepped) = window(daemon, ended);
    if advance.completes {
        record_natural_end(daemon, ended, true);
    }
    let Some((slot, item_id)) = correct else {
        stop_stale_advance(daemon, ended);
        return;
    };
    // The Session the jobs name must exist before anything is recorded as
    // observed: a correction that cannot reach the device changes nothing.
    // Documented no-ops for Local and Cast: a device advance exists only on
    // the Session output (design D14), so no other output has a stale device
    // to correct and no session id to read.
    let session_id = match &daemon.owner.output {
        PlaybackOutput::Local | PlaybackOutput::Cast(_) => return,
        PlaybackOutput::EmbySession(output) => output.session_id.clone(),
    };
    let Some(session_id) = session_id else {
        tracing::warn!(name: "daemon.emby_session_advance.no_session", target: "daemon", slot = ?slot, "the Emby Session output has no session id; the device keeps the stale item");
        return;
    };
    // The corrected slot is where the device plays from now: the observation
    // comes before the window it is the base of, so a Shuffle re-pick is the
    // window the load sends.
    observe_slot(daemon, slot);
    let (after, after_stepped) = window(daemon, slot);
    let runtime_ticks = slot_runtime_ticks(&daemon.owner.core.queue, slot);
    let start_ticks =
        mbv_player::resume_ticks_for_slot(&daemon.owner.core.queue, slot).unwrap_or(0);
    let load = crate::emby_session::load::SessionLoad {
        slot,
        item_id: ItemId::new(item_id),
        runtime_ticks,
        session_id: EmbySessionId::new(session_id),
        start_ticks,
        next: after,
    };
    // Documented no-ops for Local and Cast: the correction addresses the
    // Session's device only (design D14), so no other output has a device to
    // submit the corrected load to and nothing is submitted.
    let submitted = match &mut daemon.owner.output {
        PlaybackOutput::EmbySession(output) => {
            crate::emby_session::load::submit_play(output, &daemon.merged_tx, load)
        }
        PlaybackOutput::Local | PlaybackOutput::Cast(_) => false,
    };
    if !submitted {
        return;
    }
    let mut stepped = stepped;
    stepped.extend(after_stepped);
    crate::emby_session::load::report_stepped_slots(&daemon.ctrl_clients, &stepped);
    let _ = daemon.owner.core.queue.set_active_slot(slot);
    daemon.shared_queue.publish_observed(&daemon.owner.core);
    if let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output {
        // The corrected load replaces the item the device holds, so the stale
        // item's advance and any pause a hand-off still owed go with it.
        output.advance = None;
        output.pending_pause = false;
    }
    tracing::info!(name: "daemon.emby_session_advance.corrected", target: "daemon", slot = ?slot, start_ticks, "the device moved to a stale queued item; the queue's next admissible slot was loaded");
}

/// Stops the device when the queue has no next admissible slot to correct it
/// with (design D14): the device must not play the item a queue edit made
/// stale, and the queue ends on the old slot. With the loaded item gone and the
/// stale observation dropped, the output presents nothing until the next
/// observation reports what the device does, as a user stop does.
fn stop_stale_advance(daemon: &mut DaemonLoop, ended: QueueSlotId) {
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    crate::emby_session::command::stop(output);
    output.loaded = None;
    output.queued_next = None;
    output.advance = None;
    output.pending_pause = false;
    output.last_observation = None;
    output.observed_at = None;
    tracing::info!(name: "daemon.emby_session_advance.stopped", target: "daemon", slot = ?ended, "the device moved to a stale queued item and the queue has no next admissible slot; the device was told to stop");
}

/// Makes the queued next item the loaded one after the device's own move
/// (design D14 step 3): its slot, item id, and runtime, with the observation
/// that showed it as its sighting, so no failure or end of the old item carries
/// over. The runtime caps the end estimate (design D6).
fn reset_loaded_item(daemon: &mut DaemonLoop, now: Instant) {
    // Documented no-ops for Local and Cast: the queued next item is the
    // record the Session's own `play_last` append keeps (design D14 step 4),
    // so no other output holds a slot to make loaded.
    let queued_slot = match &daemon.owner.output {
        PlaybackOutput::EmbySession(output) => {
            output.queued_next.as_ref().map(|queued| queued.slot)
        }
        PlaybackOutput::Local | PlaybackOutput::Cast(_) => None,
    };
    let Some(slot) = queued_slot else {
        return;
    };
    let runtime_ticks = slot_runtime_ticks(&daemon.owner.core.queue, slot);
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    let Some(queued) = output.queued_next.take() else {
        return;
    };
    output.loaded = Some(EmbySessionLoadedItem {
        slot,
        item_id: queued.item_id,
        runtime_ticks,
        loaded_at: now,
        seen_playing: true,
        end_handled: false,
        failure_reported: false,
    });
    output.advance = None;
    output.pending_pause = false;
}

/// Appends the slot after the new active one to the device (design D14 step 4):
/// one `play_last` with its item id, recorded as the queued next item. With no
/// slot the Session can play after the active one, nothing is queued, and each
/// slot the search stepped past is reported only when a slot was sent.
fn append_next_slot(daemon: &mut DaemonLoop, active: QueueSlotId) {
    let (next, stepped) = window(daemon, active);
    let Some((slot, item_id)) = next else {
        return;
    };
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    if crate::emby_session::load::submit_play_last(output, slot, item_id) {
        crate::emby_session::load::report_stepped_slots(&daemon.ctrl_clients, &stepped);
    }
}

/// The runtime of the Emby item in `slot`, in ticks; 0 when the slot is gone or
/// holds no Emby item.
fn slot_runtime_ticks(queue: &mbv_queue::PlaybackQueue, slot: QueueSlotId) -> i64 {
    queue
        .slot(slot)
        .and_then(|slot| slot.item.as_emby())
        .map_or(0, |item| item.runtime_ticks)
}

/// The window the queue puts after `active` on the Session output (design D14):
/// the next admissible slot and its Emby item id, with the slots the search
/// stepped past. It is the window a load, an append, or a correction sends.
fn window(
    daemon: &DaemonLoop,
    active: QueueSlotId,
) -> (Option<(QueueSlotId, String)>, Vec<SessionAdmission>) {
    let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
        return (None, Vec::new());
    };
    crate::emby_session::load::next_admissible_slot(
        &daemon.owner.core,
        active,
        &output.playable_media_types,
    )
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
///
/// The Emby Session's admission is synchronous (design D6, D7), so it runs
/// here, before the dispatch: a rejection inside the dispatch could never
/// match the step record this function writes after it returns, and the
/// advance would wedge. A rejected target is reported and the next one is
/// picked from after it, bounded by the queue length — the same report and
/// step again a Cast resolve failure drives.
fn step_target_advance(daemon: &mut DaemonLoop, target: StepTarget) {
    let StepTarget::Jump(slot_id) = target else {
        // The edge or a coalesced step ended the advance: the output stays
        // idle (design D6), or the step already loading the next slot takes
        // over.
        clear_advance(daemon);
        return;
    };
    if let Some(rejected) = unplayable_step_target(daemon, slot_id) {
        report_step_rejection(daemon, &rejected);
        step_past_rejected_target(daemon, slot_id);
        return;
    }
    let request_id = {
        let mut ctx = crate::cast::owner_context(daemon);
        crate::step::dispatch_step_target(&mut ctx, slot_id, Direction::Next)
    };
    if let Some(advance) = daemon.owner.output.advance_mut() {
        advance.remaining -= 1;
        advance.step_request = Some(request_id);
    }
}

/// The reason the active output cannot play `slot`, when it is an Emby Session
/// and the item fails its admission (design D6, D7). Only the Emby Session
/// needs this check ahead of the dispatch: its load reports a rejection
/// synchronously, while a Cast resolve failure re-enters the loop as an event
/// and reaches `step_failed`.
fn unplayable_step_target(daemon: &DaemonLoop, slot: QueueSlotId) -> Option<SessionAdmission> {
    let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
        return None;
    };
    let item = &daemon.owner.core.queue.slot(slot)?.item;
    crate::emby_session::load::admit(item, &output.playable_media_types).err()
}

/// Reports a step target the Session cannot play (design D6, D7). The owner
/// issued the step itself, so the item and the reason go to every Client, as
/// a Cast advance's uncastable slot is reported.
fn report_step_rejection(daemon: &DaemonLoop, rejected: &SessionAdmission) {
    tracing::warn!(
        name: "daemon.emby_session_advance.step_over",
        target: "daemon",
        item = %rejected.item,
        reason = %rejected.reason,
        "the advance's step target cannot play on the Emby Session; stepping over it"
    );
    crate::core::broadcast(
        &daemon.ctrl_clients,
        &mbv_ctrl::CtrlEvent::CommandRejected(format!("{} ({})", rejected.item, rejected.reason)),
    );
}

/// Steps past a slot the Session cannot play (design D6, D7): the attempt is
/// spent, so the queue length still bounds the loop, and the next target is
/// resolved from after the rejected slot.
fn step_past_rejected_target(daemon: &mut DaemonLoop, slot_id: QueueSlotId) {
    let remaining = match daemon.owner.output.advance_mut() {
        Some(advance) => {
            advance.remaining = advance.remaining.saturating_sub(1);
            advance.remaining
        }
        None => return,
    };
    if remaining == 0 {
        clear_advance(daemon);
        return;
    }
    continue_after_failed_step(daemon, slot_id);
}

/// Ends the advance without a step: the queue has no target for it, or a
/// queued user transition the failed step's expiry promoted took the queue
/// over.
pub(crate) fn clear_advance(daemon: &mut DaemonLoop) {
    daemon.owner.output.set_advance(None);
}

/// Records `slot` as the owner's observed active slot (the only path for that
/// record) — the settle path's pattern.
fn observe_slot(daemon: &mut DaemonLoop, slot: QueueSlotId) {
    let crate::core::DaemonPlayerOwner { core, .. } = &mut daemon.owner;
    core.note_observed_active_slot(Some(slot));
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
