use super::core::DaemonEvent;
use super::{
    AuthorityHolder, ClientRegistry, CtrlClientId, CtrlSender, CtrlTransport, DaemonOwnerContext,
    DaemonPlayerOwner, PendingIdleQueueLoad, SharedQueueState, audio_only_rejection,
    dispatch_slot_jump, reset_slot_jumps, send_to,
};
use mbv_ctrl::player::PlayerCommand;
use mbv_ctrl::{
    CtrlCmd, CtrlEvent, PlaybackGeneration, PlaybackRequestId, QueueLoadRequestId, QueueOpId,
    QueueOpOutcome,
};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_player::{Player, PlayerOwnerState};
use mbv_queue::ExecSlot;
use mbv_queue::QueueSlotId;
use mbv_queue::{PlaybackQueue, QueueItem};
use std::sync::{Arc, Mutex, mpsc};

use super::control_queue::{
    abs_queue_transport_rejection, admit_queue_items, admit_queue_slots, broadcast_queue_state,
    daemon_admits, project_queue_state, unified_queue_state_for_peer,
};

mod playback;
mod queue_edit;
mod queue_load;
mod queue_setup;

pub(crate) fn queue_load_span(
    client_id: CtrlClientId,
    request_id: QueueLoadRequestId,
) -> tracing::Span {
    tracing::info_span!(
        target: "ctrl",
        "queue.load",
        client = %client_id,
        queue_request = request_id,
    )
}

pub(crate) fn intent_span(
    client_id: CtrlClientId,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
) -> tracing::Span {
    tracing::info_span!(
        target: "ctrl",
        "ctrl.intent",
        client = %client_id,
        request = %request_id,
        generation = generation,
    )
}

pub(crate) use playback::play_resolved_items;
pub(crate) use queue_load::{
    cancel_pending_idle_queue_load, cancel_pending_idle_queue_load_if_run_changed,
    complete_pending_idle_queue_load, expire_pending_idle_queue_load,
};

/// Mints the next queue lineage into `SharedQueueState.lineage`, the single
/// source of truth for queue lineage (needed by other threads that seed
/// newly-connecting ctrl clients off `SharedQueueState`), and returns it.
fn mint_queue_lineage(shared_queue: &SharedQueueState) -> mbv_queue::QueueLineage {
    let mut lineage = shared_queue.lineage.lock().unwrap();
    lineage.0 = lineage
        .0
        .checked_add(1)
        .expect("owner queue lineage exhausted");
    *lineage
}

/// Everything a dispatched ctrl command needs. The event loop (and tests)
/// build one per command and pass it whole to `handle_ctrl_for_role`; the
/// per-command-family handlers below take `&mut CtrlContext`, so no handler
/// repeats a long positional parameter list.
pub(super) struct CtrlContext<'a> {
    pub(super) reply_tx: &'a CtrlSender,
    pub(super) client_id: CtrlClientId,
    pub(super) client: &'a Arc<Mutex<EmbyClient>>,
    pub(super) player: &'a Player,
    pub(super) audio_only: bool,
    pub(super) owner: &'a mut DaemonPlayerOwner,
    pub(super) shared_queue: &'a SharedQueueState,
    pub(super) ctrl_clients: &'a ClientRegistry,
    pub(super) has_audiobookshelf: bool,
    pub(super) merged_tx: &'a mpsc::Sender<DaemonEvent>,
    pub(super) owner_settings: super::OwnerSettingsReader,
    pub(super) role: crate::DaemonRole,
    pub(super) op: std::cell::Cell<Option<QueueOpId>>,
}

impl CtrlContext<'_> {
    /// Whether the Emby service is configured. Derived per use instead of
    /// stored: only the event-loop thread mutates the client token, so the
    /// value is stable for the duration of one dispatched command.
    pub(crate) fn has_emby(&self) -> bool {
        !self.client.lock().unwrap().token.is_empty()
    }

    /// Rejection context valid before any handler has taken mutable borrows
    /// of the owner's queue — used by the dispatcher's role gate. Handlers
    /// that reject mid-arm build a `RejectContext` literal instead, because
    /// their outstanding mutable borrows of the owner's queue would conflict
    /// with a whole-context shared borrow here.
    fn rejection_context(&self, lineage: mbv_queue::QueueLineage) -> RejectContext<'_> {
        RejectContext {
            reply_tx: self.reply_tx,
            ctrl_clients: self.ctrl_clients,
            client_id: self.client_id,
            player: self.player,
            queue: &self.owner.core.queue,
            source: &self.owner.core.source,
            lineage,
            op: &self.op,
        }
    }
}

/// The reply and queue snapshot a command rejection needs. Passed whole to
/// `reject_command`; `queue`/`source` are borrowed per rejection because the
/// caller may hold mutable borrows of the owner's queue at that point, so
/// this struct cannot be pre-built from `CtrlContext`.
struct RejectContext<'a> {
    reply_tx: &'a CtrlSender,
    ctrl_clients: &'a ClientRegistry,
    client_id: CtrlClientId,
    player: &'a Player,
    queue: &'a PlaybackQueue,
    source: &'a mbv_queue::QueueSource,
    lineage: mbv_queue::QueueLineage,
    op: &'a std::cell::Cell<Option<QueueOpId>>,
}

/// Sends a command rejection to the requesting client and re-publishes the
/// current queue snapshot so the rejected client's projection resyncs.
fn reject_command(ctx: &RejectContext<'_>, reason: &str) {
    let RejectContext {
        reply_tx,
        ctrl_clients,
        client_id,
        player,
        queue,
        source,
        lineage,
        op,
    } = *ctx;
    if let Some(op) = op.take() {
        send_to(
            reply_tx,
            &CtrlEvent::QueueOpResult {
                op,
                outcome: QueueOpOutcome::Rejected(reason.to_string()),
            },
        );
        return;
    }
    send_to(reply_tx, &CtrlEvent::CommandRejected(reason.to_string()));
    let status = player.status.lock().unwrap().clone();
    let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
    let supports_abs_book_queue = ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_book_queue(client_id);
    send_to(
        reply_tx,
        &unified_queue_state_for_peer(
            &status,
            queue,
            source,
            lineage,
            queue.active_slot_id(),
            None,
            None,
            supports_abs_queue,
            supports_abs_book_queue,
        ),
    );
}

/// Sends the rejection reply for a command whose owner-role gate
/// (`CtrlCmd::requires_owner`) failed. Reply shape/event and reason text are
/// kept per-command, matching what each arm sent before the gate moved here.
/// Matches `OwnerGateRejection` exhaustively: its 3 variants are exactly the
/// gated commands, so there is no wildcard/unreachable arm to fall into.
fn send_role_gate_rejection(rejection: &mbv_ctrl::OwnerGateRejection, ctx: &RejectContext<'_>) {
    match rejection {
        mbv_ctrl::OwnerGateRejection::AdoptQueue => {
            reject_command(ctx, "Stay-alive owner queues cannot be adopted by Clients");
        }
        mbv_ctrl::OwnerGateRejection::QueueLoadIdle { request_id } => {
            queue_load::reject_queue_load(
                ctx.reply_tx,
                *request_id,
                "idle queue loads are supported only by the Stay-alive owner".to_string(),
            );
        }
        mbv_ctrl::OwnerGateRejection::QueueSourceUpdate => reject_command(
            ctx,
            "queue source updates are supported only by the Stay-alive owner",
        ),
    }
}

/// `RequestShutdown` pre-handling: validates the request, persists the queue,
/// and hands broadcast authority back to ctrl when a remote owner shuts down.
/// Returns `false` when the request was rejected; the caller then sends no
/// further reply.
fn prepare_shutdown(ctx: &CtrlContext<'_>) -> bool {
    tracing::info!(name: "daemon.shutdown_request.received", target: "daemon", client = %ctx.client_id, "shutdown request received");
    if (ctx.owner_settings)().stay_alive {
        tracing::info!(name: "daemon.shutdown_request.rejected", target: "daemon", "shutdown request rejected: daemon is in stay-alive mode");
        send_to(
            ctx.reply_tx,
            &CtrlEvent::ShutdownRejected {
                reason: "daemon is in stay-alive mode".to_string(),
            },
        );
        return false;
    }
    let is_local = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .is_local_client(ctx.client_id);
    if !is_local {
        send_to(
            ctx.reply_tx,
            &CtrlEvent::ShutdownRejected {
                reason: "lifecycle requests require local transport".to_string(),
            },
        );
        return false;
    }

    let player_status = ctx.player.status.lock().unwrap().clone();
    let mut queue_state = project_queue_state(
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &player_status,
    );

    if ctx.role != crate::DaemonRole::Local
        && queue_state.items.is_empty()
        && let Some(existing) = mbv_config::load_queue_state()
        && !existing.items.is_empty()
    {
        queue_state = existing;
    }

    if let Err(e) = mbv_config::save_queue_state(&queue_state) {
        tracing::error!(name: "daemon.shutdown_queue_persist.failed", target: "daemon", error = %e, "coordinated shutdown rejected: queue persistence failed");
        send_to(
            ctx.reply_tx,
            &CtrlEvent::ShutdownRejected {
                reason: format!("queue persistence failed: {e}"),
            },
        );
        return false;
    }

    let mut clients = ctx.ctrl_clients.lock().unwrap();
    if clients.authority == AuthorityHolder::EmbyRemote {
        clients.authority = AuthorityHolder::Ctrl;
    }
    true
}

/// `CtrlCmd::Stop`: stop playback and interrupt any in-flight or queued slot
/// jump — a stop makes every awaited transition identity unobservable. Shared
/// verbatim by the `Stop` command and `PlaybackIntentAction::Stop`.
fn handle_stop(ctx: &mut CtrlContext<'_>) {
    let DaemonPlayerOwner {
        core: PlayerOwnerState { transitions, .. },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    ctx.player.stop();
    reset_slot_jumps(transitions, queued_transition_origin);
}

pub(super) fn handle_ctrl_for_role(cmd: CtrlCmd, mut ctx: CtrlContext<'_>) {
    ctx.op.set(queue_op_id(&cmd));
    cancel_pending_idle_queue_load_if_run_changed(ctx.owner, ctx.player);
    // `SharedQueueState.lineage` is the single source of truth (other
    // threads read it for cold ctrl-client snapshots); this is a
    // function-local snapshot so hot-path comparisons below don't
    // re-lock per read; `mint_queue_lineage` below writes the new value
    // straight to `shared_queue.lineage` for the next command's read.
    let queue_lineage = *ctx.shared_queue.lineage.lock().unwrap();
    if ctx.owner.pending_idle_load.is_some() && !matches!(&cmd, CtrlCmd::RequestShutdown) {
        if let CtrlCmd::UnifiedQueueLoadIdle { request_id, .. } = &cmd {
            queue_load::reject_queue_load(
                ctx.reply_tx,
                *request_id,
                "another idle queue load is pending".to_string(),
            );
        } else {
            reject_command(
                &ctx.rejection_context(queue_lineage),
                "owner is finalizing an idle queue load",
            );
        }
        return;
    }
    match cmd.requires_owner() {
        mbv_ctrl::OwnerGate::OwnerOnly(rejection) if ctx.role != crate::DaemonRole::Local => {
            send_role_gate_rejection(&rejection, &ctx.rejection_context(queue_lineage));
            return;
        }
        mbv_ctrl::OwnerGate::NonOwnerOnly(rejection) if ctx.role == crate::DaemonRole::Local => {
            send_role_gate_rejection(&rejection, &ctx.rejection_context(queue_lineage));
            return;
        }
        _ => {}
    }
    if matches!(cmd, CtrlCmd::RequestShutdown) && !prepare_shutdown(&ctx) {
        return;
    }

    dispatch_ctrl_command(cmd, &mut ctx, queue_lineage);
    if let Some(op) = ctx.op.take() {
        answer_queue_op(&ctx, op);
    }
}

fn queue_op_id(cmd: &CtrlCmd) -> Option<QueueOpId> {
    match cmd {
        CtrlCmd::UnifiedQueueReplace { op, .. }
        | CtrlCmd::UnifiedQueueAppend { op, .. }
        | CtrlCmd::UnifiedQueueRemoveSlot { op, .. }
        | CtrlCmd::UnifiedQueueRemoveSlots { op, .. }
        | CtrlCmd::UnifiedQueueMoveSlot { op, .. }
        | CtrlCmd::UnifiedQueuePlaySlot { op, .. }
        | CtrlCmd::UnifiedQueueSourceUpdate { op, .. } => *op,
        CtrlCmd::UnifiedQueueClearOp { op }
        | CtrlCmd::UnifiedQueueRefresh { op }
        | CtrlCmd::UnifiedQueueApplyProgress { op, .. } => Some(*op),
        _ => None,
    }
}

fn answer_queue_op(ctx: &CtrlContext<'_>, op: QueueOpId) {
    let status = ctx.player.status.lock().unwrap().clone();
    let (in_flight, queued_latest) = ctx.owner.core.transitions.summaries();
    let (supports_abs_queue, supports_abs_book_queue) = {
        let clients = ctx.ctrl_clients.lock().unwrap();
        (
            clients.supports_abs_queue(ctx.client_id),
            clients.supports_abs_book_queue(ctx.client_id),
        )
    };
    let event = unified_queue_state_for_peer(
        &status,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        *ctx.shared_queue.lineage.lock().unwrap(),
        *ctx.shared_queue.observed_active_slot.lock().unwrap(),
        in_flight,
        queued_latest,
        supports_abs_queue,
        supports_abs_book_queue,
    );
    let CtrlEvent::UnifiedQueueState(state) = event else {
        unreachable!("queue projection always returns UnifiedQueueState")
    };
    send_to(
        ctx.reply_tx,
        &CtrlEvent::QueueOpResult {
            op,
            outcome: QueueOpOutcome::Applied(Box::new(state)),
        },
    );
}

fn dispatch_ctrl_command(
    cmd: CtrlCmd,
    ctx: &mut CtrlContext<'_>,
    queue_lineage: mbv_queue::QueueLineage,
) {
    match cmd {
        CtrlCmd::Hello(_) => {
            tracing::warn!(name: "daemon.ctrl_client_hello.unexpected", target: "daemon", "unexpected ctrl protocol hello after negotiation");
        }
        CtrlCmd::UnifiedAdoptQueue {
            items,
            cursor,
            source: new_source,
        } => queue_setup::handle_adopt_queue(ctx, queue_lineage, items, cursor, new_source),
        // Stale index-addressed jump from a cross-version peer. Slot-id +
        // request-identity JumpTo is now the only jump path, so an ordinal
        // index carries no evidence about which slot was intended: reject it
        // visibly via the existing command-rejection path (design D6).
        CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::JumpTo(_)) => {
            send_to(
                ctx.reply_tx,
                &CtrlEvent::CommandRejected(
                    "index-addressed queue jump is no longer supported; use unified queue commands"
                        .to_string(),
                ),
            );
        }
        CtrlCmd::PlayerCmd(pc) => {
            ctx.player.send_command(PlayerCommand::from(pc));
        }
        CtrlCmd::Stop => handle_stop(ctx),
        CtrlCmd::PlaybackIntent(intent) => playback::handle_playback_intent(ctx, intent),
        CtrlCmd::RequestShutdown => {
            send_to(ctx.reply_tx, &CtrlEvent::ShutdownAccepted);
            let _ = ctx.merged_tx.send(DaemonEvent::Shutdown);
        }
        CtrlCmd::ApplyServiceSetup { .. } => {}
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id,
            slots,
            cursor,
            source: new_source,
        } => queue_load::handle_queue_load_idle(ctx, request_id, slots, cursor, new_source),
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: new_source,
            lineage: cmd_lineage,
            ..
        } => queue_setup::handle_queue_source_update(ctx, queue_lineage, new_source, cmd_lineage),
        // ── Unified queue commands ──────────────────────────────────────
        CtrlCmd::UnifiedQueueReplace {
            items,
            slots,
            start_idx,
            source: new_source,
            ..
        } => queue_setup::handle_queue_replace(
            ctx,
            queue_lineage,
            items,
            slots,
            start_idx,
            new_source,
        ),
        CtrlCmd::UnifiedQueueAppend { items, before, .. } => {
            queue_setup::handle_queue_append(ctx, queue_lineage, items, before);
        }
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id, .. } => {
            queue_edit::handle_queue_remove_slot(ctx, queue_lineage, slot_id);
        }
        CtrlCmd::UnifiedQueueRemoveSlots { slot_ids, .. } => {
            queue_edit::handle_queue_remove_slots(ctx, slot_ids);
        }
        CtrlCmd::UnifiedQueueMoveSlot {
            slot_id, to_index, ..
        } => {
            queue_edit::handle_queue_move_slot(ctx, queue_lineage, slot_id, to_index);
        }
        CtrlCmd::UnifiedQueuePlaySlot { slot_id, .. } => {
            queue_edit::handle_queue_play_slot(ctx, queue_lineage, slot_id);
        }
        CtrlCmd::UnifiedQueueClear | CtrlCmd::UnifiedQueueClearOp { .. } => {
            queue_edit::handle_queue_clear(ctx);
        }
        CtrlCmd::UnifiedQueueRefresh { .. } | CtrlCmd::UnifiedQueueApplyProgress { .. } => {
            reject_command(
                &ctx.rejection_context(queue_lineage),
                "queue operation is not supported by this owner yet",
            );
        }
    }
}

pub(crate) fn owner_admin_transport_allowed(
    role: crate::DaemonRole,
    kind: mbv_queue::ServiceKind,
    transport: Option<CtrlTransport>,
) -> bool {
    let role_allowed =
        role == crate::DaemonRole::Packaged || kind == mbv_queue::ServiceKind::Audiobookshelf;
    role_allowed && transport == Some(CtrlTransport::Local)
}
