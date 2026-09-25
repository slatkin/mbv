use super::*;

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

pub(super) fn reject_queue_load(
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

pub(in crate::daemon) fn cancel_pending_idle_queue_load(
    owner: &mut DaemonPlayerOwner,
    reason: &str,
) -> bool {
    let Some(pending) = owner.pending_idle_load.take() else {
        return false;
    };
    reject_queue_load(&pending.reply_tx, pending.request_id, reason.to_string());
    true
}

pub(in crate::daemon) fn cancel_pending_idle_queue_load_if_run_changed(
    owner: &mut DaemonPlayerOwner,
    player: &Player,
) -> bool {
    let current_run = player.status.lock().unwrap().sequence_generation;
    if owner
        .pending_idle_load
        .as_ref()
        .is_some_and(|pending| pending.stopped_run != current_run)
    {
        return cancel_pending_idle_queue_load(owner, "playback run changed during queue load");
    }
    false
}

pub(in crate::daemon) fn expire_pending_idle_queue_load(
    owner: &mut DaemonPlayerOwner,
    now: Instant,
) -> bool {
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

pub(in crate::daemon) fn complete_pending_idle_queue_load(
    run_identity: crate::ctrl::PlaybackGeneration,
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
/// `CtrlCmd::UnifiedQueueLoadIdle`: an owner-capable peer loads a whole queue
/// without starting playback. When playback is active, the load parks as
/// `PendingIdleQueueLoad` until the stop finalizes.
pub(super) fn handle_queue_load_idle(
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

    let stopped_run = ctx.player.status.lock().unwrap().sequence_generation;
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
