//! The Owner's playback output: local playback or one Cast receiver
//! (cast-owner-output design D1/D2/D3). The Owner holds exactly one output;
//! the Cast side owns the worker sender, the receiver identity, and the
//! connection state, and every receiver call runs on the Cast worker.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use mbv_cast::{
    CastClient, CastError, CastJob, CastReceiver, CastStatus, CastTransport, ConnectMode,
    spawn_cast_worker,
};
use mbv_ctrl::player::PlayerCommand;
use mbv_ctrl::{CastConnection, CtrlEvent, OutputSelection, OutputState, PlaybackGeneration};
use mbv_queue::QueueSlotId;

use crate::{CtrlClientId, CtrlContext};

use super::DaemonLoop;
use super::core::DaemonEvent;
use super::ctrl::{CtrlSender, send_to};

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
/// The ABS lifecycle of a loaded episode joins it in task 6.4.
pub(crate) struct CastOutput {
    /// The only path to the receiver: the `CastClient` lives on the worker
    /// thread and is never `Send`. `pub(super)` so the crate's tests can
    /// submit sentinel jobs that make asynchronous job order assertable.
    pub(super) worker: mpsc::Sender<CastJob>,
    receiver: CastReceiver,
    connection: CastConnection,
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
}

/// The item the Cast receiver was told to play (design D1). `end_handled`
/// stops a second poll of the same `Finished` entry from advancing twice
/// (design D7); task 6.2 owns it.
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
                    let _ = merged_tx.send(DaemonEvent::Cast(CastOwnerEvent::Connect {
                        receiver_id,
                        outcome,
                    }));
                }
            });
        }
    }
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
        } => handle_connect_outcome(daemon, &receiver_id, outcome),
        CastOwnerEvent::Status {
            receiver_id,
            poll_id,
            result,
        } => handle_cast_status(daemon, &receiver_id, poll_id, result),
        CastOwnerEvent::Reconnect {
            receiver_id,
            outcome,
        } => handle_reconnect_outcome(daemon, &receiver_id, outcome),
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
/// event-loop thread. Its outcome re-enters through
/// `CastOwnerEvent::Resolved`; on success the resolved media loads at
/// `start_ticks`, on failure the transition expires and the failure is
/// reported (design D5). Returns whether the dispatch started.
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
pub(super) fn expire_cast_transition(ctx: &mut super::core::DaemonOwnerContext<'_>) {
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
pub(super) fn stop_active_output(player: &mbv_player::Player, output: &mut PlaybackOutput) {
    match output {
        PlaybackOutput::Local => player.stop(),
        PlaybackOutput::Cast(cast) => {
            let worker = cast.worker.clone();
            cast.loaded = None;
            submit_command_job(&worker, "stop", |transport| transport.stop());
        }
    }
}

/// Routes one `CtrlCmd::PlayerCmd` wire command to the active output (design
/// D4, row 5.3). Local forwards it to the run unchanged. On a Cast output the
/// transport commands act on the receiver — `TogglePause` picks its direction
/// from the receiver's last reported state, a relative `Seek` starts from the
/// receiver's reported position, and the 0-100 wire volume scales to the
/// receiver's 0-1 level; track selection has no cast primitive and is
/// rejected with a reason; the next-up and skip-intro overlays are mpv script
/// messages with no receiver counterpart, a documented no-op. `JumpTo` never
/// reaches here: the dispatcher rejects it before conversion.
pub(super) fn route_player_cmd(ctx: &mut CtrlContext<'_>, cmd: mbv_ctrl::WireCommand) {
    let PlaybackOutput::Cast(cast) = &mut ctx.owner.output else {
        ctx.player.send_command(PlayerCommand::from(cmd));
        return;
    };
    match cmd {
        mbv_ctrl::WireCommand::TogglePause => {
            let paused = matches!(
                cast.last_status.as_ref().map(|status| status.state),
                Some(mbv_cast::CastPlaybackState::Paused)
            );
            let worker = cast.worker.clone();
            submit_command_job(&worker, "toggle_pause", move |transport| {
                if paused {
                    transport.play()
                } else {
                    transport.pause()
                }
            });
        }
        mbv_ctrl::WireCommand::Seek(delta) => {
            let base = f64::from(
                cast.last_status
                    .as_ref()
                    .and_then(|status| status.position_seconds)
                    .unwrap_or(0.0),
            );
            let target = cast_seconds_f32((base + delta).max(0.0));
            let worker = cast.worker.clone();
            submit_command_job(&worker, "seek", move |transport| transport.seek(target));
        }
        mbv_ctrl::WireCommand::SeekAbsolute(seconds) => {
            let seconds = cast_seconds_f32(seconds);
            let worker = cast.worker.clone();
            submit_command_job(&worker, "seek", move |transport| transport.seek(seconds));
        }
        mbv_ctrl::WireCommand::SetVolume(volume) => {
            let volume = u8::try_from(volume.clamp(0, 100)).unwrap_or(u8::MAX);
            let level = f32::from(volume) / 100.0;
            let worker = cast.worker.clone();
            submit_command_job(&worker, "set_volume", move |transport| {
                transport.set_volume(level)
            });
        }
        mbv_ctrl::WireCommand::SetMute(muted) => {
            let worker = cast.worker.clone();
            submit_command_job(&worker, "set_muted", move |transport| {
                transport.set_muted(muted)
            });
        }
        mbv_ctrl::WireCommand::SetAudio(_)
        | mbv_ctrl::WireCommand::SetSub(_)
        | mbv_ctrl::WireCommand::SetSubtitlePrefs { .. } => {
            send_to(
                ctx.reply_tx,
                &CtrlEvent::CommandRejected(
                    "audio and subtitle track selection is not supported on a cast receiver"
                        .to_string(),
                ),
            );
        }
        mbv_ctrl::WireCommand::NextUpShow { .. }
        | mbv_ctrl::WireCommand::NextUpDismiss
        | mbv_ctrl::WireCommand::SkipIntroDismiss => {
            // Documented no-op: these drive the mpv next-up and skip-intro
            // overlays, which a receiver has no counterpart for.
        }
        mbv_ctrl::WireCommand::JumpTo(_) => {
            unreachable!("inbound WireCommand::JumpTo is rejected before conversion (design D6)")
        }
    }
}

/// Narrows a wire seconds value to the receiver protocol's f32. The approved
/// cast the TUI's cast target carried (`src/app/state/playback_target/cast.rs`,
/// issue #804) moves here as the TUI cast runtime is deleted (row 8.3): no
/// lossless f64-to-f32 conversion exists, and `rust_cast`'s seek is f32.
#[expect(
    clippy::cast_possible_truncation,
    reason = "seek seconds narrow to the receiver protocol's f32; no lossless conversion exists (approved, issue #804)"
)]
fn cast_seconds_f32(value: f64) -> f32 {
    value as f32
}

/// Submits one receiver-command job (design D4, row 5.3): the operation runs
/// on the Cast worker against the transport it owns; a failure is logged — a
/// dead connection is reported by the next status poll, which disconnects
/// and reconnects (design D8).
fn submit_command_job(
    worker: &mpsc::Sender<CastJob>,
    command: &'static str,
    op: impl FnOnce(&mut dyn CastTransport) -> Result<(), CastError> + Send + 'static,
) {
    let _ = worker.send(Box::new(move |transport: &mut dyn CastTransport| {
        if let Err(error) = op(transport) {
            tracing::warn!(
                name: "cast.command.failed",
                target: "cast",
                command,
                error = %error,
                "the receiver refused the command"
            );
        }
    }));
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
    let mut ctx = owner_context(daemon);
    expire_cast_transition(&mut ctx);
}

fn handle_connect_outcome(daemon: &mut DaemonLoop, receiver_id: &str, outcome: CastConnectOutcome) {
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
                last_keep_alive: Instant::now(),
                last_poll: None,
                poll_sent: None,
                next_poll_id: 0,
                reconnecting: false,
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

/// Cast tick timers (design D8, task 3.3), driven by `DaemonLoop::tick` with
/// the loop's injected clock. On a connected output the tick submits the 3s
/// pong job and the 7s status poll (which pongs first) as jobs on the Cast
/// worker; results come back as `DaemonEvent::Cast`. The 20s worker watchdog
/// moves with the poll: a poll with no worker event within the answer bound
/// is a wedged worker and gets the disconnect-and-reconnect recovery.
/// A disconnected output runs no jobs; its reconnect attempt is in flight.
pub(super) fn tick_cast(daemon: &mut DaemonLoop, now: Instant) {
    let merged_tx = daemon.merged_tx.clone();
    let wedged = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        if matches!(cast.connection, CastConnection::Connected) {
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
        disconnect_and_reconnect(daemon);
    }
}

/// Submits one pong-only job (design D8). It produces no worker event and
/// never touches the watchdog clock: a socket this job finds dead is reported
/// by the next status poll.
fn submit_keep_alive_job(worker: &mpsc::Sender<CastJob>) {
    let _ = worker.send(Box::new(|transport: &mut dyn CastTransport| {
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
    let _ = worker.send(Box::new(move |transport: &mut dyn CastTransport| {
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

/// One status-poll result (design D8, task 3.3). Only the result whose id
/// matches the in-flight poll — the same id stamped into that poll's job —
/// disarms the watchdog clock: a late result from a previous worker (the
/// watchdog reconnect race) carries a stale id and is dropped, so it can
/// neither spuriously disconnect the restored worker nor disturb the new
/// poll's bookkeeping. A returned status is stored (task 6.1 owns its
/// translation and extrapolation); a poll error is a broken connection, so
/// the output disconnects and reconnects.
fn handle_cast_status(
    daemon: &mut DaemonLoop,
    receiver_id: &str,
    poll_id: u64,
    result: Result<CastStatus, String>,
) {
    let (error, settle_slot) = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        if cast.receiver.id != receiver_id
            || !matches!(cast.connection, CastConnection::Connected)
            || cast
                .poll_sent
                .is_none_or(|(in_flight, _)| in_flight != poll_id)
        {
            return;
        }
        match result {
            Ok(status) => {
                cast.last_status = Some(status);
                cast.status_at = Some(Instant::now());
                cast.poll_sent = None;
                cast.last_poll = Some(Instant::now());
                (None, settle_check(&cast))
            }
            Err(error) => {
                cast.poll_sent = None;
                (Some(error), None)
            }
        }
    };
    if let Some(error) = error {
        tracing::warn!(name: "cast.status_poll.failed", target: "cast", receiver = %receiver_id, error = %error, "cast status poll failed; reconnecting");
        disconnect_and_reconnect(daemon);
        return;
    }
    if let Some(loaded_slot) = settle_slot {
        settle_cast_transition(daemon, loaded_slot);
    }
}

/// Whether a stored status settles the loaded item's transition (design D5,
/// row 5.2): the status names the loaded content id in a playing, buffering,
/// or paused state — the Cast counterpart of the run's `TrackChanged`
/// confirmation. Returns the loaded slot when it does.
fn settle_check(cast: &CastOutput) -> Option<QueueSlotId> {
    let loaded = cast.loaded.as_ref()?;
    let status = cast.last_status.as_ref()?;
    let confirmed = matches!(
        status.state,
        mbv_cast::CastPlaybackState::Playing
            | mbv_cast::CastPlaybackState::Buffering
            | mbv_cast::CastPlaybackState::Paused
    ) && status.playing_content_id.as_deref() == Some(loaded.content_id.as_str());
    confirmed.then_some(loaded.slot)
}

/// Settles the in-flight transition a confirming receiver status closes (row
/// 5.2): the loaded slot becomes the observed active slot, the transition
/// settles, a queued transition the settle promotes is dispatched, and the
/// guarded play intent the load carried is applied. Mirrors the run's
/// `TrackChanged` handling (observe → settle → publish).
fn settle_cast_transition(daemon: &mut DaemonLoop, loaded_slot: QueueSlotId) {
    // Only the transition the loaded item belongs to settles here: a newer
    // in-flight transition targets a slot the receiver has not confirmed.
    let Some(in_flight) = daemon
        .owner
        .core
        .transitions
        .in_flight()
        .filter(|t| t.target == loaded_slot)
    else {
        return;
    };
    let mut ctx = owner_context(daemon);
    let order_before = ctx.owner.core.play_order();
    ctx.owner.core.note_observed_active_slot(Some(loaded_slot));
    if ctx.owner.core.play_order() != order_before {
        super::forward_play_order_advance(&ctx.owner.core, ctx.player);
    }
    super::core::settle_and_redispatch(&mut ctx, in_flight.request_id, loaded_slot);
    // The guarded play intent the load registered under settles with its
    // transition; without this a Cast play would sit unresolved and coalesce
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
    super::broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
}

/// The reconnect a poll error or a watchdog timeout spawned (design D8/D11,
/// task 3.3). A success restores the existing output's worker and connection;
/// a failure leaves the output `Disconnected` (task 7.4 refines the recovery
/// outcomes). A displaced outcome — the output moved on or was replaced — is
/// discarded, and its worker sender dropped to end the thread.
fn handle_reconnect_outcome(
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
fn disconnect_and_reconnect(daemon: &mut DaemonLoop) {
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
            let _ = merged_tx.send(DaemonEvent::Cast(CastOwnerEvent::Reconnect {
                receiver_id,
                outcome,
            }));
        }
    });
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
