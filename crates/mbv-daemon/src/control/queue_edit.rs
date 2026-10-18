use super::{
    CtrlContext, DaemonOwnerContext, DaemonPlayerOwner, PlayerCommand, PlayerOwnerState,
    QueueSlotId, RejectContext, broadcast_queue_state, dispatch_slot_jump,
    forward_play_order_advance, mint_queue_lineage, reject_command,
    reset_play_order_for_queue_replacement, reset_slot_jumps,
};

/// `CtrlCmd::UnifiedQueueRemoveSlot`: remove the slot identified by
/// `slot_id`, handing off playback when it was the active slot.
pub(super) fn handle_queue_remove_slot(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    slot_id: u64,
) {
    let except_op_client = ctx.except_op_client();
    let order_before = ctx.owner.core.play_order();
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
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
                op: &ctx.op,
                play_order: *ctx.shared_queue.play_order.lock().unwrap(),
            },
            "slot not found; remove skipped",
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
            except_op_client,
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
        // The edit can drop the Shuffle pick; re-pick and forward the new
        // advance decision (queue-shuffle-play-order design D2).
        ctx.owner.core.note_queue_slot_removed(sid);
        if ctx.owner.core.play_order() != order_before {
            forward_play_order_advance(&ctx.owner.core, ctx.player);
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
            except_op_client,
        );
        // The edit can drop the Shuffle pick; re-pick and forward the new
        // advance decision (queue-shuffle-play-order design D2).
        ctx.owner.core.note_queue_slot_removed(sid);
        if ctx.owner.core.play_order() != order_before {
            forward_play_order_advance(&ctx.owner.core, ctx.player);
        }
        ctx.player.send_command(PlayerCommand::QueueRemove(sid));
    }
}

/// `CtrlCmd::UnifiedQueueRemoveSlots`: remove every listed slot as one queue
/// edit, so a client that selected a range never observes the queue shrinking
/// one row per round trip.
pub(super) fn handle_queue_remove_slots(ctx: &mut CtrlContext<'_>, slot_ids: Vec<u64>) {
    let except_op_client = ctx.except_op_client();
    let order_before = ctx.owner.core.play_order();
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
        except_op_client,
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
        for sid in &removed {
            ctx.owner.core.note_queue_slot_removed(*sid);
        }
    } else {
        // The player run keeps its own queue copy; its per-slot edits
        // are not published, so one command per removed slot is fine
        // and lets it resolve the active-slot hand-off itself.
        for sid in &removed {
            // Each edit can drop the Shuffle pick; re-pick before the
            // advance decision is forwarded once below (design D2).
            ctx.owner.core.note_queue_slot_removed(*sid);
            ctx.player.send_command(PlayerCommand::QueueRemove(*sid));
        }
    }
    if ctx.owner.core.play_order() != order_before {
        forward_play_order_advance(&ctx.owner.core, ctx.player);
    }
}

/// `CtrlCmd::UnifiedQueueShuffle`: reorder the whole queue once at random,
/// with the active slot first, and publish the new order as one snapshot.
pub(super) fn handle_queue_shuffle(ctx: &mut CtrlContext<'_>, rng: &mut impl rand::Rng) {
    let except_op_client = ctx.except_op_client();
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
    let order = queue.shuffled_slot_order(rng);
    if matches!(
        queue.reorder_slots(&order),
        mbv_queue::QueueMutationResult::Applied(())
    ) {
        broadcast_queue_state(
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            queue,
            source,
            transitions,
            except_op_client,
        );
        // One command carries the whole new order (queue-content-shuffle
        // task 3.3): the run drains its command channel per tick, so a
        // move list could leave an mpv EndFile processed against a
        // partially applied order. The run applies the order inside one
        // handler, active slot first, so its `current_idx` ends at 0.
        ctx.player.send_command(PlayerCommand::QueueReorder(order));
    }
}

/// `CtrlCmd::UnifiedQueueMoveSlot`: move the slot identified by `slot_id` to
/// `to_index`.
pub(super) fn handle_queue_move_slot(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    slot_id: u64,
    to_index: usize,
) {
    let except_op_client = ctx.except_op_client();
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
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
                op: &ctx.op,
                play_order: *ctx.shared_queue.play_order.lock().unwrap(),
            },
            "slot not found; move skipped",
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
            except_op_client,
        );
        ctx.player
            .send_command(PlayerCommand::QueueMove(sid, to_index));
    }
}

/// `CtrlCmd::UnifiedQueuePlaySlot`: begin playback of an existing slot
/// identified by `slot_id`.
pub(super) fn handle_queue_play_slot(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    slot_id: u64,
) {
    let except_op_client = ctx.except_op_client();
    let sid = QueueSlotId::from_raw(slot_id);
    // No client request id on this command, so the owner mints
    // one to correlate the settling observation.
    if ctx.owner.core.queue.slot(sid).is_none() {
        reject_command(
            &ctx.rejection_context(lineage),
            "slot not found; play skipped",
        );
        return;
    }
    let (request_id, generation) = ctx.owner.core.transitions.mint_local_id();
    // `dispatch_slot_jump` publishes the snapshot after
    // accepting, so the pending slot reaches Clients.
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
        mbv_player::transition::Transition::new(request_id, generation, sid),
        except_op_client,
    );
}

/// Applies provider-qualified progress to every matching inactive queue slot.
pub(super) fn handle_queue_apply_progress(
    ctx: &mut CtrlContext<'_>,
    updates: Vec<mbv_ctrl::ProgressUpdate>,
) {
    let active_slot = ctx.owner.core.queue.active_slot_id();
    let mut applied = false;
    for update in updates {
        let matching_slots: Vec<_> = ctx
            .owner
            .core
            .queue
            .slots()
            .iter()
            .filter(|slot| {
                Some(slot.slot_id) != active_slot && slot.item.content_id() == update.content_id
            })
            .map(|slot| slot.slot_id)
            .collect();
        for slot_id in matching_slots {
            let _ = ctx.owner.core.queue.apply_progress(
                slot_id,
                update.position_ticks,
                update.finished,
            );
            applied = true;
        }
    }
    if applied {
        broadcast_queue_state(
            ctx.ctrl_clients,
            ctx.player,
            ctx.shared_queue,
            &ctx.owner.core.queue,
            &ctx.owner.core.source,
            &ctx.owner.core.transitions,
            Some(ctx.client_id),
        );
    }
}

/// `CtrlCmd::UnifiedQueueClear`: clear all slots and stop playback.
pub(super) fn handle_queue_clear(ctx: &mut CtrlContext<'_>) {
    let except_op_client = ctx.except_op_client();
    // Clearing installs a new lineage: the whole-queue replacement that
    // resets the order (design D4).
    ctx.owner.core.queue.clear();
    ctx.owner.core.source = mbv_queue::QueueSource::Unknown;
    mint_queue_lineage(ctx.shared_queue);
    reset_play_order_for_queue_replacement(&mut ctx.owner.core, ctx.shared_queue, ctx.player);
    ctx.player.advance_sequence_generation();
    ctx.player.send_command(PlayerCommand::SubmitQueue {
        items: Vec::new(),
        start_idx: 0,
    });
    ctx.player.stop();
    reset_slot_jumps(
        &mut ctx.owner.core.transitions,
        &mut ctx.owner.queued_transition_origin,
    );
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        except_op_client,
    );
}
