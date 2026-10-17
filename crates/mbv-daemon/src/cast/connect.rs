//! Output selection and connection lifecycle (cast-owner-output design
//! D2/D3/D8/D10/D11): `SelectPlaybackOutput`, the Local→Cast and Cast→Cast
//! hand-offs, the Cast→Local hand-back, connect and reconnect outcomes, and
//! the disconnect-and-reconnect recovery a poll error or a wedged worker
//! spawns.

use std::sync::mpsc;
use std::time::Instant;

use mbv_cast::{CastClient, CastJob, CastReceiver, ConnectMode, spawn_cast_worker};
use mbv_ctrl::{CtrlEvent, OutputConnection, OutputSelection};
use mbv_queue::QueueSlotId;

use crate::{CtrlSender, send_to};

use super::DaemonLoop;
use super::PlaybackOutput;
use super::status::refresh_presentation;

/// A selection waiting for its Cast worker to connect (design D2). `reply_tx`
/// answers the selecting Client when the connect fails; on success the
/// broadcast is the reply. `handoff` is the active playback the selection
/// captured from the old output — the local run or the receiver that was the
/// output (design D10, row 7.1); `None` when the output was idle at the
/// selection.
pub struct PendingCastConnect {
    receiver: CastReceiver,
    reply_tx: CtrlSender,
    handoff: Option<HandoffPlayback>,
}

/// The active playback a selection captures from the old output for the
/// hand-off (design D10 step 1, row 7.1): the slot, its position, and its
/// pause state.
#[derive(Clone, Copy, Debug)]
pub struct HandoffPlayback {
    pub slot: QueueSlotId,
    pub start_ticks: i64,
    pub paused: bool,
}

/// Captures the Local output's active playback for the hand-off (design D10
/// step 1, row 7.1): the run-observed active slot — the queue's own marker
/// when no run observation exists yet — with the presented position and
/// pause state. A slot counts as active only while the output plays or
/// pauses it, so a stopped or idle output hands nothing over. The Cast
/// counterpart is `capture_cast_handoff`.
fn capture_handoff_playback(daemon: &DaemonLoop) -> Option<HandoffPlayback> {
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
            // discarded when it arrives. The selection still applies below —
            // when a receiver is the output, the hand-back follows.
            if let Some(pending) = daemon.owner.pending_cast_connect.take() {
                send_to(
                    &pending.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "a newer selection chose local playback".to_string(),
                    ),
                );
            }
            // Selecting Local while a receiver is the output hands the
            // active slot back (design D10, row 7.1 dispatch B): capture,
            // stop and tear down the receiver, then continue the slot on a
            // fresh local run.
            if matches!(daemon.owner.output, PlaybackOutput::Cast(_)) {
                hand_cast_back_to_local(daemon, reply_tx);
            }
        }
        OutputSelection::Cast {
            receiver_id,
            name,
            host,
            port,
        } => {
            // A connect already in flight loses to this newer selection —
            // even when the newer selection asks for the receiver that is
            // already the output (last selection wins): its selector is told
            // now, so no Client hangs, and its late outcome matches nothing
            // and is discarded when it arrives.
            if let Some(displaced) = daemon.owner.pending_cast_connect.take() {
                send_to(
                    &displaced.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "superseded by a newer receiver selection".to_string(),
                    ),
                );
            }
            // Selecting the receiver that is already the output is no
            // hand-off: the output is already where the selection asks it to
            // be, and a full connect-stop-teardown-reload cycle would only
            // interrupt the playback it presents.
            if let PlaybackOutput::Cast(cast) = &daemon.owner.output
                && cast.receiver.id == receiver_id
            {
                send_to(
                    reply_tx,
                    &CtrlEvent::CommandRejected(
                        "this receiver is already the playback output".to_string(),
                    ),
                );
                return;
            }
            // The Cast hand-offs (design D10, row 7.1): a selection while
            // another receiver is the output moves the captured playback to
            // the new receiver; a selection while Local plays hands the
            // active slot over. Either way the active slot's state is
            // captured at the selection and continues after a successful
            // connect; an idle output hands nothing over.
            let handoff = if matches!(daemon.owner.output, PlaybackOutput::Cast(_)) {
                // One re-derivation pins the presented position to the
                // selection instant without a fresh poll (row 7.1 dispatch
                // B), and the capture reads it.
                refresh_presentation(daemon, Instant::now());
                capture_cast_handoff(daemon)
            } else {
                capture_handoff_playback(daemon)
            };
            let Some(connector) = daemon.cast_connector.as_ref() else {
                send_to(
                    reply_tx,
                    &CtrlEvent::CommandRejected(
                        "this owner has no Cast playback output".to_string(),
                    ),
                );
                return;
            };
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
            daemon.owner.pending_cast_connect = Some(PendingCastConnect {
                receiver,
                reply_tx: reply_tx.clone(),
                handoff,
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
        // Task 3.2 handles an Emby Session selection. No Owner hosts that
        // output yet, so the selector is told instead of left waiting.
        OutputSelection::EmbySession { .. } => {
            send_to(
                reply_tx,
                &CtrlEvent::CommandRejected(
                    "this owner cannot host an Emby Session playback output".to_string(),
                ),
            );
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
    let PendingCastConnect {
        receiver,
        reply_tx,
        handoff,
    } = pending;
    match outcome {
        CastConnectOutcome::Connected(worker) => {
            apply_connected_output(daemon, receiver_id, worker, receiver, reply_tx, handoff);
        }
        CastConnectOutcome::Failed(reason) => {
            tracing::warn!(name: "daemon.cast_output.connect_failed", target: "daemon", receiver = %receiver_id, error = %reason, "cast connect failed; playback output unchanged");
            send_to(
                &reply_tx,
                &CtrlEvent::CommandRejected(format!("connecting to the receiver failed: {reason}")),
            );
        }
    }
}

/// Applies one successful connect to the owner's output (task 3.2): the swap
/// stops the old output, installs the new receiver as the output, re-presents
/// it (row 6.1: the presented status starts clean), and continues a captured
/// hand-off on the new receiver.
fn apply_connected_output(
    daemon: &mut DaemonLoop,
    receiver_id: &str,
    worker: mpsc::Sender<CastJob>,
    receiver: CastReceiver,
    reply_tx: CtrlSender,
    handoff: Option<HandoffPlayback>,
) {
    let active_now = daemon.player.status.lock().unwrap().active;
    if handoff.is_none() && active_now {
        // Playback may have started on the old output after the
        // select (the select-time capture found nothing active).
        // Swapping now would leave two live outputs, so the connect
        // is discarded and reported instead.
        drop(worker);
        send_to(
            &reply_tx,
            &CtrlEvent::CommandRejected(
                "playback started while connecting; stop playback before moving it to a receiver"
                    .to_string(),
            ),
        );
        return;
    }
    log_handoff_connect_outcome(active_now, handoff);
    // Every swap stops the old output (design D10 step 3), whether
    // or not a hand-off was captured: the local run stops; the old
    // receiver's media stops with its stop reports — none where
    // nothing was loaded or reported — and the deliberate output
    // change tears the old receiver's app down (design D12), on the
    // old receiver never the new one. The stop also makes every
    // awaited transition identity the old output could confirm
    // unobservable (the same rule a ctrl stop applies).
    stop_old_output_for_handoff(daemon);
    crate::core::reset_slot_jumps(
        &mut daemon.owner.core.transitions,
        &mut daemon.owner.queued_transition_origin,
    );
    let pending_handoff_load = handoff.map(|handoff| super::PendingHandoffLoad {
        slot: handoff.slot,
        start_ticks: handoff.start_ticks,
        paused: handoff.paused,
        reply_tx,
    });
    install_connected_output(daemon, receiver_id, worker, receiver, pending_handoff_load);
    if let Some(handoff) = handoff {
        // The queue's own active marker follows the handed-off slot,
        // the same bookkeeping a Cast dispatch does.
        let _ = daemon.owner.core.queue.set_active_slot(handoff.slot);
        spawn_handoff_resolve(daemon, handoff);
    }
}

/// Logs what a successful connect means for a captured hand-off (design D10
/// step 3): the old output stops and its slot continues on the new receiver,
/// or the captured playback stopped while connecting and the output swaps
/// with nothing to continue.
fn log_handoff_connect_outcome(active_now: bool, handoff: Option<HandoffPlayback>) {
    if let Some(handoff) = handoff {
        if active_now {
            tracing::info!(
                name: "daemon.cast_handoff.old_output_stopped",
                target: "daemon",
                slot = ?handoff.slot,
                "the connect succeeded; the old output stops and its slot continues on the new receiver"
            );
        } else {
            tracing::info!(
                name: "daemon.cast_handoff.playback_stopped",
                target: "daemon",
                slot = ?handoff.slot,
                "the captured playback stopped while connecting; the output swaps with nothing to continue"
            );
        }
    }
}

/// Installs the connected receiver as the playback output (task 3.2): the
/// output is built clean (row 6.1: the presented status starts clean),
/// logged, installed, re-presented, and broadcast.
fn install_connected_output(
    daemon: &mut DaemonLoop,
    receiver_id: &str,
    worker: mpsc::Sender<CastJob>,
    receiver: CastReceiver,
    pending_handoff_load: Option<super::PendingHandoffLoad>,
) {
    let output = super::CastOutput {
        worker,
        events: daemon.merged_tx.clone(),
        receiver,
        connection: OutputConnection::Connected,
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
        pending_handoff_load,
    };
    tracing::info!(name: "daemon.cast_output.connected", target: "daemon", receiver = %receiver_id, output = ?output, "cast receiver is now the playback output");
    daemon.owner.output = PlaybackOutput::Cast(Box::new(output));
    refresh_presentation(daemon, Instant::now());
    broadcast_output_change(daemon);
}

/// Stops the old output for a successful swap (design D10 step 3): the local
/// run stops, or the old receiver's media stops with its stop reports; when
/// the old output was itself a receiver, the deliberate output change also
/// tears its app down (design D12) — on the old receiver, never the new one.
fn stop_old_output_for_handoff(daemon: &mut DaemonLoop) {
    let old_output_is_cast = matches!(&daemon.owner.output, PlaybackOutput::Cast(_));
    crate::cast::stop_active_output(
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
        crate::cast::queue_teardown_job(old);
    }
}

/// The Cast→Local hand-off (design D10 steps 3-5, row 7.1 dispatch B): the
/// receiver's presented playback is captured, the old output stops — the
/// receiver's media-stop job first, then its app teardown (design D12: only
/// a deliberate output change tears the app down; a user Stop never does) —
/// the output swaps to Local and broadcasts, and a captured slot continues
/// on a fresh local run at the captured position, paused when the receiver
/// was paused (row 7.2). The stop reports go out before the local run
/// starts (design D9).
fn hand_cast_back_to_local(daemon: &mut DaemonLoop, reply_tx: &CtrlSender) {
    // The presented status is the last receiver report extrapolated to now
    // (design D8, row 6.1); one re-derivation pins it to the selection
    // instant without a fresh poll (row 7.1 dispatch B), and the capture
    // reads it.
    refresh_presentation(daemon, Instant::now());
    let handoff = capture_cast_handoff(daemon);
    crate::cast::stop_active_output(
        &daemon.player,
        &mut daemon.owner.output,
        &daemon.owner.core.queue,
        &daemon.ctrl_clients,
    );
    teardown_old_receiver_app(daemon);
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

/// Captures the Cast output's active playback for the hand-off (design D10
/// step 1, row 7.1 dispatch B): the loaded slot with the presented position
/// and pause state — the last receiver report extrapolated, never a fresh
/// poll. The presented status is active only while the receiver plays,
/// buffers, or pauses the loaded item (design D8, row 6.1), so an idle,
/// stopped, or foreign-playing receiver hands nothing over.
fn capture_cast_handoff(daemon: &DaemonLoop) -> Option<HandoffPlayback> {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return None;
    };
    let loaded = cast.loaded.as_ref()?;
    let status = daemon.player.status.lock().unwrap();
    status.active.then(|| HandoffPlayback {
        slot: loaded.slot,
        start_ticks: status.position_ticks,
        paused: status.paused,
    })
}

/// Continues a handed-off slot on a fresh local run (design D10 step 5, row
/// 7.1 dispatch B; row 7.2). A local cold start takes no start position of
/// its own: the resume position is the slot item's recorded progress
/// (`resume_ticks_for_slot` reads it at dispatch), so the captured receiver
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
            name: "daemon.cast_handoff.slot_missing",
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
        name: "daemon.cast_handoff.local_start",
        target: "daemon",
        slot = ?handoff.slot,
        position_ticks = handoff.start_ticks,
        paused = handoff.paused,
        "the handed-off slot continues on a fresh local run"
    );
    if !crate::core::submit_queue_slots_cold_start(
        &daemon.player,
        daemon.owner.core.queue.slot_pairs(),
        start_idx,
        &daemon.client,
    ) {
        tracing::warn!(
            name: "daemon.cast_handoff.local_start.rejected",
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
    let order_before = daemon.owner.core.play_order();
    daemon.owner.core.note_observed_active_slot(None);
    if daemon.owner.core.play_order() != order_before {
        crate::forward_play_order_advance(&daemon.owner.core, &daemon.player);
    }
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

/// Starts the resolve the hand-off load needs (design D10 step 5, row 7.1):
/// the captured slot's Cast source is resolved off the event-loop thread and
/// its outcome re-enters through `CastOwnerEvent::HandoffResolved`. A slot
/// edited out of the queue while the connect ran loads nothing.
fn spawn_handoff_resolve(daemon: &mut DaemonLoop, handoff: HandoffPlayback) {
    let Some(slot) = daemon.owner.core.queue.slot(handoff.slot) else {
        tracing::warn!(
            name: "daemon.cast_handoff.slot_missing",
            target: "daemon",
            slot = ?handoff.slot,
            "the handed-off slot is absent; nothing was loaded"
        );
        return;
    };
    let item = slot.item.clone();
    let resolve_ctx = crate::cast_resolve::owner_resolve_context(
        &daemon.client,
        daemon.audiobookshelf_runtime.as_ref(),
    );
    let merged_tx = daemon.merged_tx.clone();
    let slot_id = handoff.slot;
    crate::cast_resolve::spawn_cast_resolve(item, resolve_ctx, move |result| {
        let _ = merged_tx.send(crate::DaemonEvent::Cast(
            super::CastOwnerEvent::HandoffResolved {
                slot: slot_id,
                result,
            },
        ));
    });
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
                cast.connection = OutputConnection::Connected;
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
        if !matches!(cast.connection, OutputConnection::Connected) {
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
    cast.connection = OutputConnection::Disconnected;
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
