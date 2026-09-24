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

/// Fetches `item_ids` from Emby off the event-loop thread and sends the
/// result through `tx` as a `DaemonEvent`, built by `to_event`. Shared by
/// every ctrl handler that resolves item ids against Emby before rejoining
/// the daemon's single-threaded event loop.
fn spawn_item_lookup<F>(
    client: &Arc<Mutex<EmbyClient>>,
    tx: &mpsc::Sender<DaemonEvent>,
    item_ids: Vec<String>,
    to_event: F,
) where
    F: FnOnce(Result<Vec<EmbyItem>, String>) -> Option<DaemonEvent> + Send + 'static,
{
    let tx = tx.clone();
    let lookup_client = client.lock().unwrap().clone();
    std::thread::spawn(move || {
        if let Some(event) = to_event(lookup_client.get_items_by_ids(&item_ids)) {
            let _ = tx.send(event);
        }
    });
}

/// Plays a resolved-by-id playback intent. Replaces the legacy `PlayItems`
/// wire command, which carried both the wire shape and this internal
/// control-flow re-entry; the wire variant is gone (ADR 0020), so the
/// resolved-play path now lives here as a plain function.
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_resolved_items(
    fetched: Vec<EmbyItem>,
    start_idx: usize,
    start_ticks: i64,
    new_source: crate::config::QueueSource,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    queue: &mut PlaybackQueue,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    transitions: &crate::playback_transition::OwnerTransitionState,
) {
    let queue_items: Vec<QueueItem> = fetched
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    let start_idx = start_idx.min(queue_items.len().saturating_sub(1));
    *queue = PlaybackQueue::from_queue_items(queue_items, Some(start_idx));
    *source = new_source;
    mint_queue_lineage(shared_queue);
    broadcast_queue_state(
        ctrl_clients,
        player,
        shared_queue,
        queue,
        source,
        transitions,
    );
    if fetched.len() == 1 {
        let mut play_item = fetched[0].clone();
        if start_ticks > 0 {
            play_item.playback_position_ticks = start_ticks;
        }
        let c = Arc::new(client.lock().unwrap().clone());
        player.play(&play_item, c, 100);
    } else {
        let mut play_items = fetched;
        if start_ticks > 0 {
            play_items[start_idx].playback_position_ticks = start_ticks;
        }
        let c = Arc::new(client.lock().unwrap().clone());
        player.play_queue(play_items, start_idx, c, 100);
    }
}

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

fn install_idle_queue_load(
    request_id: crate::ctrl::QueueLoadRequestId,
    slots: Vec<(QueueSlotId, QueueItem)>,
    cursor: usize,
    source: crate::config::QueueSource,
    reply_tx: &CtrlSender,
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    let active_slot = slots.get(cursor).map(|(slot_id, _)| *slot_id);
    player.advance_sequence_generation();
    player.set_initial_queue(
        &slots
            .iter()
            .map(|(_, item)| item.clone())
            .collect::<Vec<_>>(),
        cursor,
    );
    reset_slot_jumps(
        &mut owner.core.transitions,
        &mut owner.queued_transition_origin,
    );
    owner.core.queue = PlaybackQueue::from_slot_items(
        slots,
        active_slot,
        crate::playback_queue::QueueRevision::default(),
    );
    owner.core.source = source;
    mint_queue_lineage(shared_queue);
    owner.core.note_observed_active_slot(None);
    *shared_queue.observed_active_slot.lock().unwrap() = None;
    broadcast_queue_state(
        ctrl_clients,
        player,
        shared_queue,
        &owner.core.queue,
        &owner.core.source,
        &owner.core.transitions,
    );
    // Persistence is owned by the caller's dirty-flag pass: every path that
    // reaches here (`UnifiedQueueLoadIdle` handled directly, and
    // `complete_pending_idle_queue_load` on a committed pending load) marks the
    // owner queue dirty, so the loop persists once through the injected store.
    send_to(
        reply_tx,
        &CtrlEvent::UnifiedQueueLoadResult {
            request_id,
            result: crate::ctrl::QueueLoadResult::Accepted,
        },
    );
}

fn reject_queue_load(
    reply_tx: &CtrlSender,
    request_id: crate::ctrl::QueueLoadRequestId,
    reason: String,
) {
    send_to(
        reply_tx,
        &CtrlEvent::UnifiedQueueLoadResult {
            request_id,
            result: crate::ctrl::QueueLoadResult::Rejected { reason },
        },
    );
}

const IDLE_QUEUE_LOAD_STOP_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn cancel_pending_idle_queue_load(owner: &mut DaemonPlayerOwner, reason: &str) -> bool {
    let Some(pending) = owner.pending_idle_load.take() else {
        return false;
    };
    reject_queue_load(&pending.reply_tx, pending.request_id, reason.to_string());
    true
}

pub(super) fn cancel_pending_idle_queue_load_if_run_changed(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
) -> bool {
    let current_run = (0, player.status.lock().unwrap().sequence_generation);
    if owner
        .pending_idle_load
        .as_ref()
        .is_some_and(|pending| pending.stopped_run != current_run)
    {
        return cancel_pending_idle_queue_load(owner, "playback run changed during queue load");
    }
    false
}

pub(super) fn expire_pending_idle_queue_load(owner: &mut DaemonPlayerOwner, now: Instant) -> bool {
    if owner.pending_idle_load.as_ref().is_some_and(|pending| {
        now.duration_since(pending.started_at) >= IDLE_QUEUE_LOAD_STOP_TIMEOUT
    }) {
        return cancel_pending_idle_queue_load(
            owner,
            "timed out waiting for playback stop finalization",
        );
    }
    false
}

pub(super) fn complete_pending_idle_queue_load(
    run_identity: (
        crate::ctrl::PlaybackRequestId,
        crate::ctrl::PlaybackGeneration,
    ),
    failure: Option<String>,
    owner: &mut DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) -> bool {
    if !owner
        .pending_idle_load
        .as_ref()
        .is_some_and(|pending| pending.stopped_run == run_identity)
    {
        return false;
    }
    let pending = owner.pending_idle_load.take().expect("checked above");
    if let Some(reason) = failure {
        reject_queue_load(&pending.reply_tx, pending.request_id, reason);
        return true;
    }
    install_idle_queue_load(
        pending.request_id,
        pending.slots,
        pending.cursor,
        pending.source,
        &pending.reply_tx,
        owner,
        player,
        shared_queue,
        ctrl_clients,
    );
    true
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
        crate::ctrl::OwnerGateRejection::QueueLoadIdle { request_id } => reject_queue_load(
            ctx.reply_tx,
            request_id,
            "idle queue loads are supported only by the Stay-alive owner".to_string(),
        ),
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
            reject_queue_load(
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

    match cmd {
        CtrlCmd::Hello(_) => {
            log::warn!(target: "daemon", "unexpected ctrl protocol hello after negotiation");
        }
        CtrlCmd::UnifiedAdoptQueue {
            items,
            cursor,
            source: new_source,
        } => handle_adopt_queue(&mut ctx, queue_lineage, items, cursor, new_source),
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
        CtrlCmd::Stop => handle_stop(&mut ctx),
        CtrlCmd::PlaybackIntent(intent) => handle_playback_intent(&mut ctx, intent),
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
        } => handle_queue_load_idle(&mut ctx, request_id, slots, cursor, new_source),
        CtrlCmd::UnifiedQueueSourceUpdate {
            source: new_source,
            lineage: cmd_lineage,
        } => handle_queue_source_update(&mut ctx, queue_lineage, new_source, cmd_lineage),
        // ── Unified queue commands ──────────────────────────────────────
        CtrlCmd::UnifiedQueueReplace {
            items,
            slots,
            start_idx,
            source: new_source,
        } => handle_queue_replace(&mut ctx, queue_lineage, items, slots, start_idx, new_source),
        CtrlCmd::UnifiedQueueAppend { items } => {
            handle_queue_append(&mut ctx, queue_lineage, items)
        }
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id } => {
            handle_queue_remove_slot(&mut ctx, queue_lineage, slot_id)
        }
        CtrlCmd::UnifiedQueueRemoveSlots { slot_ids } => {
            handle_queue_remove_slots(&mut ctx, slot_ids)
        }
        CtrlCmd::UnifiedQueueMoveSlot { slot_id, to_index } => {
            handle_queue_move_slot(&mut ctx, queue_lineage, slot_id, to_index)
        }
        CtrlCmd::UnifiedQueuePlaySlot { slot_id } => {
            handle_queue_play_slot(&mut ctx, queue_lineage, slot_id)
        }
        CtrlCmd::UnifiedQueueClear => handle_queue_clear(&mut ctx),
    }
}

/// `CtrlCmd::UnifiedAdoptQueue`: a Client seeds a cold daemon's queue.
fn handle_adopt_queue(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    items: Vec<QueueItem>,
    cursor: usize,
    new_source: crate::config::QueueSource,
) {
    let has_emby = ctx.has_emby();
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    // Adoption only applies to a Cold daemon — one with no queue yet.
    if !queue.is_empty() {
        log::warn!(
            target: "daemon",
            "ignoring UnifiedAdoptQueue: daemon already has a queue ({} slot(s))",
            queue.len()
        );
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "daemon already has a queue; adoption skipped".to_string(),
        );
        return;
    }
    let supports_abs_queue = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_queue(ctx.client_id);
    let supports_abs_book_queue = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_book_queue(ctx.client_id);
    if let Some(reason) =
        abs_queue_transport_rejection(&items, supports_abs_queue, supports_abs_book_queue)
    {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            reason,
        );
        return;
    }
    let (items, next_cursor) = admit_queue_items(
        items,
        Some(cursor),
        ctx.audio_only,
        has_emby,
        ctx.has_audiobookshelf,
    );
    ctx.player.set_initial_queue(&items, next_cursor);
    reset_slot_jumps(transitions, queued_transition_origin);
    *queue = PlaybackQueue::from_queue_items(items, Some(next_cursor));
    *source = new_source;
    mint_queue_lineage(ctx.shared_queue);
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
    );

    let adopted_slots: Vec<(QueueSlotId, String)> = queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item
                .as_emby()
                .map(|item| (slot.slot_id, item.id.clone()))
        })
        .collect();
    if !adopted_slots.is_empty() {
        let item_ids: Vec<String> = adopted_slots
            .iter()
            .map(|(_, item_id)| item_id.clone())
            .collect();
        spawn_item_lookup(
            ctx.client,
            ctx.merged_tx,
            item_ids,
            move |result| match result {
                Ok(items) => {
                    let items_by_id: std::collections::HashMap<String, EmbyItem> = items
                        .into_iter()
                        .map(|item| (item.id.clone(), item))
                        .collect();
                    let enriched = adopted_slots
                        .into_iter()
                        .filter_map(|(slot_id, item_id)| {
                            items_by_id
                                .get(&item_id)
                                .cloned()
                                .map(|item| (slot_id, item))
                        })
                        .collect();
                    Some(DaemonEvent::QueueEnriched(enriched))
                }
                Err(error) => {
                    log::warn!(target: "queue", "adopted queue enrichment fetch failed: {error}");
                    None
                }
            },
        );
    }
}

/// `CtrlCmd::PlaybackIntent`: accept the intent (coalescing, pipe status),
/// then resolve its action. Correlated playback control — separate from
/// `PlayerCmd` so guarded actions cannot silently fall back to the old,
/// unacknowledged command path.
fn handle_playback_intent(ctx: &mut CtrlContext<'_>, intent: crate::ctrl::PlaybackIntent) {
    let pipe_output = ctx.client.lock().unwrap().config.audio_pipe_enabled;
    let intents = &mut ctx.owner.intents;
    let accepted = intents.accept(ctx.client_id, intent.clone(), pipe_output);
    let coalesced = accepted.iter().any(|event| {
        matches!(
            event.outcome,
            crate::ctrl::PlaybackIntentOutcome::Coalesced { .. }
        )
    });
    for event in accepted {
        send_to(ctx.reply_tx, &CtrlEvent::PlaybackIntent(event));
    }
    if let Some(status) = intents.pipe_status() {
        log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
        send_to(ctx.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
    }
    if coalesced {
        return;
    }
    let intent_request_id = intent.request_id;
    let intent_generation = intent.generation;
    match intent.action {
        crate::ctrl::PlaybackIntentAction::Play {
            item_ids,
            start_idx,
            start_ticks,
            source: intent_source,
        } => resolve_play_intent(
            ctx,
            intent_request_id,
            intent_generation,
            item_ids,
            start_idx,
            start_ticks,
            intent_source,
        ),
        crate::ctrl::PlaybackIntentAction::Stop => handle_stop(ctx),
        crate::ctrl::PlaybackIntentAction::SetPaused { paused } => {
            if ctx.player.status.lock().unwrap().paused != paused {
                ctx.player.send_command(PlayerCommand::TogglePause);
            }
        }
        action @ (crate::ctrl::PlaybackIntentAction::Next
        | crate::ctrl::PlaybackIntentAction::Previous) => {
            step_to_neighbor_slot(ctx, action, intent_request_id, intent_generation);
        }
    }
}

/// `PlaybackIntentAction::Play`: resolve the intent's item ids against Emby
/// off the event-loop thread and rejoin with `DaemonEvent::PlaybackResolved`.
fn resolve_play_intent(
    ctx: &mut CtrlContext<'_>,
    request_id: crate::ctrl::PlaybackRequestId,
    generation: crate::ctrl::PlaybackGeneration,
    item_ids: Vec<String>,
    start_idx: usize,
    start_ticks: i64,
    intent_source: crate::config::QueueSource,
) {
    if !ctx.has_emby() {
        return;
    }
    let intents = &mut ctx.owner.intents;
    intents.mark_resolving(request_id);
    if let Some(status) = intents.pipe_status() {
        log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
        send_to(ctx.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
    }
    let client_id = ctx.client_id;
    spawn_item_lookup(ctx.client, ctx.merged_tx, item_ids, move |fetched| {
        Some(DaemonEvent::PlaybackResolved {
            start_idx,
            start_ticks,
            source: intent_source,
            client_id,
            request_id,
            generation,
            fetched,
        })
    });
}

/// A relative Next/Previous step advances from the *desired* active slot
/// — the newest queued or in-flight transition, else the slot the run
/// observes playing, else the queue's active marker. Stepping from the
/// published `current_idx` mirror instead recomputes the neighbor from a
/// coordinate that lags one transition behind while a jump settles, so
/// rapid Next presses kept landing on (or re-issuing) the wrong slot.
fn step_to_neighbor_slot(
    ctx: &mut CtrlContext<'_>,
    action: crate::ctrl::PlaybackIntentAction,
    request_id: crate::ctrl::PlaybackRequestId,
    generation: crate::ctrl::PlaybackGeneration,
) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    let base_idx = transitions
        .queued_latest()
        .or_else(|| transitions.in_flight())
        .map(|t| t.target)
        .or_else(|| {
            let observed = *ctx.shared_queue.observed_active_slot.lock().unwrap();
            observed
        })
        .or_else(|| queue.active_slot_id())
        .and_then(|slot| queue.slot_index(slot));
    let neighbor_idx = base_idx.and_then(|idx| match action {
        crate::ctrl::PlaybackIntentAction::Previous => idx.checked_sub(1),
        _ => Some(idx + 1).filter(|&next| next < queue.len()),
    });
    log::info!(
        target: "transition",
        "playback intent: action={:?} queued_latest={:?} in_flight={:?} observed_active_slot={:?} queue_active_slot={:?} base_idx={:?} neighbor_idx={:?} queue_len={}",
        action,
        transitions.queued_latest().map(|t| t.target),
        transitions.in_flight().map(|t| t.target),
        *ctx.shared_queue.observed_active_slot.lock().unwrap(),
        queue.active_slot_id(),
        base_idx,
        neighbor_idx,
        queue.len(),
    );
    if let Some(slot_id) = neighbor_idx.and_then(|idx| queue.slots().get(idx).map(|s| s.slot_id)) {
        dispatch_slot_jump(
            transitions,
            queued_transition_origin,
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            queue,
            source,
            ctx.client_id,
            crate::playback_transition::Transition::new(request_id, generation, slot_id),
        );
    }
}

/// `CtrlCmd::UnifiedQueueLoadIdle`: an owner-capable peer loads a whole queue
/// without starting playback. When playback is active, the load parks as
/// `PendingIdleQueueLoad` until the stop finalizes.
fn handle_queue_load_idle(
    ctx: &mut CtrlContext<'_>,
    request_id: crate::ctrl::QueueLoadRequestId,
    slots: Vec<crate::ctrl::UnifiedQueueSlot>,
    cursor: usize,
    new_source: crate::config::QueueSource,
) {
    let (supports_operation, supports_abs_queue, supports_abs_book_queue) = {
        let clients = ctx.ctrl_clients.lock().unwrap();
        (
            clients.supports_owner_queue_load(ctx.client_id),
            clients.supports_abs_queue(ctx.client_id),
            clients.supports_abs_book_queue(ctx.client_id),
        )
    };
    let reason = if !supports_operation {
        Some("peer did not negotiate owner queue-load capability".to_string())
    } else {
        abs_queue_transport_rejection(
            slots.iter().map(|slot| &slot.item),
            supports_abs_queue,
            supports_abs_book_queue,
        )
    };
    if let Some(reason) = reason {
        reject_queue_load(ctx.reply_tx, request_id, reason);
        return;
    }
    let submitted: Vec<_> = slots
        .into_iter()
        .map(|slot| (QueueSlotId::from_raw(slot.slot_id), slot.item))
        .collect();
    let was_nonempty = !submitted.is_empty();
    let (admitted, next_cursor) = admit_queue_slots(
        submitted,
        Some(cursor),
        ctx.audio_only,
        ctx.has_emby(),
        ctx.has_audiobookshelf,
    );
    let admission_error = if was_nonempty && admitted.is_empty() {
        Some("Playback owner rejected the queue load".to_string())
    } else {
        audio_only_rejection(ctx.audio_only, admitted.iter().map(|(_, item)| item))
    };
    if let Some(reason) = admission_error {
        reject_queue_load(ctx.reply_tx, request_id, reason);
        return;
    }

    let stopped_run = (0, ctx.player.status.lock().unwrap().sequence_generation);
    if ctx.player.status.lock().unwrap().active {
        ctx.owner.pending_idle_load = Some(PendingIdleQueueLoad {
            request_id,
            slots: admitted,
            cursor: next_cursor,
            source: new_source,
            reply_tx: ctx.reply_tx.clone(),
            stopped_run,
            started_at: Instant::now(),
        });
        ctx.player.stop();
        return;
    }

    install_idle_queue_load(
        request_id,
        admitted,
        next_cursor,
        new_source,
        ctx.reply_tx,
        ctx.owner,
        ctx.player,
        ctx.shared_queue,
        ctx.ctrl_clients,
    );
}

/// `CtrlCmd::UnifiedQueueSourceUpdate`: update only the source of the owner
/// queue if its lineage still matches.
fn handle_queue_source_update(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    new_source: crate::config::QueueSource,
    cmd_lineage: crate::ctrl::QueueLineage,
) {
    let supports_operation = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_owner_queue_load(ctx.client_id);
    if !supports_operation {
        send_to(
            ctx.reply_tx,
            &CtrlEvent::CommandRejected(
                "peer did not negotiate owner queue-load capability".to_string(),
            ),
        );
    } else if cmd_lineage != lineage {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &ctx.owner.core.queue,
                source: &ctx.owner.core.source,
                lineage,
            },
            "queue source update rejected: owner queue lineage changed".to_string(),
        );
    } else {
        let DaemonPlayerOwner {
            core:
                PlayerOwnerState {
                    queue,
                    source,
                    transitions,
                    ..
                },
            ..
        } = &mut *ctx.owner;
        *source = new_source;
        broadcast_queue_state(
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            queue,
            source,
            transitions,
        );
    }
}

/// `CtrlCmd::UnifiedQueueReplace`: replace the entire queue with item-generic
/// slots and optionally begin playback from `start_idx`.
fn handle_queue_replace(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    items: Vec<QueueItem>,
    slots: Vec<crate::ctrl::UnifiedQueueSlot>,
    start_idx: Option<usize>,
    new_source: crate::config::QueueSource,
) {
    let has_emby = ctx.has_emby();
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    let submitted_slots: Vec<(crate::playback_queue::QueueSlotId, QueueItem)> = if slots.is_empty()
    {
        items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                (
                    crate::playback_queue::QueueSlotId::from_raw((index + 1) as u64),
                    item,
                )
            })
            .collect()
    } else {
        slots
            .into_iter()
            .map(|slot| {
                (
                    crate::playback_queue::QueueSlotId::from_raw(slot.slot_id),
                    slot.item,
                )
            })
            .collect()
    };
    let submitted_items: Vec<QueueItem> = submitted_slots
        .iter()
        .map(|(_, item)| item.clone())
        .collect();
    let supports_abs_queue = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_queue(ctx.client_id);
    let supports_abs_book_queue = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_book_queue(ctx.client_id);
    if let Some(reason) = abs_queue_transport_rejection(
        &submitted_items,
        supports_abs_queue,
        supports_abs_book_queue,
    ) {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            reason,
        );
        return;
    }
    let (slots, next_cursor) = admit_queue_slots(
        submitted_slots,
        start_idx,
        ctx.audio_only,
        has_emby,
        ctx.has_audiobookshelf,
    );
    if slots.is_empty() {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "Playback owner rejected the queue replacement".to_string(),
        );
        return;
    }
    // Audio-only admission: reject if the daemon is in audio-only
    // mode and any item is non-audio.
    let admitted_items: Vec<QueueItem> = slots.iter().map(|(_, item)| item.clone()).collect();
    if let Some(reason) = audio_only_rejection(ctx.audio_only, &admitted_items) {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            reason,
        );
        return;
    }
    let active_slot = slots.get(next_cursor).map(|(slot_id, _)| *slot_id);
    *queue = PlaybackQueue::from_slot_items(
        slots,
        active_slot,
        crate::playback_queue::QueueRevision::default(),
    );
    *source = new_source;
    mint_queue_lineage(ctx.shared_queue);
    reset_slot_jumps(transitions, queued_transition_origin);
    // A new queue invalidates the previous playback observation (design
    // D2): clear it on both the shared snapshot and the owner core so
    // Next/Previous fall back to the new queue's active slot until the
    // first TrackChanged observation arrives. Momentary `None` is
    // correct — the observed slot follows playback, not the replace.
    // The shared clear happens with the reset so the publish is
    // already coherent; the owner-core clear happens before that
    // publish too, so both copies of the observation are gone by the
    // time a client can read the new snapshot.
    *ctx.shared_queue.observed_active_slot.lock().unwrap() = None;
    // `send_command` alone only reaches an already-running mpv
    // thread; on a freshly started daemon no thread exists yet, so
    // route through `submit_queue_slots`, which cold-starts one when
    // needed (as `play_resolved_items` does).
    let queue_slots = queue.slot_pairs();
    let all_audio = queue_slots.iter().all(|slot| slot.item.is_audio());
    let c = Arc::new(ctx.client.lock().unwrap().clone());
    let headless = ctx.player.headless_for(&c, all_audio);
    ctx.player
        .submit_queue_slots(queue_slots, next_cursor, Some(c), headless, 100);
    ctx.owner.core.note_observed_active_slot(None);
    // Publish after the submit, not before: the snapshot resolves its
    // active slot from the queue marker only while the player reports
    // active, and a cold-start submit is what flips that flag and seeds
    // the start item. Broadcasting first handed clients a new queue
    // with no active slot, so their now-playing projection fell back to
    // a stale `current_idx` and showed the queue's first row until the
    // next broadcast — the wrong-track flash this change removes.
    // Reborrowing `ctx.owner.core` here (rather than the handler's earlier
    // destructured fields) is what lets the publish follow the submit.
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
    );
}

/// `CtrlCmd::UnifiedQueueAppend`: append item-generic values to the tail of
/// the queue.
fn handle_queue_append(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    items: Vec<QueueItem>,
) {
    let has_emby = ctx.has_emby();
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        ..
    } = &mut *ctx.owner;
    if items.is_empty() {
        return;
    }
    let supports_abs_queue = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_queue(ctx.client_id);
    let supports_abs_book_queue = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_abs_book_queue(ctx.client_id);
    if let Some(reason) =
        abs_queue_transport_rejection(&items, supports_abs_queue, supports_abs_book_queue)
    {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            reason,
        );
        return;
    }
    let mut items = items;
    items.retain(|item| daemon_admits(item, ctx.audio_only, has_emby, ctx.has_audiobookshelf));
    if items.is_empty() {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "Playback owner rejected the queue append".to_string(),
        );
        return;
    }
    // Audio-only admission: reject if any appended item is non-audio.
    if let Some(reason) = audio_only_rejection(ctx.audio_only, &items) {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            reason,
        );
        return;
    }
    // Allocate the owner slot ids once, in the daemon's canonical
    // queue, and hand the same ids to the Playback run.
    let items_for_player: Vec<ExecSlot> = items
        .into_iter()
        .map(|item| {
            let slot_id = queue.append(item.clone());
            ExecSlot { slot_id, item }
        })
        .collect();
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
    );
    // Append to the player's queue rather than replacing the whole queue.
    ctx.player.send_command(PlayerCommand::QueueAppend {
        items: items_for_player,
    });
}

/// `CtrlCmd::UnifiedQueueRemoveSlot`: remove the slot identified by
/// `slot_id`, handing off playback when it was the active slot.
fn handle_queue_remove_slot(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    slot_id: u64,
) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    let sid = QueueSlotId::from_raw(slot_id);
    if queue.slot(sid).is_none() {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "slot not found; remove skipped".to_string(),
        );
    } else if queue.active_slot_id() == Some(sid) {
        queue.remove_active_slot_confirmed(sid);
        broadcast_queue_state(
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            queue,
            source,
            transitions,
        );
        if queue.is_empty() {
            // Clear the player's queue and stop.
            ctx.player.advance_sequence_generation();
            ctx.player.send_command(PlayerCommand::SubmitQueue {
                items: Vec::new(),
                start_idx: 0,
            });
            ctx.player.stop();
            reset_slot_jumps(transitions, queued_transition_origin);
        } else {
            // Removing the playing slot forces a track change that
            // carries no awaited transition identity, so anything in
            // flight can never settle: interrupt it deliberately.
            reset_slot_jumps(transitions, queued_transition_origin);
            ctx.player.send_command(PlayerCommand::QueueRemove(sid));
        }
    } else {
        queue.remove_slot(sid);
        broadcast_queue_state(
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            queue,
            source,
            transitions,
        );
        ctx.player.send_command(PlayerCommand::QueueRemove(sid));
    }
}

/// `CtrlCmd::UnifiedQueueRemoveSlots`: remove every listed slot as one queue
/// edit, so a client that selected a range never observes the queue shrinking
/// one row per round trip.
fn handle_queue_remove_slots(ctx: &mut CtrlContext<'_>, slot_ids: Vec<u64>) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    // One canonical revision and one published snapshot for the whole
    // edit: a client that selected a range must never observe the
    // queue shrinking one row per round trip.
    let mut removed = Vec::new();
    let mut removed_active = false;
    for slot_id in slot_ids {
        let sid = QueueSlotId::from_raw(slot_id);
        if queue.slot(sid).is_none() {
            continue;
        }
        if queue.active_slot_id() == Some(sid) {
            queue.remove_active_slot_confirmed(sid);
            removed_active = true;
        } else {
            queue.remove_slot(sid);
        }
        removed.push(sid);
    }
    if removed.is_empty() {
        return;
    }
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
    );
    if removed_active {
        // Removing the playing slot forces a track change that carries
        // no awaited transition identity, so anything in flight can
        // never settle: interrupt it deliberately (same as the
        // single-slot arm).
        reset_slot_jumps(transitions, queued_transition_origin);
    }
    if queue.is_empty() {
        ctx.player.advance_sequence_generation();
        ctx.player.send_command(PlayerCommand::SubmitQueue {
            items: Vec::new(),
            start_idx: 0,
        });
        ctx.player.stop();
        reset_slot_jumps(transitions, queued_transition_origin);
    } else {
        // The player run keeps its own queue copy; its per-slot edits
        // are not published, so one command per removed slot is fine
        // and lets it resolve the active-slot hand-off itself.
        for sid in removed {
            ctx.player.send_command(PlayerCommand::QueueRemove(sid));
        }
    }
}

/// `CtrlCmd::UnifiedQueueMoveSlot`: move the slot identified by `slot_id` to
/// `to_index`.
fn handle_queue_move_slot(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    slot_id: u64,
    to_index: usize,
) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        ..
    } = &mut *ctx.owner;
    let sid = QueueSlotId::from_raw(slot_id);
    if queue.slot(sid).is_none() {
        reject_command(
            RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "slot not found; move skipped".to_string(),
        );
    } else {
        queue.move_slot(sid, to_index);
        broadcast_queue_state(
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            queue,
            source,
            transitions,
        );
        ctx.player
            .send_command(PlayerCommand::QueueMove(sid, to_index));
    }
}

/// `CtrlCmd::UnifiedQueuePlaySlot`: begin playback of an existing slot
/// identified by `slot_id`.
fn handle_queue_play_slot(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    slot_id: u64,
) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    let sid = QueueSlotId::from_raw(slot_id);
    match queue.slot(sid) {
        Some(_) => {
            // No client request id on this command, so the owner mints
            // one to correlate the settling observation.
            let (request_id, generation) = transitions.mint_local_id();
            // `dispatch_slot_jump` publishes the snapshot after
            // accepting, so the pending slot reaches Clients.
            dispatch_slot_jump(
                transitions,
                queued_transition_origin,
                ctx.ctrl_clients,
                ctx.player,
                ctx.shared_queue,
                queue,
                source,
                ctx.client_id,
                crate::playback_transition::Transition::new(request_id, generation, sid),
            );
        }
        None => {
            reject_command(
                RejectContext {
                    reply_tx: ctx.reply_tx,
                    ctrl_clients: ctx.ctrl_clients,
                    client_id: ctx.client_id,
                    player: ctx.player,
                    queue: &*queue,
                    source: &*source,
                    lineage,
                },
                "slot not found; play skipped".to_string(),
            );
        }
    }
}

/// `CtrlCmd::UnifiedQueueClear`: clear all slots and stop playback.
fn handle_queue_clear(ctx: &mut CtrlContext<'_>) {
    let DaemonPlayerOwner {
        core:
            PlayerOwnerState {
                queue,
                source,
                transitions,
                ..
            },
        queued_transition_origin,
        ..
    } = &mut *ctx.owner;
    queue.clear();
    *source = crate::config::QueueSource::Unknown;
    mint_queue_lineage(ctx.shared_queue);
    ctx.player.advance_sequence_generation();
    ctx.player.send_command(PlayerCommand::SubmitQueue {
        items: Vec::new(),
        start_idx: 0,
    });
    ctx.player.stop();
    reset_slot_jumps(transitions, queued_transition_origin);
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
    );
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
