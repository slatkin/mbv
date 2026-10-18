use super::super::core::DaemonEvent;
use super::{
    CtrlContext, DaemonOwnerContext, DaemonPlayerOwner, broadcast_queue_state, dispatch_slot_jump,
    handle_stop, mint_queue_lineage, reset_play_order_for_queue_replacement, send_to,
};
use crate::ctrl::CtrlClientId;
use mbv_ctrl::player::PlayerCommand;
use mbv_ctrl::{CtrlEvent, PlaybackGeneration, PlaybackRequestId};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_player::PlayerOwnerState;
use mbv_queue::{PlaybackQueue, QueueItem};
use std::sync::{Arc, Mutex, mpsc};

/// Fetches `item_ids` from Emby off the event-loop thread and sends the
/// result through `tx` as a `DaemonEvent`, built by `to_event`. Shared by
/// every ctrl handler that resolves item ids against Emby before rejoining
/// the daemon's single-threaded event loop.
pub(super) fn spawn_item_lookup<F>(
    client: &Arc<Mutex<EmbyClient>>,
    tx: &mpsc::Sender<DaemonEvent>,
    item_ids: Vec<String>,
    to_event: F,
) where
    F: FnOnce(Result<Vec<EmbyItem>, crate::DaemonLibError>) -> Option<DaemonEvent> + Send + 'static,
{
    let tx = tx.clone();
    let lookup_client = client.lock().unwrap().clone();
    std::thread::spawn(mbv_core::applog::carry_context(move || {
        if let Some(event) = to_event(
            lookup_client
                .get_items_by_ids(&item_ids)
                .map_err(crate::DaemonLibError::from),
        ) {
            let _ = tx.send(event);
        }
    }));
}

/// Plays a resolved-by-id playback intent. Replaces the legacy `PlayItems`
/// wire command, which carried both the wire shape and this internal
/// control-flow re-entry; the wire variant is gone (ADR 0020), so the
/// resolved-play path now lives here as a plain function. `client_id`,
/// `request_id`, and `generation` are the play intent's identity: on a Cast
/// output they name the in-flight transition the load registers (design D4),
/// so a resolve failure can reject the intent and a confirming receiver
/// status can settle it.
pub(crate) fn play_resolved_items(
    ctx: &mut DaemonOwnerContext<'_>,
    fetched: Vec<EmbyItem>,
    start_idx: usize,
    start_ticks: i64,
    new_source: mbv_queue::QueueSource,
    client_id: CtrlClientId,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
) {
    let start_idx = install_resolved_queue(ctx, &fetched, start_idx, new_source);
    if dispatch_resolved_remote(ctx, start_ticks, client_id, request_id, generation) {
        return;
    }
    play_resolved_locally(ctx, fetched, start_idx, start_ticks);
}

/// Installs the resolved items as the one canonical queue (design D4): the new
/// lineage, the reset play order, and the broadcast every attached Client
/// reads. Returns the clamped start index the play starts at.
fn install_resolved_queue(
    ctx: &mut DaemonOwnerContext<'_>,
    fetched: &[EmbyItem],
    start_idx: usize,
    new_source: mbv_queue::QueueSource,
) -> usize {
    let DaemonPlayerOwner {
        core: PlayerOwnerState { queue, source, .. },
        ..
    } = &mut *ctx.owner;
    let queue_items: Vec<QueueItem> = fetched
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    let start_idx = start_idx.min(queue_items.len().saturating_sub(1));
    let mint = queue.revision_mint();
    *queue = PlaybackQueue::from_queue_items(queue_items, Some(start_idx), mint);
    *source = new_source;
    // The resolved play installs a new lineage: the whole-queue replacement
    // that resets the order (design D4).
    mint_queue_lineage(ctx.shared_queue);
    reset_play_order_for_queue_replacement(&mut ctx.owner.core, ctx.shared_queue);
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
    start_idx
}

/// Loads the resolved play's active slot on a remote output (design D4, D7):
/// a receiver and a Session each hold one item, so only that slot's item is
/// resolved or admitted and loaded — never the whole queue (#944, #945).
/// Returns whether a remote output handled the play, which leaves the local
/// run untouched.
fn dispatch_resolved_remote(
    ctx: &mut DaemonOwnerContext<'_>,
    start_ticks: i64,
    client_id: CtrlClientId,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
) -> bool {
    match ctx.owner.output {
        crate::cast::PlaybackOutput::Cast(_) => {
            play_resolved_on_cast(ctx, start_ticks, client_id, request_id, generation);
            true
        }
        crate::cast::PlaybackOutput::EmbySession(_) => {
            play_resolved_on_session(ctx, start_ticks, client_id, request_id, generation);
            true
        }
        crate::cast::PlaybackOutput::Local => false,
    }
}

/// Resolves the resolved play's start slot and builds its transition under
/// the intent's identity (design D4, D7): the queue's active slot is the one
/// slot the play loads — a receiver and a Session each hold one item, so only
/// that slot's item is resolved or admitted, never the whole queue (#944,
/// #945). The in-flight transition under the intent's identity settles on a
/// confirming receiver status or observation and expires and reports on a
/// resolve failure or rejected item. `None` when the queue has no active
/// slot; the caller reports that under its output's own event name.
fn resolved_play_transition(
    ctx: &mut DaemonOwnerContext<'_>,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
) -> Option<mbv_player::transition::Transition> {
    let start_slot = ctx.owner.core.queue.active_slot_id()?;
    Some(mbv_player::transition::Transition::new(
        request_id, generation, start_slot,
    ))
}

/// Loads the active slot on the Cast receiver (design D4): the play registers
/// as the in-flight transition under the intent's identity, so a confirming
/// receiver status settles it and a resolve failure expires and reports it.
fn play_resolved_on_cast(
    ctx: &mut DaemonOwnerContext<'_>,
    start_ticks: i64,
    client_id: CtrlClientId,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
) {
    let Some(transition) = resolved_play_transition(ctx, request_id, generation) else {
        tracing::warn!(name: "daemon.cast_play.start_slot_missing", target: "daemon", "the resolved play has no start slot; nothing was loaded");
        return;
    };
    match ctx.owner.core.transitions.accept(transition) {
        mbv_player::transition::DispatchDecision::DispatchNow(t) => {
            crate::cast::dispatch_cast_load(ctx, t, start_ticks, Some(client_id));
        }
        mbv_player::transition::DispatchDecision::Queued { .. } => {
            tracing::warn!(name: "daemon.cast_play.start_slot_missing", target: "daemon", "the resolved play's transition was queued behind another; nothing was loaded");
        }
    }
}

/// Loads the active slot on the Emby Session (emby-session-owner-output design
/// D7): only the selected slot's item is admitted and loaded onto the Session,
/// never the whole queue (#945). The play registers as the in-flight
/// transition under the intent's identity, so an observation that shows the
/// loaded item settles it and a rejected item expires and reports it.
fn play_resolved_on_session(
    ctx: &mut DaemonOwnerContext<'_>,
    start_ticks: i64,
    client_id: CtrlClientId,
    request_id: PlaybackRequestId,
    generation: PlaybackGeneration,
) {
    let Some(transition) = resolved_play_transition(ctx, request_id, generation) else {
        tracing::warn!(name: "daemon.emby_session_play.start_slot_missing", target: "daemon", "the resolved play has no start slot; nothing was loaded");
        return;
    };
    crate::emby_session::dispatch_accepted(ctx, transition, start_ticks, Some(client_id));
}

/// Plays the resolved items on the local run (design D4): one item is a plain
/// play, several are a queue starting at the resolved index.
fn play_resolved_locally(
    ctx: &mut DaemonOwnerContext<'_>,
    fetched: Vec<EmbyItem>,
    start_idx: usize,
    start_ticks: i64,
) {
    if fetched.len() == 1 {
        let mut play_item = fetched[0].clone();
        if start_ticks > 0 {
            play_item.playback_position_ticks = start_ticks;
        }
        let c = Arc::new(ctx.client.lock().unwrap().clone());
        ctx.player.play(&play_item, c, 100);
    } else {
        let mut play_items = fetched;
        if start_ticks > 0 {
            play_items[start_idx].playback_position_ticks = start_ticks;
        }
        let c = Arc::new(ctx.client.lock().unwrap().clone());
        ctx.player.play_queue(play_items, start_idx, c, 100);
    }
}

/// `CtrlCmd::PlaybackIntent`: accept the intent (coalescing, pipe status),
/// then resolve its action. Correlated playback control — separate from
/// `PlayerCmd` so guarded actions cannot silently fall back to the old,
/// unacknowledged command path.
pub(super) fn handle_playback_intent(ctx: &mut CtrlContext<'_>, intent: mbv_ctrl::PlaybackIntent) {
    // Correlation span (design D5): every line this handler logs carries the
    // intent's client, request and generation.
    let _intent_span =
        super::intent_span(ctx.client_id, intent.request_id, intent.generation).entered();
    let pipe_output = ctx.client.lock().unwrap().config.audio_pipe_enabled;
    let intents = &mut ctx.owner.intents;
    let accepted = intents.accept(ctx.client_id, intent.clone(), pipe_output);
    let coalesced = accepted.iter().any(|event| {
        matches!(
            event.outcome,
            mbv_ctrl::PlaybackIntentOutcome::Coalesced { .. }
        )
    });
    for event in accepted {
        send_to(ctx.reply_tx, &CtrlEvent::PlaybackIntent(event));
    }
    if let Some(status) = intents.pipe_status() {
        tracing::info!(name: "daemon.pipe_latency.status", target: "pipe_latency", request = %status.request_id, generation = status.generation, phase = ?status.phase, elapsed_ms = intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default(), "pipe playback status");
        send_to(ctx.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
    }
    if coalesced {
        return;
    }
    let intent_request_id = intent.request_id;
    let intent_generation = intent.generation;
    match intent.action {
        mbv_ctrl::PlaybackIntentAction::Play {
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
        mbv_ctrl::PlaybackIntentAction::Stop => handle_stop(ctx),
        mbv_ctrl::PlaybackIntentAction::SetPaused { paused } => {
            if matches!(
                ctx.owner.output,
                crate::cast::PlaybackOutput::Cast(_) | crate::cast::PlaybackOutput::EmbySession(_)
            ) {
                // A remote output is the output (row 5.3 correction, spec
                // "play, pause, next, previous, and stop from either Client
                // act on that receiver"; emby-session-owner-output design D9):
                // the desired state translates to an explicit pause or play
                // on the receiver or the Session, and the guarded intent
                // settles now — the output's own pause change reports through
                // its poll (Cast design D8, Session design D5), not a run
                // event, so nothing else would settle it.
                crate::cast::set_receiver_paused(&mut ctx.owner.output, paused);
                if let Some(event) = ctx.owner.intents.applied_if_current(
                    ctx.client_id,
                    intent_request_id,
                    intent_generation,
                ) {
                    ctx.ctrl_clients
                        .lock()
                        .unwrap()
                        .send_to_client(ctx.client_id, &CtrlEvent::PlaybackIntent(event));
                }
            } else if ctx.player.status.lock().unwrap().paused != paused {
                ctx.player.send_command(PlayerCommand::TogglePause);
            }
        }
        action @ (mbv_ctrl::PlaybackIntentAction::Next
        | mbv_ctrl::PlaybackIntentAction::Previous) => {
            step_to_neighbor_slot(ctx, &action, intent_request_id, intent_generation);
        }
    }
}

/// `PlaybackIntentAction::Play`: resolve the intent's item ids against Emby
/// off the event-loop thread and rejoin with `DaemonEvent::PlaybackResolved`.
fn resolve_play_intent(
    ctx: &mut CtrlContext<'_>,
    request_id: mbv_ctrl::PlaybackRequestId,
    generation: mbv_ctrl::PlaybackGeneration,
    item_ids: Vec<String>,
    start_idx: usize,
    start_ticks: i64,
    intent_source: mbv_queue::QueueSource,
) {
    if !ctx.has_emby() {
        return;
    }
    let intents = &mut ctx.owner.intents;
    intents.mark_resolving(request_id);
    if let Some(status) = intents.pipe_status() {
        tracing::info!(name: "daemon.pipe_latency.status", target: "pipe_latency", request = %status.request_id, generation = status.generation, phase = ?status.phase, elapsed_ms = intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default(), "pipe playback status");
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
    action: &mbv_ctrl::PlaybackIntentAction,
    request_id: mbv_ctrl::PlaybackRequestId,
    generation: mbv_ctrl::PlaybackGeneration,
) {
    // Only a relative action reaches this step; any other action has no
    // neighbor to move to.
    let Some(direction) = Option::<mbv_ctrl::Direction>::from(action) else {
        return;
    };
    // Content the Owner did not load is the device's own playback (design
    // D9): next and previous go to the device as its own commands, and the
    // Owner loads nothing.
    if let crate::cast::PlaybackOutput::EmbySession(session) = &mut ctx.owner.output
        && crate::emby_session::command::route_step(session, direction)
    {
        return;
    }
    let queue = &ctx.owner.core.queue;
    let transitions = &ctx.owner.core.transitions;
    let observed_active_slot = ctx.owner.core.observed_active_slot();
    let target = ctx.owner.core.relative_step_target(direction);
    tracing::info!(
        name: "daemon.transition_step.requested",
        target: "transition",
        action = ?action,
        queued_latest = ?transitions.queued_latest().map(|t| t.target),
        in_flight = ?transitions.in_flight().map(|t| t.target),
        observed_active_slot = ?observed_active_slot,
        queue_active_slot = ?queue.active_slot_id(),
        target_slot = ?target,
        queue_len = queue.len(),
        "playback intent step requested"
    );
    if let mbv_player::owner_state::StepTarget::Jump(slot_id) = target {
        dispatch_slot_jump(
            &mut DaemonOwnerContext {
                player: ctx.player,
                client: ctx.client,
                owner: &mut *ctx.owner,
                shared_queue: ctx.shared_queue,
                ctrl_clients: ctx.ctrl_clients,
                merged_tx: ctx.merged_tx,
                audiobookshelf: ctx.audiobookshelf,
            },
            super::super::core::JumpOrigin::Ctrl(ctx.client_id),
            mbv_player::transition::Transition::with_cause(
                request_id,
                generation,
                slot_id,
                mbv_player::transition::TransitionCause::Step(direction),
            ),
            None,
        );
    } else if target == mbv_player::owner_state::StepTarget::Coalesced {
        send_to(
            ctx.reply_tx,
            &CtrlEvent::PlaybackIntent(mbv_ctrl::PlaybackIntentEvent {
                request_id,
                generation,
                outcome: mbv_ctrl::PlaybackIntentOutcome::Coalesced {
                    canonical_request_id: transitions
                        .queued_latest()
                        .or_else(|| transitions.in_flight())
                        .map_or(request_id, |t| t.request_id),
                },
            }),
        );
    }
}
