use super::*;

/// `CtrlCmd::UnifiedQueueRemoveSlot`: remove the slot identified by
/// `slot_id`, handing off playback when it was the active slot.
pub(super) fn handle_queue_remove_slot(
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
pub(super) fn handle_queue_remove_slots(ctx: &mut CtrlContext<'_>, slot_ids: Vec<u64>) {
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
pub(super) fn handle_queue_move_slot(
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
pub(super) fn handle_queue_play_slot(
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
pub(super) fn handle_queue_clear(ctx: &mut CtrlContext<'_>) {
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
