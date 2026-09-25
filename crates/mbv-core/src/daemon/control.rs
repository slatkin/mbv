use super::core::DaemonEvent;
use super::*;
use crate::api::EmbyClient;
use crate::api::EmbyItem;
use crate::ctrl::{CtrlCmd, CtrlEvent};
use crate::playback_execution_sequence::ExecSlot;
use crate::playback_queue::{PlaybackQueue, QueueItem, QueueSlotId};
use crate::player::{Player, PlayerCommand, PlayerOwnerState};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::control_queue::*;

mod playback;
mod queue_edit;
mod queue_load;
mod queue_setup;

pub(in crate::daemon) use playback::play_resolved_items;
pub(in crate::daemon) use queue_load::{
    cancel_pending_idle_queue_load, cancel_pending_idle_queue_load_if_run_changed,
    complete_pending_idle_queue_load, expire_pending_idle_queue_load,
};

/// Mints the next queue lineage into `SharedQueueState.lineage`, the single
/// source of truth for queue lineage (needed by other threads that seed
/// newly-connecting ctrl clients off `SharedQueueState`), and returns it.
fn mint_queue_lineage(shared_queue: &SharedQueueState) -> crate::ctrl::QueueLineage {
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
    pub(super) stay_alive: bool,
    pub(super) role: crate::daemon::DaemonRole,
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
    fn rejection_context(&self, lineage: crate::ctrl::QueueLineage) -> RejectContext<'_> {
        RejectContext {
            reply_tx: self.reply_tx,
            ctrl_clients: self.ctrl_clients,
            client_id: self.client_id,
            player: self.player,
            queue: &self.owner.core.queue,
            source: &self.owner.core.source,
            lineage,
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
    source: &'a crate::config::QueueSource,
    lineage: crate::ctrl::QueueLineage,
}

/// Sends a command rejection to the requesting client and re-publishes the
/// current queue snapshot so the rejected client's projection resyncs.
fn reject_command(ctx: RejectContext<'_>, reason: String) {
    let RejectContext {
        reply_tx,
        ctrl_clients,
        client_id,
        player,
        queue,
        source,
        lineage,
    } = ctx;
    send_to(reply_tx, &CtrlEvent::CommandRejected(reason));
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
fn send_role_gate_rejection(rejection: crate::ctrl::OwnerGateRejection, ctx: RejectContext<'_>) {
    match rejection {
        crate::ctrl::OwnerGateRejection::AdoptQueue => reject_command(
            ctx,
            "Stay-alive owner queues cannot be adopted by Clients".to_string(),
        ),
        crate::ctrl::OwnerGateRejection::QueueLoadIdle { request_id } => {
            queue_load::reject_queue_load(
                ctx.reply_tx,
                request_id,
                "idle queue loads are supported only by the Stay-alive owner".to_string(),
            )
        }
        crate::ctrl::OwnerGateRejection::QueueSourceUpdate => send_to(
            ctx.reply_tx,
            &CtrlEvent::CommandRejected(
                "queue source updates are supported only by the Stay-alive owner".to_string(),
            ),
        ),
    }
}

/// `RequestShutdown` pre-handling: validates the request, persists the queue,
/// and hands broadcast authority back to ctrl when a remote owner shuts down.
/// Returns `false` when the request was rejected; the caller then sends no
/// further reply.
fn prepare_shutdown(ctx: &CtrlContext<'_>) -> bool {
    log::info!(
        target: "daemon",
        "RequestShutdown received from ctrl client {}", ctx.client_id
    );
    if ctx.stay_alive {
        log::info!(target: "daemon", "RequestShutdown rejected: daemon is in stay-alive mode");
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

    if ctx.role != crate::daemon::DaemonRole::Local && queue_state.items.is_empty() {
        if let Some(existing) = crate::config::load_queue_state() {
            if !existing.items.is_empty() {
                queue_state = existing;
            }
        }
    }

    if let Err(e) = crate::config::save_queue_state(&queue_state) {
        log::error!(
            target: "daemon",
            "coordinated shutdown rejected: queue persistence failed: {e}"
        );
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
    cancel_pending_idle_queue_load_if_run_changed(ctx.owner, ctx.player);
    if ctx.owner.pending_idle_load.is_some() && !matches!(&cmd, CtrlCmd::RequestShutdown) {
        if let CtrlCmd::UnifiedQueueLoadIdle { request_id, .. } = cmd {
            queue_load::reject_queue_load(
                ctx.reply_tx,
                request_id,
                "another idle queue load is pending".to_string(),
            );
        } else {
            send_to(
                ctx.reply_tx,
                &CtrlEvent::CommandRejected("owner is finalizing an idle queue load".to_string()),
            );
        }
        return;
    }
    // `SharedQueueState.lineage` is the single source of truth (other
    // threads read it for cold ctrl-client snapshots); this is a
    // function-local snapshot so hot-path comparisons below don't
    // re-lock per read; `mint_queue_lineage` below writes the new value
    // straight to `shared_queue.lineage` for the next command's read.
    let queue_lineage = *ctx.shared_queue.lineage.lock().unwrap();
    match cmd.requires_owner() {
        crate::ctrl::OwnerGate::OwnerOnly(rejection)
            if ctx.role != crate::daemon::DaemonRole::Local =>
        {
            send_role_gate_rejection(rejection, ctx.rejection_context(queue_lineage));
            return;
        }
        crate::ctrl::OwnerGate::NonOwnerOnly(rejection)
            if ctx.role == crate::daemon::DaemonRole::Local =>
        {
            send_role_gate_rejection(rejection, ctx.rejection_context(queue_lineage));
            return;
        }
        _ => {}
    }
    if matches!(cmd, CtrlCmd::RequestShutdown) && !prepare_shutdown(&ctx) {
        return;
    }

    dispatch_ctrl_command(cmd, &mut ctx, queue_lineage);
}

fn dispatch_ctrl_command(
    cmd: CtrlCmd,
    ctx: &mut CtrlContext<'_>,
    queue_lineage: crate::ctrl::QueueLineage,
) {
    match cmd {
        CtrlCmd::Hello(_) => {
            log::warn!(target: "daemon", "unexpected ctrl protocol hello after negotiation");
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
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::JumpTo(_)) => {
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
        } => queue_setup::handle_queue_source_update(ctx, queue_lineage, new_source, cmd_lineage),
        // ── Unified queue commands ──────────────────────────────────────
        CtrlCmd::UnifiedQueueReplace {
            items,
            slots,
            start_idx,
            source: new_source,
        } => queue_setup::handle_queue_replace(
            ctx,
            queue_lineage,
            items,
            slots,
            start_idx,
            new_source,
        ),
        CtrlCmd::UnifiedQueueAppend { items } => {
            queue_setup::handle_queue_append(ctx, queue_lineage, items);
        }
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id } => {
            queue_edit::handle_queue_remove_slot(ctx, queue_lineage, slot_id);
        }
        CtrlCmd::UnifiedQueueRemoveSlots { slot_ids } => {
            queue_edit::handle_queue_remove_slots(ctx, slot_ids);
        }
        CtrlCmd::UnifiedQueueMoveSlot { slot_id, to_index } => {
            queue_edit::handle_queue_move_slot(ctx, queue_lineage, slot_id, to_index);
        }
        CtrlCmd::UnifiedQueuePlaySlot { slot_id } => {
            queue_edit::handle_queue_play_slot(ctx, queue_lineage, slot_id);
        }
        CtrlCmd::UnifiedQueueClear => queue_edit::handle_queue_clear(ctx),
    }
}

pub(crate) fn owner_admin_transport_allowed(
    role: crate::daemon::DaemonRole,
    kind: crate::config::ServiceKind,
    transport: Option<CtrlTransport>,
) -> bool {
    let role_allowed = role == crate::daemon::DaemonRole::Packaged
        || kind == crate::config::ServiceKind::Audiobookshelf;
    role_allowed && transport == Some(CtrlTransport::Local)
}
