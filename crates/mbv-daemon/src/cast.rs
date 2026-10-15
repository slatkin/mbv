//! The Owner's playback output: local playback or one Cast receiver
//! (cast-owner-output design D1/D2/D3). The Owner holds exactly one output;
//! the Cast side owns the worker sender, the receiver identity, and the
//! connection state, and every receiver call runs on the Cast worker.

use std::sync::mpsc;
use std::time::Instant;

use mbv_cast::{CastClient, CastJob, CastReceiver, CastStatus, ConnectMode, spawn_cast_worker};
use mbv_ctrl::{CastConnection, CtrlEvent, OutputSelection, OutputState};
use mbv_queue::QueueSlotId;

use super::DaemonLoop;
use super::core::DaemonEvent;
use super::ctrl::{CtrlSender, send_to};

/// The single playback output a Player owner plays through (design D1): the
/// type system makes two active outputs unrepresentable. The Cast side is
/// boxed: it carries the worker sender, the last status, and the loaded item
/// while `Local` carries nothing (later units grow it further).
#[derive(Default)]
pub(crate) enum PlaybackOutput {
    #[default]
    Local,
    Cast(Box<CastOutput>),
}

/// One Cast receiver as the Owner's output (design D1). The worker sender is
/// the only path to the receiver: the `CastClient` itself lives on the worker
/// thread and is never `Send`. `last_status` is the most recent poll result
/// (task 3.3 fills it); `loaded` is the item the receiver was told to play.
/// Its progress target and ABS lifecycle join it in tasks 4.1/6.4.
pub(crate) struct CastOutput {
    worker: mpsc::Sender<CastJob>,
    receiver: CastReceiver,
    connection: CastConnection,
    last_status: Option<CastStatus>,
    status_at: Option<Instant>,
    loaded: Option<CastLoadedItem>,
}

/// The item the Cast receiver was told to play (design D1). `end_handled`
/// stops a second poll of the same `Finished` entry from advancing twice
/// (design D7); task 6.2 owns it.
pub(crate) struct CastLoadedItem {
    slot: QueueSlotId,
    content_id: String,
    end_handled: bool,
}

impl std::fmt::Debug for CastLoadedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CastLoadedItem")
            .field("slot", &self.slot)
            .field("content_id", &self.content_id)
            .field("end_handled", &self.end_handled)
            .finish()
    }
}

impl std::fmt::Debug for CastOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CastOutput")
            .field("worker", &self.worker)
            .field("receiver", &self.receiver)
            .field("connection", &self.connection)
            .field("last_status", &self.last_status)
            .field("status_at", &self.status_at)
            .field("loaded", &self.loaded)
            .finish()
    }
}

/// The ctrl `OutputState` an output projects (design D3).
pub(crate) fn output_state(output: &PlaybackOutput) -> OutputState {
    match output {
        PlaybackOutput::Local => OutputState::Local,
        PlaybackOutput::Cast(cast) => OutputState::Cast {
            receiver_id: cast.receiver.id.clone(),
            name: cast.receiver.friendly_name.clone(),
            connection: cast.connection,
        },
    }
}

/// A selection waiting for its Cast worker to connect (design D2). `reply_tx`
/// answers the selecting Client when the connect fails; on success the
/// broadcast is the reply.
pub(crate) struct PendingCastConnect {
    receiver: CastReceiver,
    reply_tx: CtrlSender,
}

/// The result of one connect attempt, carried by `DaemonEvent::Cast`.
pub(crate) enum CastOwnerEvent {
    Connect {
        receiver_id: String,
        outcome: CastConnectOutcome,
    },
}

/// The connect outcome a Cast connector reports (design D2). A connected
/// worker is owned by whoever stores it: dropping the sender ends the worker
/// thread, which is how a displaced or unused connect is discarded.
#[derive(Debug)]
pub enum CastConnectOutcome {
    Connected(mpsc::Sender<CastJob>),
    Failed(String),
}

/// What a Cast connector needs for one connect attempt.
#[derive(Debug)]
pub struct CastConnectRequest {
    pub host: String,
    pub port: u16,
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
        let CastConnectRequest { host, port, reply } = request;
        std::thread::spawn(move || {
            let outcome = match spawn_cast_worker(move || {
                CastClient::connect(&host, port, ConnectMode::FirstSelect)
            }) {
                Ok(worker) => CastConnectOutcome::Connected(worker),
                Err(error) => CastConnectOutcome::Failed(error.to_string()),
            };
            let _ = reply.send(outcome);
        });
    })
}

/// `CtrlCmd::SelectPlaybackOutput` (design D3, task 3.2). Called by the event
/// loop before the ctrl dispatcher, because the connector lives on the loop.
pub(super) fn handle_select_playback_output(
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
                    let _ = merged_tx.send(DaemonEvent::Cast(CastOwnerEvent::Connect {
                        receiver_id,
                        outcome,
                    }));
                }
            });
        }
    }
}

/// `DaemonEvent::Cast`: apply one connect outcome to the owner's output
/// (task 3.2). A connect failure keeps the current output and reports to the
/// selecting Client; a success with no active slot swaps the output and
/// broadcasts it to peers that negotiated `playback-output`.
pub(super) fn handle_cast_event(daemon: &mut DaemonLoop, event: CastOwnerEvent) {
    let CastOwnerEvent::Connect {
        receiver_id,
        outcome,
    } = event;
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
            let output = CastOutput {
                worker,
                receiver: pending.receiver,
                connection: CastConnection::Connected,
                last_status: None,
                status_at: None,
                loaded: None,
            };
            tracing::info!(name: "daemon.cast_output.connected", target: "daemon", receiver = %receiver_id, output = ?output, "cast receiver is now the playback output");
            daemon.owner.output = PlaybackOutput::Cast(Box::new(output));
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

/// Mirrors the owner's output into the shared state and broadcasts it to
/// peers that negotiated `playback-output` (design D3).
fn broadcast_output_change(daemon: &DaemonLoop) {
    let state = output_state(&daemon.owner.output);
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
