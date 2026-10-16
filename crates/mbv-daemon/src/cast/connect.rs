//! Output selection and connection lifecycle (cast-owner-output design
//! D2/D3/D8/D11): `SelectPlaybackOutput`, connect and reconnect outcomes, and
//! the disconnect-and-reconnect recovery a poll error or a wedged worker
//! spawns.

use std::sync::mpsc;
use std::time::Instant;

use mbv_cast::{CastClient, CastJob, CastReceiver, ConnectMode, spawn_cast_worker};
use mbv_ctrl::{CastConnection, CtrlEvent, OutputSelection};

use crate::{CtrlSender, send_to};

use super::DaemonLoop;
use super::PlaybackOutput;
use super::status::refresh_presentation;

/// A selection waiting for its Cast worker to connect (design D2). `reply_tx`
/// answers the selecting Client when the connect fails; on success the
/// broadcast is the reply.
pub struct PendingCastConnect {
    receiver: CastReceiver,
    reply_tx: CtrlSender,
}

/// What a Cast connector needs for one connect attempt.
#[derive(Debug)]
pub struct CastConnectRequest {
    pub host: String,
    pub port: u16,
    /// First select or automatic reconnect (design D11): a reconnect never
    /// launches over another sender's running app.
    pub mode: ConnectMode,
    /// One-shot reply path; the connector sends exactly one outcome on it.
    pub reply: mpsc::Sender<CastConnectOutcome>,
}

/// Spawns one Cast worker connection for the Owner (design D2). The Local
/// daemon supplies a connector through `DaemonRuntimeHooks`; `mbvd` passes
/// `None`. The connector hands the (blocking) connect to its own thread and
/// reports the outcome on the request's reply path.
pub type CastConnector = Box<dyn Fn(CastConnectRequest) + Send>;

/// The Local daemon's Cast connector (design D2): connect in first-select
/// mode (design D11) on a dedicated worker thread.
pub fn local_cast_connector() -> CastConnector {
    Box::new(|request: CastConnectRequest| {
        let CastConnectRequest {
            host,
            port,
            mode,
            reply,
        } = request;
        std::thread::spawn(move || {
            let outcome = match spawn_cast_worker(move || CastClient::connect(&host, port, mode)) {
                Ok(worker) => CastConnectOutcome::Connected(worker),
                Err(error) => CastConnectOutcome::Failed(error.to_string()),
            };
            let _ = reply.send(outcome);
        });
    })
}

/// `CtrlCmd::SelectPlaybackOutput` (design D3, task 3.2). Called by the event
/// loop before the ctrl dispatcher, because the connector lives on the loop.
pub fn handle_select_playback_output(
    daemon: &mut DaemonLoop,
    reply_tx: &CtrlSender,
    selection: OutputSelection,
) {
    match selection {
        OutputSelection::Local => {
            // A pending Cast connect loses to the newer Local selection: the
            // selector is told, and the late outcome matches nothing and is
            // discarded when it arrives.
            if let Some(pending) = daemon.owner.pending_cast_connect.take() {
                send_to(
                    &pending.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "a newer selection kept local playback".to_string(),
                    ),
                );
                return;
            }
            // Selecting Local while a receiver is the output hands the active
            // slot back (design D10); the hand-off sequence is task 7.1.
            if matches!(daemon.owner.output, PlaybackOutput::Cast(_)) {
                send_to(
                    reply_tx,
                    &CtrlEvent::CommandRejected(
                        "moving receiver playback back to local is not supported yet".to_string(),
                    ),
                );
            }
        }
        OutputSelection::Cast {
            receiver_id,
            name,
            host,
            port,
        } => {
            let Some(connector) = daemon.cast_connector.as_ref() else {
                send_to(
                    reply_tx,
                    &CtrlEvent::CommandRejected(
                        "this owner has no Cast playback output".to_string(),
                    ),
                );
                return;
            };
            // Local → Cast is the only selection this unit swaps; hand-offs
            // between outputs keep the current output until task 7.1.
            if matches!(daemon.owner.output, PlaybackOutput::Cast(_)) {
                send_to(
                    reply_tx,
                    &CtrlEvent::CommandRejected(
                        "moving playback between outputs is not supported yet".to_string(),
                    ),
                );
                return;
            }
            // A slot counts as active while the local run plays or pauses it
            // (design D10 step 1); moving active playback is task 7.1.
            if daemon.player.status.lock().unwrap().active {
                send_to(
                    reply_tx,
                    &CtrlEvent::CommandRejected(
                        "stop playback before moving it to a receiver".to_string(),
                    ),
                );
                return;
            }
            let receiver = CastReceiver {
                id: receiver_id.clone(),
                friendly_name: name,
                host,
                port,
            };
            let (outcome_tx, outcome_rx) = mpsc::channel::<CastConnectOutcome>();
            connector(CastConnectRequest {
                host: receiver.host.clone(),
                port: receiver.port,
                mode: ConnectMode::FirstSelect,
                reply: outcome_tx,
            });
            // A connect already in flight loses to this newer selection: its
            // selector is told now, so no Client hangs, and its late outcome
            // matches nothing and is discarded when it arrives.
            if let Some(displaced) = daemon.owner.pending_cast_connect.take() {
                send_to(
                    &displaced.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "superseded by a newer receiver selection".to_string(),
                    ),
                );
            }
            daemon.owner.pending_cast_connect = Some(PendingCastConnect {
                receiver,
                reply_tx: reply_tx.clone(),
            });
            let merged_tx = daemon.merged_tx.clone();
            std::thread::spawn(move || {
                if let Ok(outcome) = outcome_rx.recv() {
                    let _ =
                        merged_tx.send(crate::DaemonEvent::Cast(super::CastOwnerEvent::Connect {
                            receiver_id,
                            outcome,
                        }));
                }
            });
        }
    }
}

/// One connect outcome a Cast connector reported (design D2). A connected
/// worker is owned by whoever stores it: dropping the sender ends the worker
/// thread, which is how a displaced or unused connect is discarded.
#[derive(Debug)]
pub enum CastConnectOutcome {
    Connected(mpsc::Sender<CastJob>),
    Failed(String),
}

/// A connect outcome applied to the owner's output (task 3.2). A success
/// swaps the output, re-presents it (row 6.1: the presented status starts
/// clean), and broadcasts the change; a failure keeps the current output and
/// reports to the selecting Client.
pub(super) fn handle_connect_outcome(
    daemon: &mut DaemonLoop,
    receiver_id: &str,
    outcome: CastConnectOutcome,
) {
    let waiting = daemon
        .owner
        .pending_cast_connect
        .as_ref()
        .is_some_and(|pending| pending.receiver.id == receiver_id);
    if !waiting {
        // No selection waits for this receiver: the outcome is displaced (the
        // last selection wins), so its worker has no owner and the dropped
        // sender ends the worker thread.
        if let CastConnectOutcome::Connected(worker) = outcome {
            drop(worker);
        }
        return;
    }
    let Some(pending) = daemon.owner.pending_cast_connect.take() else {
        return;
    };
    match outcome {
        CastConnectOutcome::Connected(worker) => {
            // Local playback may have started after the select (the
            // select-time check only guards the selection moment). Swapping
            // now would leave two live outputs, so the connect is discarded
            // and reported instead.
            if daemon.player.status.lock().unwrap().active {
                drop(worker);
                send_to(
                    &pending.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "local playback started while connecting; stop playback before moving it to a receiver"
                            .to_string(),
                    ),
                );
                return;
            }
            let output = super::CastOutput {
                worker,
                receiver: pending.receiver,
                connection: CastConnection::Connected,
                last_status: None,
                status_at: None,
                loaded: None,
                last_keep_alive: Instant::now(),
                last_poll: None,
                poll_sent: None,
                next_poll_id: 0,
                reconnecting: false,
                advance: None,
                pending_seek: None,
            };
            tracing::info!(name: "daemon.cast_output.connected", target: "daemon", receiver = %receiver_id, output = ?output, "cast receiver is now the playback output");
            daemon.owner.output = PlaybackOutput::Cast(Box::new(output));
            refresh_presentation(daemon, Instant::now());
            broadcast_output_change(daemon);
        }
        CastConnectOutcome::Failed(reason) => {
            tracing::warn!(name: "daemon.cast_output.connect_failed", target: "daemon", receiver = %receiver_id, error = %reason, "cast connect failed; playback output unchanged");
            send_to(
                &pending.reply_tx,
                &CtrlEvent::CommandRejected(format!("connecting to the receiver failed: {reason}")),
            );
        }
    }
}

/// The reconnect a poll error or a watchdog timeout spawned (design D8/D11,
/// task 3.3). A success restores the existing output's worker and connection;
/// a failure leaves the output `Disconnected` (task 7.4 refines the recovery
/// outcomes). A displaced outcome — the output moved on or was replaced — is
/// discarded, and its worker sender dropped to end the thread.
pub(super) fn handle_reconnect_outcome(
    daemon: &mut DaemonLoop,
    receiver_id: &str,
    outcome: CastConnectOutcome,
) {
    let restored = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            drop_connected_worker(outcome);
            return;
        };
        if cast.receiver.id != receiver_id || !cast.reconnecting {
            drop_connected_worker(outcome);
            return;
        }
        cast.reconnecting = false;
        match outcome {
            CastConnectOutcome::Connected(worker) => {
                cast.worker = worker;
                cast.connection = CastConnection::Connected;
                cast.poll_sent = None;
                cast.last_poll = Some(Instant::now());
                cast.last_keep_alive = Instant::now();
                true
            }
            CastConnectOutcome::Failed(reason) => {
                tracing::warn!(name: "daemon.cast_output.reconnect_failed", target: "daemon", receiver = %receiver_id, error = %reason, "cast reconnect failed; the output stays disconnected");
                false
            }
        }
    };
    if restored {
        broadcast_output_change(daemon);
    }
}

/// A connected worker with no owner: dropping its sender ends the worker
/// thread (the same discard `handle_connect_outcome` applies).
fn drop_connected_worker(outcome: CastConnectOutcome) {
    if let CastConnectOutcome::Connected(worker) = outcome {
        drop(worker);
    }
}

/// Marks the Cast output disconnected, broadcasts the change, and spawns one
/// reconnect attempt in reconnect mode (design D8). The queue, the active
/// slot, and the loaded item are untouched: the connection loss says nothing
/// about the owner's playback intent. The wedged or dead worker's sender is
/// replaced when the reconnect succeeds; while disconnected no jobs run.
pub(super) fn disconnect_and_reconnect(daemon: &mut DaemonLoop) {
    let (receiver_id, host, port) = {
        let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
            return;
        };
        if !matches!(cast.connection, CastConnection::Connected) {
            return;
        }
        (
            cast.receiver.id.clone(),
            cast.receiver.host.clone(),
            cast.receiver.port,
        )
    };
    let connector = daemon
        .cast_connector
        .as_ref()
        .expect("a Cast output exists only where a connector was supplied");
    let (outcome_tx, outcome_rx) = mpsc::channel::<CastConnectOutcome>();
    connector(CastConnectRequest {
        host,
        port,
        mode: ConnectMode::Reconnect,
        reply: outcome_tx,
    });
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    cast.connection = CastConnection::Disconnected;
    cast.poll_sent = None;
    cast.reconnecting = true;
    broadcast_output_change(daemon);
    let merged_tx = daemon.merged_tx.clone();
    std::thread::spawn(move || {
        if let Ok(outcome) = outcome_rx.recv() {
            let _ = merged_tx.send(crate::DaemonEvent::Cast(super::CastOwnerEvent::Reconnect {
                receiver_id,
                outcome,
            }));
        }
    });
}

/// Mirrors the owner's output into the shared state and broadcasts it to
/// peers that negotiated `playback-output` (design D3).
fn broadcast_output_change(daemon: &DaemonLoop) {
    let state = super::output_state(&daemon.owner.output);
    *daemon.shared_queue.playback_output.lock().unwrap() = state.clone();
    let Ok(json) = serde_json::to_string(&CtrlEvent::PlaybackOutput(state)) else {
        return;
    };
    daemon
        .ctrl_clients
        .lock()
        .unwrap()
        .broadcast_playback_output_gated(&json);
}
