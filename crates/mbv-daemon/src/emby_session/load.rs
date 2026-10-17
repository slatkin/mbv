//! Loading the device's window onto the Emby Session output
//! (emby-session-owner-output design D7 and D14, tasks 4.2 and 9.2): the
//! admission rule, the queue's active marker, the in-flight transition
//! registration, and the one `play` job the device receives. The device
//! fetches its own stream, so there is no resolve step: the Owner sends item
//! ids and the position, never the queue. The job carries the active item and
//! the next admissible slot (design D14), so the device never reaches its own
//! next-episode autoplay while the queue has a slot after the active one.

use std::sync::mpsc;
use std::time::Instant;

use mbv_ctrl::CtrlEvent;
use mbv_player::PlayerOwnerState;
use mbv_queue::{PlayOrder, QueueItem, QueueSlotId};

use super::worker::{EmbySessionJob, SessionApi};
use super::{EmbySessionLoadedItem, EmbySessionOutput, EmbySessionQueuedItem};
use crate::DaemonEvent;
use crate::cast::PlaybackOutput;
use crate::core::DaemonOwnerContext;
use crate::ctrl::{ClientRegistry, CtrlClientId};
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

/// Loads the transition's target slot onto the Session (design D7, D14):
/// admit the item, point the queue's active marker at the slot, send one
/// `play` job for that item and the next admissible slot, record both as the
/// loaded item and the queued next one, and report the slots the search
/// stepped past. A slot the Session cannot play is reported and expires the
/// in-flight transition, so the gate cannot stay wedged behind a load that
/// never happened. Returns whether the play job was submitted.
pub(crate) fn dispatch(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: mbv_player::transition::Transition,
    start_ticks: i64,
    origin: Option<CtrlClientId>,
) -> bool {
    let target = transition.target;
    let Some((item_id, runtime_ticks, session_id)) = load_target(ctx, &transition, origin) else {
        return false;
    };
    let (next, stepped) = load_window(ctx, target);
    let load = SessionLoad {
        slot: target,
        item_id,
        runtime_ticks,
        session_id,
        start_ticks,
        next,
    };
    let PlaybackOutput::EmbySession(output) = &mut ctx.owner.output else {
        return false;
    };
    if !submit_play(output, ctx.merged_tx, load) {
        return false;
    }
    report_stepped_slots(ctx.ctrl_clients, &stepped);
    let _ = ctx.owner.core.queue.set_active_slot(target);
    // The new load replaces the item the output had: the end an earlier item
    // started belongs to the replaced load, so it ends here (design D6), and a
    // pause a hand-off still owed belongs to that replaced item too — the new
    // load starts playing (review r6 P2-2).
    output.advance = None;
    output.pending_pause = false;
    tracing::info!(name: "daemon.emby_session_load.started", target: "daemon", slot = ?target, start_ticks, "the Emby Session was given the loaded item and its next admissible slot");
    true
}

/// Everything one `play` job puts on the device (design D7, D14): the active
/// slot and its item, the runtime the end estimate caps at, the session id,
/// the start position, and the next admissible slot the device holds beside
/// it.
pub(crate) struct SessionLoad {
    /// The slot the load makes active.
    pub(crate) slot: QueueSlotId,
    /// The Emby item id the device is told to play now.
    pub(crate) item_id: String,
    /// The loaded slot's runtime in ticks (design D6).
    pub(crate) runtime_ticks: i64,
    /// The session id the job names (design D5 adopts it).
    pub(crate) session_id: String,
    /// The position the active item starts at: the resume position or the
    /// hand-off's capture.
    pub(crate) start_ticks: i64,
    /// The next admissible slot and its Emby item id (design D14), or `None`
    /// when the queue has no slot after the active one that the Session can
    /// play.
    pub(crate) next: Option<(QueueSlotId, String)>,
}

/// The next admissible slot the device holds beside the loaded one (design
/// D1, D14): the slot the Next step picks from `loaded` — the queue neighbor
/// in Sequential, the Shuffle pick — stepping past every slot the Session
/// cannot play, with the queue length as the bound. The device holds the pick
/// and nothing past it under Shuffle, as the step rule does.
///
/// Returns the slot and the Emby item id the load sends with the active item,
/// and the slots the search stepped past so the caller can report each one
/// once. Nothing is admissible and nothing was stepped past when the queue
/// ends at `loaded`.
///
/// The owner core's `relative_step_target(Next)` cannot answer this: the
/// load's own transition is the latest one, so a load that a Next step caused
/// would read as `Coalesced`, and the step would resolve from the previous
/// slot rather than from the one being loaded.
pub(crate) fn next_admissible_slot(
    core: &PlayerOwnerState,
    loaded: QueueSlotId,
    playable_media_types: &[String],
) -> (Option<(QueueSlotId, String)>, Vec<SessionAdmission>) {
    let mut stepped = Vec::new();
    for slot_id in step_candidates(core, loaded) {
        let Some(slot) = core.queue.slot(slot_id) else {
            break;
        };
        match admit(&slot.item, playable_media_types) {
            Ok(item) => return (Some((slot_id, item.id.clone())), stepped),
            Err(rejected) => stepped.push(rejected),
        }
    }
    (None, stepped)
}

/// The slots one Next step walks from `loaded` (design D14), in play order:
/// the Shuffle pick alone, because the device holds the pick and nothing past
/// it, and in Sequential every slot after `loaded`, so a rejected one can be
/// stepped past within the queue length.
///
/// A pick that names the slot being loaded is not a slot after it: the pick
/// re-picks when the next observation of the active slot changes it (design
/// D2), so within that window it names nothing the device should hold next.
fn step_candidates(core: &PlayerOwnerState, loaded: QueueSlotId) -> Vec<QueueSlotId> {
    match core.play_order() {
        PlayOrder::Shuffle { upcoming } => upcoming
            .into_iter()
            .filter(|pick| *pick != loaded)
            .collect(),
        PlayOrder::Sequential => core
            .queue
            .slot_index(loaded)
            .map_or_else(Vec::new, |index| {
                core.queue.slots()[index + 1..]
                    .iter()
                    .map(|slot| slot.slot_id)
                    .collect()
            }),
    }
}

/// Reports each slot the window search stepped past (design D1, D14), one
/// time each, and only when the load sent a slot: the report belongs to the
/// window the device receives. The owner picked that window, so the item and
/// the reason go to every Client, as the advance reports a step target it
/// stepped over.
pub(crate) fn report_stepped_slots(ctrl_clients: &ClientRegistry, stepped: &[SessionAdmission]) {
    for rejected in stepped {
        tracing::warn!(name: "daemon.emby_session_load.step_over", target: "daemon", item = %rejected.item, reason = %rejected.reason, "the next slot on the device cannot play on the Emby Session; stepping over it");
        crate::core::broadcast(
            ctrl_clients,
            &CtrlEvent::CommandRejected(format!("{} ({})", rejected.item, rejected.reason)),
        );
    }
}

/// The window the load of `slot` puts on the device (design D14), computed
/// from the owner's play order while the output is the Session that will
/// receive it.
fn load_window(
    ctx: &DaemonOwnerContext<'_>,
    slot: QueueSlotId,
) -> (Option<(QueueSlotId, String)>, Vec<SessionAdmission>) {
    let PlaybackOutput::EmbySession(output) = &ctx.owner.output else {
        return (None, Vec::new());
    };
    next_admissible_slot(&ctx.owner.core, slot, &output.playable_media_types)
}

/// Sends the one `play` job for the load's window to the Session's worker and
/// records the loaded item and the queued next one (design D6, D7, D14). A
/// load replaces what the device holds, so the stale mark of the item it
/// replaced goes with it and the window it sends is current. A failed `play`
/// re-enters the owner's loop as an `EmbySession` event, so the report and the
/// expiry run where the transition lives. Returns whether the job was
/// submitted.
pub(crate) fn submit_play(
    output: &mut EmbySessionOutput,
    merged_tx: &mpsc::Sender<DaemonEvent>,
    load: SessionLoad,
) -> bool {
    let SessionLoad {
        slot,
        item_id,
        runtime_ticks,
        session_id,
        start_ticks,
        next,
    } = load;
    let mut job_item_ids = vec![item_id.clone()];
    job_item_ids.extend(next.as_ref().map(|(_, next_item_id)| next_item_id.clone()));
    let merged_tx = merged_tx.clone();
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        let item_ids: Vec<&str> = job_item_ids.iter().map(String::as_str).collect();
        if let Err(error) = api.play(&session_id, &item_ids, start_ticks) {
            let _ = merged_tx.send(DaemonEvent::EmbySession(EmbySessionEvent::PlayFailed {
                slot,
                error,
            }));
        }
    });
    if output.worker.send(job).is_err() {
        tracing::warn!(name: "daemon.emby_session_load.worker_gone", target: "daemon", slot = ?slot, "the Emby Session worker is not running; nothing was loaded");
        return false;
    }
    // The device holds only the window this load sends (design D14): whatever
    // it held next is replaced, so a stale mark from a queue edit goes with the
    // replaced item.
    let replaced = output.queued_next.take();
    output.queued_next = next.map(|(slot, item_id)| EmbySessionQueuedItem {
        slot,
        item_id,
        stale: false,
    });
    if let Some(replaced) = replaced {
        tracing::debug!(name: "daemon.emby_session_load.queued_replaced", target: "daemon", slot = ?replaced.slot, stale = replaced.stale, "the load replaced the item the device held next");
    }
    output.loaded = Some(EmbySessionLoadedItem {
        slot,
        item_id,
        runtime_ticks,
        loaded_at: Instant::now(),
        seen_playing: false,
        end_handled: false,
        failure_reported: false,
    });
    true
}

/// Sends one `play_last` job for the slot the device should hold after the
/// active one (design D14, task 9.3) and records it as the queued next item.
/// The active item plays on, so a refused call is logged rather than reported
/// as a failed load. Returns whether the job was submitted.
pub(crate) fn submit_play_last(
    output: &mut EmbySessionOutput,
    slot: QueueSlotId,
    item_id: String,
) -> bool {
    let Some(session_id) = output.session_id.clone() else {
        tracing::warn!(name: "daemon.emby_session_load.no_session", target: "daemon", slot = ?slot, "the Emby Session output has no session id; nothing was queued");
        return false;
    };
    let job_item_id = item_id.clone();
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        if let Err(error) = api.play_last(&session_id, &[job_item_id.as_str()]) {
            tracing::warn!(name: "daemon.emby_session_load.play_last_failed", target: "daemon", slot = ?slot, error = %error, "the device refused the slot queued after the active one");
        }
    });
    if output.worker.send(job).is_err() {
        tracing::warn!(name: "daemon.emby_session_load.worker_gone", target: "daemon", slot = ?slot, "the Emby Session worker is not running; nothing was queued");
        return false;
    }
    output.queued_next = Some(EmbySessionQueuedItem {
        slot,
        item_id,
        stale: false,
    });
    true
}

/// Resolves what a load plays (design D7): the target slot's item, the active
/// output's admission of it, and the session id the `play` job names. A
/// missing slot, a non-Session output, an unplayable item, or a Session with
/// no session id loads nothing; the rejection path reports the item and
/// expires the transition, so the gate cannot stay wedged behind it.
fn load_target(
    ctx: &mut DaemonOwnerContext<'_>,
    transition: &mbv_player::transition::Transition,
    origin: Option<CtrlClientId>,
) -> Option<(String, i64, String)> {
    let target = transition.target;
    let Some(slot) = ctx.owner.core.queue.slot(target) else {
        tracing::warn!(name: "daemon.emby_session_load.slot_missing", target: "daemon", slot = ?target, "the load target slot is absent; nothing was loaded");
        return None;
    };
    let item = slot.item.clone();
    let PlaybackOutput::EmbySession(output) = &ctx.owner.output else {
        tracing::warn!(name: "daemon.emby_session_load.not_the_output", target: "daemon", slot = ?target, "the playback output is not an Emby Session; nothing was loaded");
        return None;
    };
    let admitted = match admit(&item, &output.playable_media_types) {
        Ok(emby_item) => emby_item,
        Err(rejected) => {
            report_rejection(ctx, transition, origin, &rejected);
            crate::cast::expire_remote_transition(ctx);
            return None;
        }
    };
    let item_id = admitted.id.clone();
    let runtime_ticks = admitted.runtime_ticks;
    let Some(session_id) = output.session_id.clone() else {
        tracing::warn!(name: "daemon.emby_session_load.no_session", target: "daemon", slot = ?target, "the Emby Session output has no session id; nothing was loaded");
        return None;
    };
    Some((item_id, runtime_ticks, session_id))
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
    crate::cast::reject_current_intent(ctx, transition);
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
