//! Selecting an Emby Session as the playback output (emby-session-owner-output
//! design D2, D4, D11; task 3.2): the hook and the owner-context gate, the
//! device lookup on the output's own worker, and the swap that installs the
//! Session. The hand-off of the old output's playback is task 6.1, so a
//! selection here only resolves, swaps, and broadcasts.

use std::sync::mpsc;
use std::time::Instant;

use mbv_ctrl::{CtrlEvent, OutputConnection};
use mbv_emby::SessionInfo;

use crate::cast::{PlaybackOutput, broadcast_output_change};
use crate::emby_session::load::admit;
use crate::emby_session::worker::spawn_emby_session_worker;
use crate::event_loop::DaemonLoop;
use crate::output::handoff::HandoffPlayback;
use crate::{CtrlSender, DaemonEvent, send_to};

use super::EmbySessionEvent;
use super::EmbySessionLoadedItem;
use super::EmbySessionOutput;
use super::worker::EmbySessionJob;
use super::worker::SessionApi;

/// A Session selection waiting for its worker's device lookup (design D4). The
/// worker it will own on success, the selected device, the reply path for a
/// rejection, and the old output's active playback captured at the selection
/// (design D11; `None` when the old output was idle). A newer selection
/// displaces it (last selection wins), and the displaced worker's dropped
/// sender ends its thread.
pub(crate) struct PendingEmbySessionSelect {
    pub(crate) worker: mpsc::Sender<EmbySessionJob>,
    pub(crate) device_id: String,
    pub(crate) name: String,
    pub(crate) reply_tx: CtrlSender,
    pub(crate) handoff: Option<HandoffPlayback>,
}

/// `OutputSelection::EmbySession` (design D2, D4). Rejects the selection when
/// the runtime does not host the output, when the owner has no Emby context,
/// or when the device is not a listed Session; selecting the device that is
/// already the output does nothing.
pub(crate) fn select(
    daemon: &mut DaemonLoop,
    reply_tx: &CtrlSender,
    device_id: String,
    name: String,
) {
    if !daemon.emby_session_output {
        send_to(
            reply_tx,
            &CtrlEvent::CommandRejected(
                "this owner does not host an Emby Session playback output".to_string(),
            ),
        );
        return;
    }
    // The worker is built from the owner's Emby client (design D4). A runtime
    // that advertises the output without an Emby context has nothing to
    // resolve a Session with, so the selection is rejected here.
    let worker = if let Some(factory) = &daemon.emby_session_worker {
        factory()
    } else if let Some(context) = daemon.emby_runtime.as_ref() {
        let client = context.client.lock().unwrap().clone();
        spawn_emby_session_worker(move || client)
    } else {
        send_to(
            reply_tx,
            &CtrlEvent::CommandRejected(
                "this owner has no Emby setup to resolve a Session with".to_string(),
            ),
        );
        return;
    };
    // A pending Cast connect loses to this newer Session selection (design
    // D11, last selection wins): its selector is told now, so no Client hangs,
    // and its late outcome matches no pending connect and is discarded. This
    // runs before the no-op below, so even a selection that changes nothing
    // still displaces the older choice.
    if let Some(displaced) = daemon.owner.pending_cast_connect.take() {
        send_to(
            &displaced.reply_tx,
            &CtrlEvent::CommandRejected("superseded by a newer Session selection".to_string()),
        );
    }
    // Selecting the device that already is the output is no hand-off (design
    // D11): the polls already follow it, so a fresh worker would only replace
    // the observation it holds.
    if let PlaybackOutput::EmbySession(current) = &daemon.owner.output
        && current.device_id == device_id
    {
        return;
    }
    // A selection already waiting for its lookup loses to this newer one: its
    // selector is told now, so no Client hangs, and its late outcome matches no
    // pending selection and is discarded.
    if let Some(displaced) = daemon.owner.pending_emby_session_select.take() {
        send_to(
            &displaced.reply_tx,
            &CtrlEvent::CommandRejected("superseded by a newer Session selection".to_string()),
        );
    }
    if !dispatch_lookup(&worker, daemon.merged_tx.clone(), device_id.clone()) {
        send_to(
            reply_tx,
            &CtrlEvent::CommandRejected("the Emby Session worker is not running".to_string()),
        );
        return;
    }
    // The hand-off is captured at the selection (design D10 step 1, D11):
    // the old output (Local, a receiver, or another Session) hands its active
    // playback over once this Session is resolved. The presented position is
    // pinned to this instant, so a slow lookup cannot move it.
    crate::output::handoff::refresh_active_output_presentation(daemon, Instant::now());
    let handoff = crate::output::handoff::capture_handoff_for_selection(daemon);
    daemon.owner.pending_emby_session_select = Some(PendingEmbySessionSelect {
        worker,
        device_id,
        name,
        reply_tx: reply_tx.clone(),
        handoff,
    });
}

/// Drops a pending Emby Session selection that a newer output selection of
/// another kind displaces (design D11, last selection wins): the displaced
/// selector is told now, and its late lookup finds no pending selection and is
/// discarded.
pub(crate) fn discard_pending_select(daemon: &mut DaemonLoop, reason: &str) {
    if let Some(displaced) = daemon.owner.pending_emby_session_select.take() {
        send_to(
            &displaced.reply_tx,
            &CtrlEvent::CommandRejected(reason.to_string()),
        );
    }
}

/// Submits the device lookup to the worker (design D4): the unfiltered
/// `/Sessions` list, matched by `DeviceId`, because the filtered list drops a
/// device that is idle but still connected (#236). The result re-enters the
/// owner's event loop as `EmbySessionEvent::Selected`.
fn dispatch_lookup(
    worker: &mpsc::Sender<EmbySessionJob>,
    events: mpsc::Sender<DaemonEvent>,
    device_id: String,
) -> bool {
    let wanted = device_id.clone();
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        let result = api
            .get_sessions()
            .and_then(|sessions| {
                sessions
                    .into_iter()
                    .find(|session| session.device_id == wanted)
                    .ok_or_else(|| format!("no Session for the device {wanted} is listed"))
            })
            .map(Box::new);
        let _ = events.send(DaemonEvent::EmbySession(EmbySessionEvent::Selected {
            device_id,
            result,
        }));
    });
    worker.send(job).is_ok()
}

/// Applies one lookup outcome (task 3.2): a matched Session becomes the
/// playback output; a missing device or a failed lookup keeps the current
/// output and reports the reason to the selecting Client.
pub(super) fn handle_selected(
    daemon: &mut DaemonLoop,
    device_id: &str,
    result: Result<Box<SessionInfo>, String>,
) {
    // The pending selection is compared before it is taken: a displaced
    // selection's late lookup must not consume the live selection that a newer
    // choice installed (design D11, last selection wins).
    let matches_pending = daemon
        .owner
        .pending_emby_session_select
        .as_ref()
        .is_some_and(|pending| pending.device_id == device_id);
    if !matches_pending {
        return;
    }
    let Some(pending) = daemon.owner.pending_emby_session_select.take() else {
        return;
    };
    match result {
        Ok(session) => install_output(daemon, pending, *session),
        Err(reason) => {
            tracing::warn!(
                name: "daemon.emby_session_output.select_failed",
                target: "daemon",
                device = %device_id,
                error = %reason,
                "the Emby Session selection failed; the playback output is unchanged"
            );
            send_to(
                &pending.reply_tx,
                &CtrlEvent::CommandRejected(format!("selecting the Emby Session failed: {reason}")),
            );
        }
    }
}

/// Installs the resolved Session as the playback output and broadcasts it
/// (task 3.2). The output starts `Connected` with the Session the lookup
/// returned as its first observation, so the status projection has a value
/// before the first poll (task 5.1), and the captured hand-off — when the old
/// output had playback — continues on it (design D11, task 6.1).
fn install_output(
    daemon: &mut DaemonLoop,
    pending: PendingEmbySessionSelect,
    session: SessionInfo,
) {
    let PendingEmbySessionSelect {
        worker,
        device_id,
        name,
        reply_tx,
        handoff,
    } = pending;
    // Local playback may have started after the selection captured nothing
    // active. Swapping now would discard it silently, so the resolved worker
    // is dropped and the selection reported instead — the same rule a Cast
    // connect applies (design D11: the Cast D10 sequence holds unchanged,
    // review r6). The guard is narrow: it fires only while the output is still
    // Local, so an Emby Session→Emby Session selection never trips it, even
    // when the old device presents content the Owner did not load.
    let local_playback_started = matches!(daemon.owner.output, PlaybackOutput::Local)
        && handoff.is_none()
        && daemon.player.status.lock().unwrap().active;
    if local_playback_started {
        drop(worker);
        send_to(
            &reply_tx,
            &CtrlEvent::CommandRejected(
                "playback started while connecting; stop playback before moving it to an Emby Session"
                    .to_string(),
            ),
        );
        return;
    }
    // Every swap stops the old output (design D10 step 3, D11), whether or not
    // a hand-off was captured: the local run stops, the old receiver's media
    // stops with its app teardown, or the old Session's device stops only
    // while it plays the loaded item. The stop makes every awaited transition
    // identity the old output could confirm unobservable.
    crate::output::handoff::stop_old_output_for_handoff(daemon);
    crate::core::reset_slot_jumps(
        &mut daemon.owner.core.transitions,
        &mut daemon.owner.queued_transition_origin,
    );
    let output = EmbySessionOutput {
        worker,
        device_id: device_id.clone(),
        name,
        session_id: Some(session.id.clone()),
        playable_media_types: session.playable_media_types.clone(),
        connection: OutputConnection::Connected,
        misses: 0,
        poll_sent: None,
        last_poll: None,
        next_poll_id: 0,
        last_observation: Some(session),
        observed_at: Some(Instant::now()),
        loaded: None,
        // A `play` cannot start paused (design D9): the captured pause waits
        // for the first observation that shows the loaded item.
        pending_pause: handoff.is_some_and(|handoff| handoff.paused),
        advance: None,
    };
    tracing::info!(
        name: "daemon.emby_session_output.selected",
        target: "daemon",
        device = %device_id,
        handoff = handoff.is_some(),
        output = ?output,
        "the Emby Session is now the playback output"
    );
    daemon.owner.output = PlaybackOutput::EmbySession(Box::new(output));
    if let Some(handoff) = handoff {
        // The queue's own active marker follows the handed-off slot, the same
        // bookkeeping a Cast hand-off applies.
        let _ = daemon.owner.core.queue.set_active_slot(handoff.slot);
        load_handoff(daemon, handoff, &reply_tx);
    }
    crate::emby_session::status::refresh_presentation(daemon, Instant::now());
    broadcast_output_change(daemon);
}

/// Loads the captured slot onto the freshly selected Session (design D7,
/// D11; task 6.1): admission, then the one `play` job for that item at the
/// captured position. An item the Session cannot play is reported to the
/// selecting Client and loads nothing; the output still becomes the Session,
/// as a Cast hand-off with an uncastable item does.
fn load_handoff(daemon: &mut DaemonLoop, handoff: HandoffPlayback, reply_tx: &CtrlSender) {
    let Some((item_id, runtime_ticks, session_id)) =
        handoff_play_target(daemon, &handoff, reply_tx)
    else {
        return;
    };
    let start_ticks = handoff.start_ticks;
    let job_item_id = item_id.clone();
    let slot_id = handoff.slot;
    let merged_tx = daemon.merged_tx.clone();
    let job: EmbySessionJob = Box::new(move |api: &dyn SessionApi| {
        if let Err(error) = api.play(&session_id, &job_item_id, start_ticks) {
            // The hand-off has no transition to expire, but the report path is
            // the same one a load's failed `play` uses (design D6, D7).
            let _ = merged_tx.send(DaemonEvent::EmbySession(EmbySessionEvent::PlayFailed {
                slot: slot_id,
                error,
            }));
        }
    });
    let PlaybackOutput::EmbySession(output) = &mut daemon.owner.output else {
        return;
    };
    if output.worker.send(job).is_err() {
        tracing::warn!(
            name: "daemon.emby_session_load.worker_gone",
            target: "daemon",
            slot = ?handoff.slot,
            "the Emby Session worker is not running; nothing was loaded"
        );
        return;
    }
    output.loaded = Some(EmbySessionLoadedItem {
        slot: handoff.slot,
        item_id,
        runtime_ticks,
        loaded_at: Instant::now(),
        seen_playing: false,
        end_handled: false,
        failure_reported: false,
    });
    tracing::info!(
        name: "daemon.emby_session_load.handoff",
        target: "daemon",
        slot = ?handoff.slot,
        start_ticks,
        paused = handoff.paused,
        "the handed-off slot loads on the Emby Session at the captured position"
    );
}

/// Resolves what a hand-off loads (design D7, D11): the item id, the runtime
/// its end estimate caps at, and the session id the `play` job names. `None`
/// when the handed-off slot is absent, the output is not a Session, the item
/// is unplayable, or the Session has no session id; an unplayable item is
/// reported to the selecting Client.
fn handoff_play_target(
    daemon: &DaemonLoop,
    handoff: &HandoffPlayback,
    reply_tx: &CtrlSender,
) -> Option<(String, i64, String)> {
    let Some(slot) = daemon.owner.core.queue.slot(handoff.slot) else {
        tracing::warn!(
            name: "daemon.handoff.slot_missing",
            target: "daemon",
            slot = ?handoff.slot,
            "the handed-off slot is absent; nothing was loaded"
        );
        return None;
    };
    let item = slot.item.clone();
    let PlaybackOutput::EmbySession(output) = &daemon.owner.output else {
        return None;
    };
    let emby_item = match admit(&item, &output.playable_media_types) {
        Ok(emby_item) => emby_item,
        Err(rejected) => {
            tracing::warn!(
                name: "daemon.emby_session_load.unplayable",
                target: "daemon",
                slot = ?handoff.slot,
                item = %rejected.item,
                reason = %rejected.reason,
                "the handed-off item cannot play on the Emby Session; nothing was loaded"
            );
            send_to(
                reply_tx,
                &CtrlEvent::CommandRejected(format!("{} ({})", rejected.item, rejected.reason)),
            );
            return None;
        }
    };
    let item_id = emby_item.id.clone();
    let runtime_ticks = emby_item.runtime_ticks;
    let Some(session_id) = output.session_id.clone() else {
        tracing::warn!(
            name: "daemon.emby_session_load.no_session",
            target: "daemon",
            slot = ?handoff.slot,
            "the Emby Session output has no session id; nothing was loaded"
        );
        return None;
    };
    Some((item_id, runtime_ticks, session_id))
}
