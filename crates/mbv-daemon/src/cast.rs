//! The Owner's playback output: local playback or one Cast receiver
//! (cast-owner-output design D1/D2/D3). The Owner holds exactly one output;
//! the Cast side owns the worker sender, the receiver identity, and the
//! connection state, and every receiver call runs on the Cast worker.
//!
//! The module splits along responsibility:
//! - `connect`: output selection, connect and reconnect outcomes, and the
//!   disconnect-and-reconnect recovery;
//! - `status`: the status-poll handler, the transition gate's Cast settle,
//!   and the presented `PlayerStatus` (design D8);
//! - `command`: routing transport commands onto the receiver.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use mbv_cast::{CastJob, CastReceiver, CastStatus};
use mbv_ctrl::{CtrlEvent, OutputState, PlaybackGeneration};
use mbv_queue::QueueSlotId;

use crate::CtrlClientId;

use super::DaemonLoop;
use super::core::DaemonEvent;

pub(crate) mod advance;
mod command;
mod connect;
mod status;

pub(super) use command::{route_player_cmd, route_transport_command, set_receiver_paused};
pub use connect::{CastConnectOutcome, CastConnectRequest, CastConnector, local_cast_connector};
pub(super) use connect::{PendingCastConnect, handle_select_playback_output};

/// The keep-alive cadence (design D8, task 3.3). The receiver terminates a
/// sender whose PONGs lapse in well under ten seconds, and the 7s poll sat
/// at that deadline while a load exchange occupied the worker; pinging every
/// 3s keeps a pong inside the deadline (TUI `cast_status.rs`, moved here).
pub(crate) const CAST_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(3);

/// The status-poll cadence (design D8, task 3.3); design targets 5-10s.
pub(crate) const CAST_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(7);

/// How long a submitted poll may go without a worker event before the worker
/// counts as wedged (design D8, task 3.3). `rust_cast` operations have no
/// socket timeout, so one unresponsive receiver exchange blocks the single
/// worker thread forever; the reconnect is the recovery.
pub(crate) const CAST_JOB_ANSWER_BOUND: Duration = Duration::from_secs(20);

/// The single playback output a Player owner plays through (design D1): the
/// type system makes two active outputs unrepresentable. The Cast side is
/// boxed: it carries the worker sender, the last status, and the loaded item
/// while `Local` carries nothing.
#[derive(Default)]
pub(crate) enum PlaybackOutput {
    #[default]
    Local,
    Cast(Box<CastOutput>),
}

/// One Cast receiver as the Owner's output (design D1). The worker sender is
/// the only path to the receiver: the `CastClient` itself lives on the worker
/// thread and is never `Send`. `last_status` is the most recent poll result;
/// `loaded` is the item the receiver was told to play. The ABS lifecycle of a
/// loaded episode joins it in task 6.4.
pub(crate) struct CastOutput {
    /// The only path to the receiver: the `CastClient` lives on the worker
    /// thread and is never `Send`. `pub(super)` so the crate's tests can
    /// submit sentinel jobs that make asynchronous job order assertable.
    pub(super) worker: mpsc::Sender<CastJob>,
    receiver: CastReceiver,
    connection: mbv_ctrl::CastConnection,
    last_status: Option<CastStatus>,
    status_at: Option<Instant>,
    /// The item the receiver was told to play. `pub(super)` so the crate's
    /// tests can assert what a resolve outcome loaded.
    pub(super) loaded: Option<CastLoadedItem>,
    /// Cast tick timers (design D8, task 3.3), owned by the output so they
    /// travel with it. `last_keep_alive` paces the 3s pong job; `last_poll`
    /// paces the 7s status poll; `poll_sent` arms the 20s worker watchdog
    /// while a poll is in flight — the id is stamped into the poll's job and
    /// echoed by its result, so a late result from a previous worker (the
    /// watchdog reconnect race) cannot match — and is disarmed by the
    /// matching result; `reconnecting` marks an in-flight reconnect attempt
    /// whose outcome is still due.
    last_keep_alive: Instant,
    last_poll: Option<Instant>,
    poll_sent: Option<(u64, Instant)>,
    /// Mints the monotonic id of the next status poll (review P2 on row
    /// 3.3): ids outlive a reconnect, so an old worker's late result can
    /// never be mistaken for the in-flight poll.
    next_poll_id: u64,
    reconnecting: bool,
    /// The advance a natural end started and that uncastable step targets
    /// may still be stepping through (design D7, row 6.2); `None` when the
    /// output has no advance under way. `pub(super)` so the crate's resolve
    /// handler can end it with the step's load.
    pub(super) advance: Option<advance::CastAdvance>,
}

/// The item the Cast receiver was told to play (design D1). `end_handled`
/// marks the end report the Owner already acted on: the second poll of the
/// same `Finished` must not advance a second time, and the same `Error` must
/// not be reported again (design D7, row 6.2).
pub(crate) struct CastLoadedItem {
    pub(super) slot: QueueSlotId,
    pub(super) content_id: String,
    /// The resolved item's progress target (design D1); the provider
    /// reporting of task 6.4 feeds from it.
    pub(super) progress: crate::cast_resolve::CastProgressTarget,
    pub(super) end_handled: bool,
}

impl std::fmt::Debug for CastLoadedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CastLoadedItem")
            .field("slot", &self.slot)
            .field("content_id", &self.content_id)
            .field("progress", &self.progress)
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
            .field("last_keep_alive", &self.last_keep_alive)
            .field("last_poll", &self.last_poll)
            .field("poll_sent", &self.poll_sent)
            .field("next_poll_id", &self.next_poll_id)
            .field("reconnecting", &self.reconnecting)
            .field("advance", &self.advance)
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

/// A Cast source resolve in flight (design D4, task 4.2): the transition it
/// belongs to (its target slot is the slot the resolve was started for, its
/// identity is the transition's), the position the resolved media starts at,
/// and the ctrl client that issued the command, for the failure report. The
/// resolve outcome re-enters through `CastOwnerEvent::Resolved`; one whose
/// slot or generation no longer matches this record belongs to a superseded
/// target and is dropped.
pub(crate) struct PendingCastResolve {
    pub(super) transition: mbv_player::transition::Transition,
    pub(super) start_ticks: i64,
    pub(super) origin: Option<CtrlClientId>,
}

/// One Cast-output event from the Cast connector or worker (cast-owner-output
/// design D2/D4/D8), carried to the owner's event loop as `DaemonEvent::Cast`:
/// connect and reconnect outcomes, status-poll results from the Cast timers on
/// the daemon tick, and Cast source resolve outcomes.
#[derive(Debug)]
pub enum CastOwnerEvent {
    Connect {
        receiver_id: String,
        outcome: CastConnectOutcome,
    },
    /// One status-poll result from the Cast worker (design D8, task 3.3).
    /// `poll_id` echoes the id stamped into the poll's job; a result whose
    /// id does not match the in-flight poll is a late result from a previous
    /// worker and is dropped. `Err` carries the rendered `CastError`: a poll
    /// error is a broken connection, not a slow receiver (`rust_cast`
    /// receives block rather than time out), so the handler disconnects and
    /// reconnects.
    Status {
        receiver_id: String,
        poll_id: u64,
        result: Result<CastStatus, String>,
    },
    /// The outcome of the automatic reconnect a poll error or a watchdog
    /// timeout spawned (design D8/D11, task 3.3). Not a user selection: a
    /// success restores the existing output's worker, a failure leaves it
    /// `Disconnected` (task 7.4 refines the outcomes).
    Reconnect {
        receiver_id: String,
        outcome: CastConnectOutcome,
    },
    /// One Cast source resolve outcome (cast-owner-output design D4, task
    /// 4.2), keyed by the slot the resolve was started for and the transition
    /// generation it belongs to. A result whose pair no longer matches the
    /// owner's pending resolve is a stale result for a superseded target and
    /// is dropped without a load.
    Resolved {
        slot: QueueSlotId,
        generation: PlaybackGeneration,
        result: Result<crate::cast_resolve::CastSource, crate::cast_resolve::Uncastable>,
    },
    /// The receiver refused the load the owner submitted for `slot` (design
    /// D5): the loaded record is dropped and the in-flight transition the
    /// load belonged to expires, so later steps are not wedged behind it.
    LoadFailed { slot: QueueSlotId, error: String },
}

/// `DaemonEvent::Cast`: apply one Cast-worker or connector outcome to the
/// owner's output (tasks 3.2/3.3). A connect failure keeps the current output
/// and reports to the selecting Client; a success with no active slot swaps
/// the output and broadcasts it to peers that negotiated `playback-output`.
/// A poll error disconnects and reconnects (design D8); a reconnect outcome
/// restores the existing output or leaves it `Disconnected`.
pub(super) fn handle_cast_event(daemon: &mut DaemonLoop, event: CastOwnerEvent) {
    match event {
        CastOwnerEvent::Connect {
            receiver_id,
            outcome,
        } => connect::handle_connect_outcome(daemon, &receiver_id, outcome),
        CastOwnerEvent::Status {
            receiver_id,
            poll_id,
            result,
        } => status::handle_cast_status(daemon, &receiver_id, poll_id, result),
        CastOwnerEvent::Reconnect {
            receiver_id,
            outcome,
        } => connect::handle_reconnect_outcome(daemon, &receiver_id, outcome),
        CastOwnerEvent::Resolved {
            slot,
            generation,
            result,
        } => crate::cast_resolve::handle_cast_resolved(daemon, slot, generation, result),
        CastOwnerEvent::LoadFailed { slot, error } => handle_load_failed(daemon, slot, &error),
    }
}

/// The `DaemonOwnerContext` the Cast handlers need, built from the loop in
/// one place so the field list cannot drift between call sites.
pub(super) fn owner_context(daemon: &mut DaemonLoop) -> super::core::DaemonOwnerContext<'_> {
    super::core::DaemonOwnerContext {
        player: &daemon.player,
        client: &daemon.client,
        owner: &mut daemon.owner,
        shared_queue: &daemon.shared_queue,
        ctrl_clients: &daemon.ctrl_clients,
        merged_tx: &daemon.merged_tx,
        audiobookshelf: daemon.audiobookshelf_runtime.as_ref(),
    }
}

/// Dispatches one Cast load for an accepted transition (design D4, row 5.1):
/// the target slot becomes the queue's active slot, the resolve is registered
/// as the owner's pending Cast resolve, and the resolve runs off the
/// event-loop thread. Its outcome re-enters through `CastOwnerEvent::Resolved`;
/// on success the resolved media loads at `start_ticks`, on failure the
/// transition expires and the failure is reported (design D5). Returns whether
/// the dispatch started.
pub(super) fn dispatch_cast_load(
    ctx: &mut super::core::DaemonOwnerContext<'_>,
    transition: mbv_player::transition::Transition,
    start_ticks: i64,
    origin: Option<CtrlClientId>,
) -> bool {
    let Some(slot) = ctx.owner.core.queue.slot(transition.target) else {
        tracing::warn!(
            name: "daemon.cast_dispatch.slot_missing",
            target: "daemon",
            slot = ?transition.target,
            "the jump target slot is absent; nothing was loaded"
        );
        return false;
    };
    let item = slot.item.clone();
    // The queue's own active marker follows the target: the snapshot that
    // the caller broadcasts resolves it once the status confirms the load.
    let _ = ctx.owner.core.queue.set_active_slot(transition.target);
    let resolve_ctx = crate::cast_resolve::owner_resolve_context(ctx.client, ctx.audiobookshelf);
    ctx.owner.pending_cast_resolve = Some(PendingCastResolve {
        transition,
        start_ticks,
        origin,
    });
    let merged_tx = ctx.merged_tx.clone();
    let slot_id = transition.target;
    let generation = transition.generation;
    tracing::info!(
        name: "daemon.cast_dispatch.resolve_started",
        target: "daemon",
        slot = ?slot_id,
        generation,
        "resolving the slot's cast source"
    );
    crate::cast_resolve::spawn_cast_resolve(item, resolve_ctx, move |result| {
        let _ = merged_tx.send(DaemonEvent::Cast(CastOwnerEvent::Resolved {
            slot: slot_id,
            generation,
            result,
        }));
    });
    true
}

/// Expires the in-flight transition a failed Cast load or resolve belonged to
/// (design D5, row 5.2), dispatching a queued transition the expiry promotes.
/// The caller reports the failure; this helper owns the gate and the publish.
/// Returns whether a queued transition was promoted, so the caller can tell a
/// user takeover from an expiry that left the queue to the caller (design
/// D7's step-over).
pub(super) fn expire_cast_transition(ctx: &mut super::core::DaemonOwnerContext<'_>) -> bool {
    if let Some((expired, dispatch_next)) = ctx.owner.core.transitions.expire_now() {
        tracing::info!(
            name: "daemon.cast_transition.expired",
            target: "transition",
            request = %expired.request_id,
            slot = ?expired.target,
            dispatch_next = dispatch_next.is_some(),
            "the failed cast load expired its transition"
        );
        // The expiry abandons the guarded intent that drove the transition,
        // mirroring `expire_and_redispatch`: without the rejection the intent
        // stays current and the next identical play coalesces into it and
        // does nothing.
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
        let origin = super::core::take_promoted_origin(ctx, dispatch_next.as_ref());
        if let Some(next) = dispatch_next {
            super::core::send_jump_or_cold_start(ctx, next, origin);
            return true;
        }
    }
    super::broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
    false
}

/// Reports a failed Cast resolve to its origin (row 5.1): the item and the
/// reason reach the ctrl client that issued the play or jump, and a current
/// guarded play intent keyed to the same request is rejected so it cannot
/// wedge the guarded protocol.
pub(super) fn report_resolve_failure(
    ctx: &mut super::core::DaemonOwnerContext<'_>,
    failed: &mbv_player::transition::Transition,
    origin: Option<CtrlClientId>,
    uncastable: &crate::cast_resolve::Uncastable,
) {
    let report = format!("{} ({})", uncastable.item, uncastable.reason);
    if let Some((connection_id, request_id, generation)) = ctx
        .owner
        .intents
        .current
        .as_ref()
        .filter(|current| {
            current.request_id == failed.request_id && current.generation == failed.generation
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
            mbv_ctrl::PlaybackIntentRejection::ResolutionFailed,
        )
    {
        ctx.ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
    }
    if let Some(connection_id) = origin {
        ctx.ctrl_clients
            .lock()
            .unwrap()
            .send_to_client(connection_id, &CtrlEvent::CommandRejected(report));
    }
}

/// Stops the active output's media (design D4, row 5.3): Local stops the
/// run; Cast stops the receiver media and forgets the loaded item, so a
/// later idle status of the stopped receiver can neither settle nor advance
/// anything (design D5/D7). Shared by the ctrl stop, the guarded stop
/// intent, and the transport (MPRIS/tray) stop, which mirrors `CtrlCmd::Stop`.
/// The Cast arm also re-presents the output: with the loaded item gone the
/// presented status shows no active playback (design D8, row 6.1).
pub(super) fn stop_active_output(
    player: &mbv_player::Player,
    output: &mut PlaybackOutput,
    queue: &mbv_queue::PlaybackQueue,
) {
    match output {
        PlaybackOutput::Local => player.stop(),
        PlaybackOutput::Cast(cast) => {
            let worker = cast.worker.clone();
            cast.loaded = None;
            // The stop ends any advance the loaded item's end started (design
            // D7): a stopped output advances nothing.
            cast.advance = None;
            command::submit_command_job(&worker, "stop", |transport| transport.stop());
            let base = player.status.lock().unwrap().clone();
            let presented = status::presented_player_status(
                &base,
                queue,
                None,
                cast.last_status.as_ref(),
                Duration::ZERO,
            );
            *player.status.lock().unwrap() = presented;
        }
    }
}

/// The receiver refused the load (design D5, row 5.2): the loaded record is
/// dropped and the in-flight transition whose target was that slot expires.
fn handle_load_failed(daemon: &mut DaemonLoop, slot: QueueSlotId, error: &str) {
    tracing::warn!(
        name: "daemon.cast_load.failed",
        target: "daemon",
        slot = ?slot,
        error = %error,
        "the receiver did not take the load"
    );
    {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        if cast
            .loaded
            .as_ref()
            .is_none_or(|loaded| loaded.slot != slot)
        {
            return;
        }
        cast.loaded = None;
    }
    status::refresh_presentation(daemon, Instant::now());
    let mut ctx = owner_context(daemon);
    expire_cast_transition(&mut ctx);
}

/// Cast tick timers (design D8, task 3.3), driven by `DaemonLoop::tick` with
/// the loop's injected clock. On a connected output the tick submits the 3s
/// pong job and the 7s status poll (which pongs first) as jobs on the Cast
/// worker; results come back as `DaemonEvent::Cast`. The 20s worker watchdog
/// moves with the poll: a poll with no worker event within the answer bound
/// is a wedged worker and gets the disconnect-and-reconnect recovery.
/// A disconnected output runs no jobs; its reconnect attempt is in flight.
/// While the output is connected the tick also advances the presented
/// position (design D8, row 6.1): extrapolation from the last report.
pub(super) fn tick_cast(daemon: &mut DaemonLoop, now: Instant) {
    let merged_tx = daemon.merged_tx.clone();
    let mut connected = false;
    let wedged = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        if matches!(cast.connection, mbv_ctrl::CastConnection::Connected) {
            connected = true;
            if now.duration_since(cast.last_keep_alive) >= CAST_KEEP_ALIVE_INTERVAL {
                submit_keep_alive_job(&cast.worker);
                cast.last_keep_alive = now;
            }
            if cast.poll_sent.is_none()
                && cast
                    .last_poll
                    .is_none_or(|at| now.duration_since(at) >= CAST_STATUS_POLL_INTERVAL)
            {
                let poll_id = cast.next_poll_id;
                cast.next_poll_id += 1;
                submit_status_poll_job(&cast.worker, &cast.receiver.id, merged_tx, poll_id);
                cast.poll_sent = Some((poll_id, now));
            }
        }
        cast.poll_sent
            .is_some_and(|(_, sent)| now.duration_since(sent) >= CAST_JOB_ANSWER_BOUND)
    };
    if wedged {
        tracing::warn!(name: "cast.worker.unresponsive", target: "cast", "cast worker produced no event within the answer bound");
        connect::disconnect_and_reconnect(daemon);
        return;
    }
    if connected {
        status::refresh_presentation(daemon, now);
    }
}

/// Submits one pong-only job (design D8). It produces no worker event and
/// never touches the watchdog clock: a socket this job finds dead is reported
/// by the next status poll.
fn submit_keep_alive_job(worker: &mpsc::Sender<CastJob>) {
    let _ = worker.send(Box::new(|transport: &mut dyn mbv_cast::CastTransport| {
        if let Err(error) = transport.keep_alive() {
            tracing::warn!(name: "cast.keep_alive.failed", target: "cast", error = %error, "cast keep-alive failed");
        }
    }));
}

/// Submits one status poll (design D8): a belt-and-braces pong ahead of the
/// status exchange, then the status result — stamped with `poll_id` — back
/// as `DaemonEvent::Cast`.
fn submit_status_poll_job(
    worker: &mpsc::Sender<CastJob>,
    receiver_id: &str,
    merged_tx: mpsc::Sender<DaemonEvent>,
    poll_id: u64,
) {
    let receiver_id = receiver_id.to_string();
    let _ = worker.send(Box::new(move |transport: &mut dyn mbv_cast::CastTransport| {
        if let Err(error) = transport.keep_alive() {
            tracing::warn!(name: "cast.keep_alive.failed", target: "cast", error = %error, "cast keep-alive failed");
        }
        let result = transport
            .status()
            .map_err(|error| error.to_string());
        let _ = merged_tx.send(DaemonEvent::Cast(CastOwnerEvent::Status {
            receiver_id,
            poll_id,
            result,
        }));
    }));
}
