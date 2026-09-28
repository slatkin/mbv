use super::super::core::DaemonEvent;
use super::{
    CtrlContext, DaemonOwnerContext, DaemonPlayerOwner, broadcast_queue_state, dispatch_slot_jump,
    handle_stop, mint_queue_lineage, send_to,
};
use mbv_ctrl::CtrlEvent;
use mbv_ctrl::player::PlayerCommand;
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
    std::thread::spawn(move || {
        if let Some(event) = to_event(
            lookup_client
                .get_items_by_ids(&item_ids)
                .map_err(crate::DaemonLibError::from),
        ) {
            let _ = tx.send(event);
        }
    });
}

/// Plays a resolved-by-id playback intent. Replaces the legacy `PlayItems`
/// wire command, which carried both the wire shape and this internal
/// control-flow re-entry; the wire variant is gone (ADR 0020), so the
/// resolved-play path now lives here as a plain function.
pub(crate) fn play_resolved_items(
    ctx: &mut DaemonOwnerContext<'_>,
    fetched: Vec<EmbyItem>,
    start_idx: usize,
    start_ticks: i64,
    new_source: mbv_queue::QueueSource,
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
    let queue_items: Vec<QueueItem> = fetched
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    let start_idx = start_idx.min(queue_items.len().saturating_sub(1));
    let mint = queue.revision_mint();
    *queue = PlaybackQueue::from_queue_items(queue_items, Some(start_idx), mint);
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
        log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
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
            if ctx.player.status.lock().unwrap().paused != paused {
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
    action: &mbv_ctrl::PlaybackIntentAction,
    request_id: mbv_ctrl::PlaybackRequestId,
    generation: mbv_ctrl::PlaybackGeneration,
) {
    let direction = Option::<mbv_ctrl::Direction>::from(action).expect("relative action");
    let queue = &ctx.owner.core.queue;
    let transitions = &ctx.owner.core.transitions;
    let observed_active_slot = ctx.owner.core.observed_active_slot();
    let target = ctx.owner.core.relative_step_target(direction);
    log::info!(
        target: "transition",
        "playback intent: action={:?} queued_latest={:?} in_flight={:?} observed_active_slot={:?} queue_active_slot={:?} target={:?} queue_len={}",
        action,
        transitions.queued_latest().map(|t| t.target),
        transitions.in_flight().map(|t| t.target),
        observed_active_slot,
        queue.active_slot_id(),
        target,
        queue.len(),
    );
    if let mbv_player::owner_state::StepTarget::Jump(slot_id) = target {
        dispatch_slot_jump(
            &mut DaemonOwnerContext {
                player: ctx.player,
                client: ctx.client,
                owner: &mut *ctx.owner,
                shared_queue: ctx.shared_queue,
                ctrl_clients: ctx.ctrl_clients,
            },
            super::super::core::JumpOrigin::Ctrl(ctx.client_id),
            mbv_player::transition::Transition::with_cause(
                request_id,
                generation,
                slot_id,
                mbv_player::transition::TransitionCause::Step(direction),
            ),
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
