//! Loading one item onto the Emby Session output (emby-session-owner-output
//! design D7, task 4.2): the admission rule, the queue's active marker, the
//! in-flight transition registration, and the one `play` job the device
//! receives. The device fetches its own stream, so there is no resolve step:
//! the Owner sends the item id and the position, never the queue.

use std::time::Instant;

use mbv_ctrl::{CtrlEvent, PlaybackIntentRejection};
use mbv_queue::QueueItem;

use super::EmbySessionLoadedItem;
use super::worker::{EmbySessionJob, SessionApi};
use crate::DaemonEvent;
use crate::cast::PlaybackOutput;
use crate::core::DaemonOwnerContext;
use crate::ctrl::CtrlClientId;
use crate::emby_session::EmbySessionEvent;

/// Why a slot cannot play on an Emby Session output (design D7): the item's
/// name for the report, and the reason. A rejected slot is reported, never
/// silently skipped at a funnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionAdmission {
    pub(crate) item: String,
    pub(crate) reason: String,
}

/// Whether the Session can play `item` (design D7). Only an Emby item the
/// Session advertises its media kind for passes; a Session that advertises no
/// kinds is treated as able to play every Emby item, because an empty list is
/// unknown capability rather than an audio-only Session. Every other item
/// fails with its reason, so the caller can name it.
pub(crate) fn admit<'a>(
    item: &'a QueueItem,
    playable_media_types: &[String],
) -> Result<&'a mbv_emby_model::EmbyItem, SessionAdmission> {
    let QueueItem::Emby(emby_item) = item else {
        return Err(SessionAdmission {
            item: item.display_name(),
            reason: "the Emby Session output plays only Emby items".to_string(),
        });
    };
    let kind = emby_item.media_type.as_str();
    if playable_media_types.is_empty()
        || playable_media_types
            .iter()
            .any(|advertised| advertised.eq_ignore_ascii_case(kind))
    {
        return Ok(emby_item);
    }
    Err(SessionAdmission {
        item: item.display_name(),
        reason: format!("the Session does not play {kind} items"),
    })
}

/// Registers the transition a funnel's load belongs to and dispatches it
/// (design D7). A transition that queues behind another loads nothing, as on
/// Cast: the queued one is dispatched when the in-flight one settles.
pub(crate) fn dispatch_accepted(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: mbv_player::transition::Transition,
    start_ticks: i64,
    origin: Option<CtrlClientId>,
) -> bool {
    match ctx.owner.core.transitions.accept(transition) {
        mbv_player::transition::DispatchDecision::DispatchNow(accepted) => {
            dispatch(ctx, accepted, start_ticks, origin)
        }
        mbv_player::transition::DispatchDecision::Queued { .. } => {
            tracing::warn!(name: "daemon.emby_session_load.queued", target: "daemon", slot = ?transition.target, "the load's transition queues behind another; nothing was loaded");
            false
        }
    }
}

/// Loads the transition's target slot onto the Session (design D7): admit the
/// item, point the queue's active marker at the slot, send one `play` job for
/// that item alone, and record it as the loaded item. A slot the Session
/// cannot play is reported and expires the in-flight transition, so the gate
/// cannot stay wedged behind a load that never happened. Returns whether the
/// play job was submitted.
pub(crate) fn dispatch(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: mbv_player::transition::Transition,
    start_ticks: i64,
    origin: Option<CtrlClientId>,
) -> bool {
    let target = transition.target;
    let Some(slot) = ctx.owner.core.queue.slot(target) else {
        tracing::warn!(name: "daemon.emby_session_load.slot_missing", target: "daemon", slot = ?target, "the load target slot is absent; nothing was loaded");
        return false;
    };
    let item = slot.item.clone();
    let PlaybackOutput::EmbySession(output) = &ctx.owner.output else {
        tracing::warn!(name: "daemon.emby_session_load.not_the_output", target: "daemon", slot = ?target, "the playback output is not an Emby Session; nothing was loaded");
        return false;
    };
    let admitted = match admit(&item, &output.playable_media_types) {
        Ok(emby_item) => emby_item,
        Err(rejected) => {
            report_rejection(ctx, &transition, origin, &rejected);
            crate::cast::expire_remote_transition(ctx);
            return false;
        }
    };
    let item_id = admitted.id.clone();
    let runtime_ticks = admitted.runtime_ticks;
    let Some(session_id) = output.session_id.clone() else {
        tracing::warn!(name: "daemon.emby_session_load.no_session", target: "daemon", slot = ?target, "the Emby Session output has no session id; nothing was loaded");
        return false;
    };
    let job_item_id = item_id.clone();
    let merged_tx = ctx.merged_tx.clone();
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        if let Err(error) = api.play(&session_id, &job_item_id, start_ticks) {
            // A failed play expires the load and reports the item once (design
            // D6, D7; task 4.3): the outcome re-enters the owner's loop as an
            // `EmbySession` event, so the report and the expiry run where the
            // transition lives.
            let _ = merged_tx.send(DaemonEvent::EmbySession(EmbySessionEvent::PlayFailed {
                slot: target,
                error,
            }));
        }
    });
    let PlaybackOutput::EmbySession(output) = &mut ctx.owner.output else {
        return false;
    };
    if output.worker.send(job).is_err() {
        tracing::warn!(name: "daemon.emby_session_load.worker_gone", target: "daemon", slot = ?target, "the Emby Session worker is not running; nothing was loaded");
        return false;
    }
    let _ = ctx.owner.core.queue.set_active_slot(target);
    // The new load replaces the item the output had: the end an earlier item
    // started belongs to the replaced load, so it ends here (design D6), and a
    // pause a hand-off still owed belongs to that replaced item too — the new
    // load starts playing (review r6 P2-2).
    output.advance = None;
    output.pending_pause = false;
    output.loaded = Some(EmbySessionLoadedItem {
        slot: target,
        item_id,
        runtime_ticks,
        loaded_at: Instant::now(),
        seen_playing: false,
        end_handled: false,
        failure_reported: false,
    });
    tracing::info!(name: "daemon.emby_session_load.started", target: "daemon", slot = ?target, start_ticks, "the Emby Session was given one item to play");
    true
}

/// Reports a slot the Session cannot play (design D7): the item and the reason
/// reach the client that issued the play or jump; when the owner itself issued
/// the step (no origin), every client is told. A current guarded intent keyed
/// to the rejected transition is rejected too, so the guarded protocol cannot
/// wedge behind it.
fn report_rejection(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: &mbv_player::transition::Transition,
    origin: Option<CtrlClientId>,
    rejected: &SessionAdmission,
) {
    tracing::warn!(name: "daemon.emby_session_load.unplayable", target: "daemon", slot = ?transition.target, item = %rejected.item, reason = %rejected.reason, "the slot's item cannot play on the Emby Session; nothing was loaded");
    let report = format!("{} ({})", rejected.item, rejected.reason);
    if let Some((connection_id, request_id, generation)) = ctx
        .owner
        .intents
        .current
        .as_ref()
        .filter(|current| {
            current.request_id == transition.request_id
                && current.generation == transition.generation
        })
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
            PlaybackIntentRejection::ResolutionFailed,
        )
    {
        ctx.ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
    }
    match origin {
        Some(connection_id) => {
            ctx.ctrl_clients
                .lock()
                .unwrap()
                .send_to_client(connection_id, &CtrlEvent::CommandRejected(report));
        }
        None => crate::core::broadcast(ctx.ctrl_clients, &CtrlEvent::CommandRejected(report)),
    }
}
