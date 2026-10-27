//! Slot-jump transition dispatch: the owner's one-in-flight gate, the
//! cold-start fallback, settling and expiring transitions, and promoting a
//! queued transition (cast-owner-output design D4/D5).

use std::time::Instant;

use mbv_ctrl::{CtrlEvent, PlaybackIntentEvent, PlaybackIntentOutcome, PlaybackRequestId};
use mbv_player::PlayerOwnerState;
use mbv_queue::QueueSlotId;

use crate::control_queue::broadcast_queue_state;
use crate::ctrl::CtrlClientId;

use super::{DaemonOwnerContext, JumpOrigin, submit_queue_slots_for_replace};

/// Deliver one accepted transition's `JumpTo` to the Player. When no Playback
/// run is alive the bare command would be dropped silently, so fall back to
/// the same cold-start submission `handle_queue_replace` uses: point the
/// canonical queue's active marker at the target slot, drop the dead run's
/// stale observation, and submit the whole queue starting at the target slot,
/// which spawns a fresh run. The accepted transition and its queued origin are
/// reset on that path because a cold run's own `TrackChanged` observation
/// carries the run identity, never this request identity — an in-flight entry
/// would otherwise linger until its expire deadline and wedge every later jump
/// behind it (queue-owner-process #857).
/// Returns whether playback delivery started (live jump, cold start, or Cast
/// load). On a Cast output the jump never reaches the local run: the target
/// slot is resolved off-thread and loaded on the receiver, and the accepted
/// transition stays in flight until a receiver status settles it (design
/// D4/D5).
pub(crate) fn send_jump_or_cold_start(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: mbv_player::transition::Transition,
    origin: Option<CtrlClientId>,
) -> bool {
    if let Some(started) = dispatch_remote_load(ctx, transition, origin) {
        return started;
    }
    let target = transition.target;
    let resume_ticks = mbv_player::resume_ticks_for_slot(&ctx.owner.core.queue, target);
    if ctx.player.send_command(transition.into_jump(resume_ticks)) {
        return true;
    }
    // No live run took the jump: the fresh run's own observation becomes
    // playback truth, so this request can never settle and must not stay
    // in flight.
    reset_slot_jumps(
        &mut ctx.owner.core.transitions,
        &mut ctx.owner.queued_transition_origin,
    );
    cold_start_at_slot(ctx, target)
}

/// Loads an accepted transition's target on a remote output, which holds one
/// item at a time (design D4, D5, D7): the Cast receiver resolves the target
/// slot off-thread and loads it, and the Emby Session admits the target and
/// loads it, so neither starts the local run. Returns `None` when the output is
/// the local run, which takes the jump itself.
fn dispatch_remote_load(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: mbv_player::transition::Transition,
    origin: Option<CtrlClientId>,
) -> Option<bool> {
    match ctx.owner.output {
        crate::cast::PlaybackOutput::Cast(_) => {
            let resume_ticks =
                mbv_player::resume_ticks_for_slot(&ctx.owner.core.queue, transition.target)
                    .unwrap_or(0);
            Some(crate::cast::dispatch_cast_load(
                ctx,
                transition,
                resume_ticks,
                origin,
            ))
        }
        crate::cast::PlaybackOutput::EmbySession(_) => {
            let resume_ticks =
                mbv_player::resume_ticks_for_slot(&ctx.owner.core.queue, transition.target)
                    .unwrap_or(0);
            Some(crate::emby_session::dispatch(
                ctx,
                transition,
                resume_ticks,
                origin,
            ))
        }
        crate::cast::PlaybackOutput::Local => None,
    }
}

/// Cold-starts the queue at `target` after the jump found no live run: point
/// the canonical queue's active marker at the target slot, drop the dead run's
/// stale observation, and submit the whole queue starting at the target slot,
/// which spawns a fresh run. Returns whether the submission was accepted.
fn cold_start_at_slot(ctx: &mut DaemonOwnerContext<'_>, target: QueueSlotId) -> bool {
    let Some(start_idx) = ctx.owner.core.queue.slot_index(target) else {
        tracing::warn!(name: "daemon.transition_dispatch.cold_start.skipped", target: "transition", slot = ?target, "no live playback run and the target slot is absent; jump dropped");
        return false;
    };
    tracing::info!(name: "daemon.transition_dispatch.cold_start", target: "transition", slot = ?target, start_idx, "no live playback run; cold-starting the queue at the target slot");
    if !submit_queue_slots_for_replace(
        ctx.player,
        ctx.owner.core.queue.slot_pairs(),
        start_idx,
        ctx.client,
    ) {
        tracing::warn!(name: "daemon.transition_dispatch.cold_start.rejected", target: "transition", slot = ?target, "the cold-start submission was rejected");
        return false;
    }
    // Mirror `handle_queue_replace`'s cold-start bookkeeping: the queue's own
    // active marker follows the requested slot and the dead run's observation
    // is dropped, so the broadcast that follows resolves the requested slot
    // as active instead of the restored cursor's slot.
    let _ = ctx.owner.core.queue.set_active_slot(target);
    ctx.owner.core.note_observed_active_slot(None);
    ctx.shared_queue.publish_observed(&ctx.owner.core);
    true
}

pub(crate) fn dispatch_slot_jump(
    ctx: &mut DaemonOwnerContext<'_>,
    origin: JumpOrigin,
    transition: mbv_player::transition::Transition,
    except: Option<CtrlClientId>,
) {
    let transition_target = transition.target;
    let transition_request_id = transition.request_id;
    let transition_generation = transition.generation;
    let origin_client = match origin {
        JumpOrigin::Ctrl(client_id) => Some(client_id),
        JumpOrigin::Transport => None,
    };
    let decision = ctx.owner.core.transitions.accept(transition);
    match decision {
        mbv_player::transition::DispatchDecision::DispatchNow(t) => {
            tracing::info!(name: "daemon.transition_dispatch.started", target: "transition", target_slot = ?transition_target, request = %transition_request_id, generation = %transition_generation, "slot jump dispatched");
            send_jump_or_cold_start(ctx, t, origin_client);
        }
        mbv_player::transition::DispatchDecision::Queued { superseded } => {
            tracing::info!(name: "daemon.transition_dispatch.queued", target: "transition", target_slot = ?transition_target, request = %transition_request_id, generation = %transition_generation, "slot jump queued");
            if let Some(s) = superseded
                && let Some((origin_request_id, origin_client)) = ctx.owner.queued_transition_origin
                && origin_request_id == s.request_id
            {
                ctx.ctrl_clients.lock().unwrap().send_to_client(
                    origin_client,
                    &CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                        request_id: s.request_id,
                        generation: s.generation,
                        outcome: PlaybackIntentOutcome::Superseded,
                    }),
                );
            }
            // Transport senders (MPRIS/tray/Emby-ws) have no ctrl wire form and
            // so no `Superseded` event to receive; only `Ctrl` origins are worth
            // tracking here.
            ctx.owner.queued_transition_origin = match origin {
                JumpOrigin::Ctrl(client_id) => Some((transition.request_id, client_id)),
                JumpOrigin::Transport => None,
            };
        }
    }
    // Accepting a transition mutates desired playback state (in_flight /
    // queued_latest); publish the coherent snapshot so Clients can render the
    // pending slot before it settles (task 4.1, design D5).
    let PlayerOwnerState {
        queue,
        source,
        transitions,
        ..
    } = &ctx.owner.core;
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
        except,
    );
}

/// Drop any in-flight/queued transition: the caller is issuing a
/// queue-replacing playback command, which deliberately interrupts them.
pub(crate) fn reset_slot_jumps(
    transitions: &mut mbv_player::transition::OwnerTransitionState,
    queued_origin: &mut Option<(PlaybackRequestId, CtrlClientId)>,
) {
    transitions.reset();
    *queued_origin = None;
}

/// The origin client of a promoted queued transition, when the tracked
/// origin's request matches it. Cleared with the dispatch either way.
pub(crate) fn take_promoted_origin(
    ctx: &mut DaemonOwnerContext<'_>,
    promoted: Option<&mbv_player::transition::Transition>,
) -> Option<CtrlClientId> {
    let origin = match ctx.owner.queued_transition_origin {
        Some((request_id, client_id)) if promoted.is_some_and(|t| t.request_id == request_id) => {
            Some(client_id)
        }
        _ => None,
    };
    ctx.owner.queued_transition_origin = None;
    origin
}

/// Settle the in-flight transition one remote output's observation of the
/// loaded item closes (cast-owner-output row 5.2, emby-session-owner-output
/// design D6/D7): the loaded slot becomes the observed active slot, the
/// transition settles, a queued transition the settle promotes is dispatched,
/// and the guarded play intent the load carried is applied. This mirrors the
/// run's `TrackChanged` handling (observe → settle → publish) for both remote
/// outputs: the Cast status poll and the Emby Session `/Sessions` poll.
pub(crate) fn settle_remote_transition(
    daemon: &mut crate::event_loop::DaemonLoop,
    loaded_slot: QueueSlotId,
) {
    // Only the transition the loaded item belongs to settles here: a newer
    // in-flight transition targets a slot the output has not confirmed.
    let Some(in_flight) = daemon
        .owner
        .core
        .transitions
        .in_flight()
        .filter(|t| t.target == loaded_slot)
    else {
        return;
    };
    let mut ctx = crate::cast::owner_context(daemon);
    ctx.owner.core.note_observed_active_slot(Some(loaded_slot));
    settle_and_redispatch(&mut ctx, in_flight.request_id, loaded_slot);
    // The guarded play intent the load registered under settles with its
    // transition; without this a remote play would sit unresolved and coalesce
    // every later play from its connection.
    if let Some((connection_id, request_id, generation)) = ctx
        .owner
        .intents
        .current
        .as_ref()
        .filter(|current| {
            current.request_id == in_flight.request_id
                && matches!(current.action, mbv_ctrl::PlaybackIntentAction::Play { .. })
        })
        .map(|current| {
            (
                current.connection_id,
                current.request_id,
                current.generation,
            )
        })
        && let Some(event) =
            ctx.owner
                .intents
                .applied_if_current(connection_id, request_id, generation)
    {
        ctx.ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
    }
    ctx.shared_queue.publish_observed(&ctx.owner.core);
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
}

/// Settle the in-flight transition against a Playback-run observation and, if
/// a newer transition was queued behind it, dispatch that one now (task 3.3).
pub(crate) fn settle_and_redispatch(
    ctx: &mut DaemonOwnerContext<'_>,
    observed_request_id: PlaybackRequestId,
    observed_slot: QueueSlotId,
) {
    let outcome = ctx
        .owner
        .core
        .transitions
        .settle(observed_request_id, observed_slot);
    let mbv_player::transition::SettleOutcome::Settled { dispatch_next } = outcome else {
        tracing::info!(name: "daemon.transition_settle.completed", target: "transition", request = %observed_request_id, slot = ?observed_slot, settled = false, dispatch_next = false, "transition settled");
        return;
    };
    tracing::info!(name: "daemon.transition_settle.completed", target: "transition", request = %observed_request_id, slot = ?observed_slot, settled = true, dispatch_next = dispatch_next.is_some(), "transition settled");
    let origin = take_promoted_origin(ctx, dispatch_next.as_ref());
    if let Some(next) = dispatch_next {
        send_jump_or_cold_start(ctx, next, origin);
    }
}

/// Bounded in-flight failure handling (task 3.4): if the Playback run has not
/// confirmed the in-flight transition before its deadline, abandon it, report a
/// timeout to its origin ctrl client, and dispatch whatever was queued behind
/// it — rebuilding the execution projection from `owner.core.queue` exactly as
/// [`settle_and_redispatch`] does.
pub(crate) fn expire_and_redispatch(ctx: &mut DaemonOwnerContext<'_>) {
    let outcome = ctx.owner.core.transitions.expire(Instant::now());
    let mbv_player::transition::ExpireOutcome::Expired {
        expired,
        dispatch_next,
    } = outcome
    else {
        return;
    };
    // Emit a timeout to the abandoned request's origin when it is the current
    // guarded intent (Next/Previous). Owner-minted jumps carry no ctrl origin
    // and need no event.
    if let Some((connection_id, request_id, generation)) = ctx
        .owner
        .intents
        .current
        .as_ref()
        .filter(|current| current.request_id == expired.request_id)
        .map(|current| {
            (
                current.connection_id,
                current.request_id,
                current.generation,
            )
        })
        && let Some(event) = ctx.owner.intents.rejected_if_current(
            connection_id,
            request_id,
            generation,
            mbv_ctrl::PlaybackIntentRejection::Unavailable,
        )
    {
        ctx.ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
    }
    let origin = take_promoted_origin(ctx, dispatch_next.as_ref());
    if let Some(next) = dispatch_next {
        send_jump_or_cold_start(ctx, next, origin);
    }
    // Abandoning / promoting a transition changed desired state; republish.
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
}
