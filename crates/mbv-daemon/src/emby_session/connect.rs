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
use crate::emby_session::worker::spawn_emby_session_worker;
use crate::event_loop::DaemonLoop;
use crate::{CtrlSender, DaemonEvent, send_to};

use super::EmbySessionEvent;
use super::EmbySessionOutput;
use super::worker::EmbySessionJob;
use super::worker::SessionApi;

/// A Session selection waiting for its worker's device lookup (design D4). The
/// worker it will own on success, the selected device, and the reply path for
/// a rejection. A newer selection displaces it (last selection wins), and the
/// displaced worker's dropped sender ends its thread.
pub(crate) struct PendingEmbySessionSelect {
    pub(crate) worker: mpsc::Sender<EmbySessionJob>,
    pub(crate) device_id: String,
    pub(crate) name: String,
    pub(crate) reply_tx: CtrlSender,
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
    daemon.owner.pending_emby_session_select = Some(PendingEmbySessionSelect {
        worker,
        device_id,
        name,
        reply_tx: reply_tx.clone(),
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
        let result = api.get_sessions().and_then(|sessions| {
            sessions
                .into_iter()
                .find(|session| session.device_id == wanted)
                .ok_or_else(|| format!("no Session for the device {wanted} is listed"))
        });
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
    device_id: String,
    result: Result<SessionInfo, String>,
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
        Ok(session) => install_output(daemon, pending, session),
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
/// before the first poll (task 5.1).
fn install_output(
    daemon: &mut DaemonLoop,
    pending: PendingEmbySessionSelect,
    session: SessionInfo,
) {
    let output = EmbySessionOutput {
        worker: pending.worker,
        device_id: pending.device_id.clone(),
        name: pending.name,
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
        pending_pause: false,
        advance: None,
    };
    tracing::info!(
        name: "daemon.emby_session_output.selected",
        target: "daemon",
        device = %pending.device_id,
        output = ?output,
        "the Emby Session is now the playback output"
    );
    daemon.owner.output = PlaybackOutput::EmbySession(Box::new(output));
    broadcast_output_change(daemon);
}
