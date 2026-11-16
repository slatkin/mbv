//! The Owner's Named-queue commands (gui-named-queues design D3, D5, D6): the
//! edits of a resting queue, the create/rename/close commands, the per-connection
//! `NamedQueueShow` subscription, and the queue list every Client receives when
//! one of them changes the store.
//!
//! The current queue is not handled here: its slots, source, lineage, and run
//! stay on the Owner core, so an edit that names it (or names no queue) takes
//! the existing queue path (design D5).

use super::queue_setup::{install_replaced_queue, start_replaced_queue};
use super::{
    CtrlContext, DaemonEvent, DaemonOwnerContext, DaemonPlayerOwner, PlaybackQueue, PlayerCommand,
    QueueSlotId, RejectContext, SharedQueueState, broadcast_queue_state, dispatch_slot_jump,
    handle_stop, reject_command, send_to, unified_queue_state_for_peer,
};
use crate::core::{JumpOrigin, NamedQueueListSeed, NamedQueueTarget, RestingNamedQueue};
use crate::ctrl::{CtrlClientId, QueueTransport, serialize_ctrl_event};
use mbv_ctrl::{CtrlEvent, QueueOpOutcome, UnifiedQueueStateData};
use mbv_queue::{
    NamedQueueId, QueueItem, QueueMutationResult, QueueSource, RemoveSlotResult,
    StartSlotObservation,
};

mod show;
pub(super) use show::handle_show;
use show::{drop_current_shows, drop_shows, publish_snapshot, resting_snapshot};

/// The reason an edit names a queue the Owner no longer holds (design D5, spec
/// `named-queues` "Queue edits name their queue").
const UNKNOWN_QUEUE: &str = "named queue not found; edit skipped";

/// The reason a play names a slot its queue does not hold.
const MISSING_SLOT: &str = "slot not found; play skipped";

/// The reason a play names a queue with no slot to start (spec `named-queues`
/// "Play starts a queue at its start slot": a request to play an empty queue
/// changes nothing).
const EMPTY_QUEUE: &str = "the named queue is empty; play skipped";

/// The reason a rename carries no name once its surrounding whitespace is
/// trimmed (spec `named-queues` "An empty name is refused").
const EMPTY_NAME: &str = "queue name is empty; rename skipped";

/// The reason the Owner refuses to close its only Named queue (spec
/// `named-queues` "The last queue does not close").
const LAST_QUEUE: &str = "the last named queue does not close";

/// The queue one addressed edit reaches, or `None` after the Owner has
/// rejected the edit because it no longer holds the queue it names (design D5).
pub(super) fn edit_target(
    ctx: &CtrlContext<'_>,
    named: Option<NamedQueueId>,
) -> Option<NamedQueueTarget> {
    target(ctx, named, UNKNOWN_QUEUE)
}

/// The queue one addressed command reaches, or `None` after the Owner has
/// rejected it because it no longer holds that queue (design D5).
fn target(
    ctx: &CtrlContext<'_>,
    named: Option<NamedQueueId>,
    skipped: &str,
) -> Option<NamedQueueTarget> {
    let target = ctx.owner.named_queues.target(named);
    if target.is_none() {
        reject(ctx, skipped);
    }
    target
}

/// Rejects one Named-queue command (design D5): through the `op` result when
/// the command carries one, so the sending Client is never left waiting, and
/// otherwise as a plain command rejection. The reply names the queue the Owner
/// still holds, so a Client whose queue has gone resyncs on the current queue.
pub(super) fn reject(ctx: &CtrlContext<'_>, reason: &str) {
    reject_command(
        &RejectContext {
            reply_tx: ctx.reply_tx,
            ctrl_clients: ctx.ctrl_clients,
            client_id: ctx.client_id,
            player: ctx.player,
            queue: &ctx.owner.core.queue,
            source: &ctx.owner.core.source,
            lineage: *ctx.shared_queue.lineage.lock().unwrap(),
            start_slot: ctx.shared_queue.start_slot(),
            op: &ctx.op,
        },
        reason,
    );
}

/// Applies one edit to a resting queue (design D5): the same `PlaybackQueue`
/// methods an edit of the current queue uses, with no playhead and no output
/// projection. A rejected edit changes nothing; an applied one is answered with
/// the queue's snapshot, published in the queue list with its new length, and
/// marked for persistence.
pub(super) fn apply_edit(
    ctx: &mut CtrlContext<'_>,
    index: usize,
    edit: impl FnOnce(&mut RestingNamedQueue) -> Result<(), String>,
) {
    let result = ctx.owner.named_queues.resting.get_mut(index).map(edit);
    match result {
        Some(Ok(())) => finish_edit(ctx, index),
        Some(Err(reason)) => reject(ctx, &reason),
        None => reject(ctx, UNKNOWN_QUEUE),
    }
}

/// Answers one applied resting-queue edit (design D3, D5): the sending Client
/// gets the queue's new snapshot as its `op` answer, every other connection
/// that shows the queue gets the snapshot too, and the store is marked for
/// persistence.
fn finish_edit(ctx: &mut CtrlContext<'_>, index: usize) {
    let except_op_client = ctx.except_op_client();
    if let Some((id, data)) = resting_snapshot(ctx, index) {
        answer(ctx, id, data);
    }
    publish_snapshot(ctx, index, except_op_client);
    broadcast_list(ctx);
    persist(ctx);
}

/// Appends `items` to a resting queue at the tail, or before the slot
/// `before` names (design D5).
pub(super) fn append(
    ctx: &mut CtrlContext<'_>,
    index: usize,
    items: Vec<QueueItem>,
    before: Option<u64>,
) {
    apply_edit(ctx, index, |resting| {
        let at = match before {
            None => resting.queue.len(),
            Some(raw) => resting
                .queue
                .slot_index(QueueSlotId::from_raw(raw))
                .ok_or_else(|| "slot not found; append anchor is stale".to_string())?,
        };
        for (offset, item) in items.into_iter().enumerate() {
            resting.queue.insert(at.saturating_add(offset), item);
        }
        Ok(())
    });
}

/// Removes one slot of a resting queue (design D5), rejecting a stale slot
/// identity exactly as the current queue's removal does.
pub(super) fn remove_slot(ctx: &mut CtrlContext<'_>, index: usize, slot_id: u64) {
    let held = ctx
        .owner
        .named_queues
        .resting
        .get(index)
        .is_some_and(|resting| resting.queue.slot(QueueSlotId::from_raw(slot_id)).is_some());
    if held {
        remove_slots(ctx, index, vec![slot_id]);
    } else {
        reject(ctx, "slot not found; remove skipped");
    }
}

/// Removes every listed slot from a resting queue as one edit (design D5), so a
/// Client that selected a range never observes the queue shrinking one row per
/// round trip. An unknown slot identity is skipped, and a removal of the start
/// slot moves the start slot with the rule the current queue uses (design D4).
pub(super) fn remove_slots(ctx: &mut CtrlContext<'_>, index: usize, slot_ids: Vec<u64>) {
    apply_edit(ctx, index, |resting| {
        for raw in slot_ids {
            let slot_id = QueueSlotId::from_raw(raw);
            let start_slot_removed_at = start_slot_position(resting, slot_id);
            let removed = match resting.queue.remove_slot(slot_id) {
                RemoveSlotResult::Removed(_) => true,
                // A resting queue has no active slot, so this arm is not
                // reachable — but the slot is still removed rather than
                // silently skipped.
                RemoveSlotResult::RequiresActiveConfirmation(slot_id) => {
                    resting.queue.remove_active_slot_confirmed(slot_id);
                    true
                }
                RemoveSlotResult::NotFound => false,
            };
            if let Some(position) = start_slot_removed_at.filter(|_| removed) {
                resting.note_start_slot(StartSlotObservation::StartSlotRemoved { index: position });
            }
        }
        Ok(())
    });
}

/// The position the slot `removed` occupies when it is the queue's start slot.
fn start_slot_position(resting: &RestingNamedQueue, removed: QueueSlotId) -> Option<usize> {
    if resting.start_slot == Some(removed) {
        resting.queue.slot_index(removed)
    } else {
        None
    }
}

/// Moves a slot of a resting queue to `to_index` (design D5).
pub(super) fn move_slot(ctx: &mut CtrlContext<'_>, index: usize, slot_id: u64, to_index: usize) {
    apply_edit(ctx, index, |resting| {
        match resting
            .queue
            .move_slot(QueueSlotId::from_raw(slot_id), to_index)
        {
            QueueMutationResult::Applied(()) => Ok(()),
            QueueMutationResult::NotFound => Err("slot not found; move skipped".to_string()),
        }
    });
}

/// Reorders a resting queue at random (design D5). Nothing plays, so no slot is
/// pinned first: the permutation comes from the Owner's own rng, exactly as it
/// does for the current queue.
pub(super) fn shuffle(ctx: &mut CtrlContext<'_>, index: usize) {
    if ctx.owner.named_queues.resting.get(index).is_none() {
        reject(ctx, UNKNOWN_QUEUE);
        return;
    }
    let DaemonPlayerOwner {
        named_queues, rng, ..
    } = &mut *ctx.owner;
    if let Some(resting) = named_queues.resting.get_mut(index) {
        let order = resting.queue.shuffled_slot_order(rng, false);
        // An order that matches the current one edits nothing: no revision and
        // no reorder, the same rule the current queue's shuffle follows. The
        // permutation is otherwise always the queue's own slots, so it applies.
        let unchanged = resting
            .queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .eq(order.iter().copied());
        if !unchanged {
            let _ = resting.queue.reorder_slots(&order);
        }
    }
    finish_edit(ctx, index);
}

/// Removes every slot of a resting queue (design D5). Its start slot goes with
/// them, exactly as a clear of the current queue does (design D4).
pub(super) fn clear(ctx: &mut CtrlContext<'_>, index: usize) {
    apply_edit(ctx, index, |resting| {
        let start_slot_removed_at = resting
            .start_slot
            .and_then(|slot_id| resting.queue.slot_index(slot_id));
        resting.queue.clear();
        if let Some(position) = start_slot_removed_at {
            resting.note_start_slot(StartSlotObservation::StartSlotRemoved { index: position });
        }
        Ok(())
    });
}

/// Updates only the source of a resting queue (design D5).
pub(super) fn set_source(ctx: &mut CtrlContext<'_>, index: usize, source: QueueSource) {
    apply_edit(ctx, index, |resting| {
        resting.source = source;
        Ok(())
    });
}

/// `CtrlCmd::NamedQueueCreate`: one new, empty, resting queue named `Queue N`
/// (design D6), published in the queue list and answered with its own snapshot
/// so the sending Client learns its identity (design D3, spec `named-queues` "A
/// Client creates and renames queues").
pub(super) fn handle_create(ctx: &mut CtrlContext<'_>) {
    let mint = ctx.owner.core.queue.revision_mint();
    let index = ctx.owner.named_queues.push_new_queue(mint);
    broadcast_list(ctx);
    if let Some((id, data)) = resting_snapshot(ctx, index) {
        answer(ctx, id, data);
    }
    persist(ctx);
}

/// `CtrlCmd::NamedQueueRename`: rename any Named queue after trimming its
/// surrounding whitespace, refusing a name that is left empty, and naming no
/// uniqueness rule — two queues may share a name (spec `named-queues` "A Client
/// creates and renames queues").
pub(super) fn handle_rename(ctx: &mut CtrlContext<'_>, queue: NamedQueueId, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        reject(ctx, EMPTY_NAME);
        return;
    }
    let Some(target) = target(ctx, Some(queue), "named queue not found; rename skipped") else {
        return;
    };
    match target {
        NamedQueueTarget::Current => {
            ctx.owner.named_queues.current_name = name.to_string();
            broadcast_list(ctx);
            if let Some((id, data)) = current_snapshot(ctx) {
                answer(ctx, id, data);
            }
        }
        NamedQueueTarget::Resting(index) => {
            if let Some(resting) = ctx.owner.named_queues.resting.get_mut(index) {
                resting.name = name.to_string();
            }
            broadcast_list(ctx);
            if let Some((id, data)) = resting_snapshot(ctx, index) {
                answer(ctx, id, data);
            }
        }
    }
    persist(ctx);
}

/// `CtrlCmd::NamedQueueClose`: close one Named queue (spec `named-queues` "A
/// Client closes queues"). The Owner refuses to close its last queue. Closing a
/// resting queue deletes it and leaves playback alone; closing the current
/// queue stops playback, deletes it, and makes the first remaining queue in
/// queue order current without playing it.
pub(super) fn handle_close(ctx: &mut CtrlContext<'_>, queue: NamedQueueId) {
    let Some(target) = target(ctx, Some(queue), "named queue not found; close skipped") else {
        return;
    };
    if ctx.owner.named_queues.queue_count() <= 1 {
        reject(ctx, LAST_QUEUE);
        return;
    }
    match target {
        NamedQueueTarget::Resting(index) => {
            // Closing deletes the queue: its slots, source, and start slot go
            // with it, and playback does not change. Whoever shows it reads the
            // current queue from the list this publishes.
            drop(ctx.owner.named_queues.take_resting(index));
            drop_shows(ctx, queue);
        }
        NamedQueueTarget::Current => {
            install_as_current(ctx);
            // The installed queue is the current queue now, so every connection
            // that showed it reads it through the current-queue snapshots.
            drop_current_shows(ctx);
        }
    }
    broadcast_list(ctx);
    if let Some((id, data)) = current_snapshot(ctx) {
        answer(ctx, id, data);
    }
    persist(ctx);
}

/// `CtrlCmd::NamedQueuePlay`: make one Named queue current and start it (spec
/// `named-queues` "Play starts a queue at its start slot"). Naming the current
/// queue starts it in place; naming a resting queue swaps it in first (design
/// D2). A request to play an empty queue changes nothing.
pub(super) fn handle_play(ctx: &mut CtrlContext<'_>, queue: NamedQueueId, slot: Option<u64>) {
    let Some(target) = edit_target(ctx, Some(queue)) else {
        return;
    };
    match target {
        NamedQueueTarget::Current => play_current(ctx, slot),
        NamedQueueTarget::Resting(index) => play_named_resting(ctx, index, slot),
    }
}

/// Starts the current queue at `slot`, or at its start slot, or at its first
/// slot when it has neither (spec `named-queues` "Play starts a queue at its
/// start slot").
fn play_current(ctx: &mut CtrlContext<'_>, slot: Option<u64>) {
    let start = if let Some(raw) = slot {
        let requested = QueueSlotId::from_raw(raw);
        if ctx.owner.core.queue.slot(requested).is_none() {
            reject(ctx, MISSING_SLOT);
            return;
        }
        requested
    } else {
        let start = start_slot_or_first(&ctx.owner.core.queue, ctx.shared_queue.start_slot());
        let Some(start) = start else {
            reject(ctx, EMPTY_QUEUE);
            return;
        };
        start
    };
    start_current_queue(ctx, start);
}

/// Starts the resting queue at `index` at `slot`, or at its own start slot, or
/// at its first slot when it has neither (design D2).
fn play_named_resting(ctx: &mut CtrlContext<'_>, index: usize, slot: Option<u64>) {
    if let Some(raw) = slot {
        play_resting(ctx, index, QueueSlotId::from_raw(raw));
        return;
    }
    let start = ctx
        .owner
        .named_queues
        .resting
        .get(index)
        .and_then(|resting| start_slot_or_first(&resting.queue, resting.start_slot));
    match start {
        Some(start) => play_resting(ctx, index, start),
        None => reject(ctx, EMPTY_QUEUE),
    }
}

/// The slot a Play of `queue` starts when the Client names none (design D4):
/// the start slot while the queue still holds it, else the first slot. `None`
/// when the queue is empty.
fn start_slot_or_first(
    queue: &PlaybackQueue,
    start_slot: Option<QueueSlotId>,
) -> Option<QueueSlotId> {
    start_slot
        .filter(|slot_id| queue.slot(*slot_id).is_some())
        .or_else(|| queue.slots().first().map(|slot| slot.slot_id))
}

/// Starts the current queue at `start`: its run already holds this queue's
/// slots, so the slot is a jump, exactly as `UnifiedQueuePlaySlot` plays a slot
/// of the current queue.
fn start_current_queue(ctx: &mut CtrlContext<'_>, start: QueueSlotId) {
    let (request_id, generation) = ctx.owner.core.transitions.mint_local_id();
    let except_op_client = ctx.except_op_client();
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
        JumpOrigin::Ctrl(ctx.client_id),
        mbv_player::transition::Transition::new(request_id, generation, start),
        except_op_client,
    );
}

/// The swap of design D2 for a play (spec `named-queues` "Play starts a queue
/// at its start slot"): parks the current queue, installs the resting queue at
/// `index` with its slot identities kept, and starts `start`. A `start` the
/// queue does not hold is rejected and nothing changes.
pub(super) fn play_resting(ctx: &mut CtrlContext<'_>, index: usize, start: QueueSlotId) {
    let next_cursor = ctx
        .owner
        .named_queues
        .resting
        .get(index)
        .and_then(|resting| resting.queue.slot_index(start));
    let Some(next_cursor) = next_cursor else {
        reject(ctx, MISSING_SLOT);
        return;
    };
    let except_op_client = ctx.except_op_client();
    let Some(taken) = swap_in(ctx, index) else {
        reject(ctx, UNKNOWN_QUEUE);
        return;
    };
    let RestingNamedQueue { queue, source, .. } = taken;
    let slots: Vec<(QueueSlotId, QueueItem)> = queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.clone()))
        .collect();
    install_replaced_queue(ctx, slots, next_cursor, source);
    finish_swap(ctx);
    start_replaced_queue(ctx, except_op_client, next_cursor, None);
}

/// The swap of design D2 for a replace: parks the current queue and makes the
/// resting queue at `index` the current queue, so the replace installs its own
/// slots in place of that queue's (design D2).
pub(super) fn replace_with_resting(ctx: &mut CtrlContext<'_>, index: usize) {
    // The command's items replace the taken queue's slots, so only its identity
    // and name are used: they are the current queue's from here on.
    if swap_in(ctx, index).is_none() {
        reject(ctx, UNKNOWN_QUEUE);
    }
}

/// The swap of design D2, steps 1 to 3: stops the run, parks the current queue
/// into the store, and makes the resting queue at `index` the current queue. Its
/// content goes back to the caller, which installs the slots it wants as the
/// current queue's. `None` when the store holds no queue at `index`: nothing
/// changed then.
fn swap_in(ctx: &mut CtrlContext<'_>, index: usize) -> Option<RestingNamedQueue> {
    let taken = ctx.owner.named_queues.take_resting(index)?;
    park_current(ctx);
    ctx.owner.named_queues.current = taken.id;
    ctx.owner.named_queues.current_name.clone_from(&taken.name);
    Some(taken)
}

/// The swap of design D2, steps 1 and 2: stops the run, then moves the current
/// queue's slots, source, lineage, and start slot into the store as a resting
/// queue. The Owner core is left with an empty queue for the caller to replace.
fn park_current(ctx: &mut CtrlContext<'_>) {
    // Step 1: the run stops. A stop keeps the last observed active slot as the
    // start slot, so the parked queue resumes where it stopped (design D4).
    handle_stop(ctx);
    ctx.shared_queue
        .note_start_slot(&ctx.owner.core.queue, StartSlotObservation::Stopped);
    // Step 2: the queue's state moves into the store, and the store's own
    // current identity and name stay put until the caller installs the queue the
    // swap took out of it.
    let start_slot = ctx.shared_queue.start_slot();
    let lineage = *ctx.shared_queue.lineage.lock().unwrap();
    let mint = ctx.owner.core.queue.revision_mint();
    let queue = std::mem::replace(
        &mut ctx.owner.core.queue,
        PlaybackQueue::from_queue_items(Vec::new(), None, mint),
    );
    let source = std::mem::replace(&mut ctx.owner.core.source, QueueSource::Unknown);
    let parked = RestingNamedQueue {
        id: ctx.owner.named_queues.current,
        name: std::mem::take(&mut ctx.owner.named_queues.current_name),
        queue,
        source,
        lineage,
        start_slot,
    };
    ctx.owner.named_queues.resting.push(parked);
}

/// The swap of design D2 completes once the new current queue's content is on
/// the Owner core: the install starts the queue at the slot it made active, so
/// that slot is the queue's start slot while it plays (design D4). Every Client
/// learns the new current queue, and the store is marked for persistence.
pub(super) fn finish_swap(ctx: &mut CtrlContext<'_>) {
    let start_slot = ctx.owner.core.queue.active_slot_id();
    ctx.shared_queue.note_start_slot(
        &ctx.owner.core.queue,
        StartSlotObservation::Installed { start_slot },
    );
    // The queue the swap installed is the current queue now: the connections
    // that showed it read it through the current-queue snapshots from here on.
    drop_current_shows(ctx);
    broadcast_list(ctx);
    persist(ctx);
}

/// The index of the resting queue holding the lowest identity: queue order is
/// creation order, and identities are minted in it (design D1, D3).
fn first_resting(ctx: &CtrlContext<'_>) -> Option<usize> {
    ctx.owner
        .named_queues
        .resting
        .iter()
        .enumerate()
        .min_by_key(|(_, resting)| resting.id)
        .map(|(index, _)| index)
}

/// Makes the first remaining resting queue the current queue without playing
/// it (spec `named-queues` "A Client closes queues"): its slots keep their
/// identities, its source, lineage, and start slot move onto the Owner core,
/// the run's queue is emptied and the active output stopped, and every Client
/// receives the whole-queue snapshot — a Client that did not negotiate named
/// queues sees the new current queue that way (spec "Clients without named
/// queues see the current queue").
fn install_as_current(ctx: &mut CtrlContext<'_>) {
    let Some(next) =
        first_resting(ctx).and_then(|index| ctx.owner.named_queues.take_resting(index))
    else {
        return;
    };
    let RestingNamedQueue {
        id,
        name,
        queue,
        source,
        lineage,
        start_slot,
    } = next;
    ctx.owner.named_queues.current = id;
    ctx.owner.named_queues.current_name = name;
    let slots: Vec<(QueueSlotId, QueueItem)> = queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.clone()))
        .collect();
    let mint = ctx.owner.core.queue.revision_mint();
    ctx.owner.core.queue = PlaybackQueue::from_slot_items(slots, start_slot, mint);
    ctx.owner.core.source = source;
    // The installed queue keeps its own lineage: it is the same queue, with the
    // same history of replacements behind it.
    *ctx.shared_queue.lineage.lock().unwrap() = lineage;
    ctx.owner.core.note_observed_active_slot(None);
    ctx.shared_queue.publish_observed(&ctx.owner.core);
    // The installed queue's start slot becomes the current queue's start slot
    // through the one write site (design D4).
    ctx.shared_queue.note_start_slot(
        &ctx.owner.core.queue,
        StartSlotObservation::Installed { start_slot },
    );
    // Nothing plays: the run's copy of the closed queue is emptied and stopped,
    // as a clear does, so a later Play cold-starts the installed queue.
    ctx.player.advance_sequence_generation();
    ctx.player.send_command(PlayerCommand::SubmitQueue {
        items: Vec::new(),
        start_idx: 0,
        start_position_secs: None,
    });
    handle_stop(ctx);
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        &ctx.owner.core.transitions,
        None,
    );
}

/// Answers one Named-queue command (design D3, D5): with the command's `op`
/// result when it carries an `op`, and otherwise as the queue's own snapshot
/// event.
fn answer(ctx: &CtrlContext<'_>, queue: NamedQueueId, data: UnifiedQueueStateData) {
    match ctx.op.take() {
        Some(op) => send_to(
            ctx.reply_tx,
            &CtrlEvent::QueueOpResult {
                op,
                outcome: QueueOpOutcome::Applied(Box::new(data)),
            },
        ),
        None => send_to(ctx.reply_tx, &CtrlEvent::NamedQueueSnapshot { queue, data }),
    }
}

/// The snapshot of the current queue as its `op` answer carries it. Its `queue`
/// field stays empty: a snapshot names a queue only when it is not the current
/// one.
fn current_snapshot(ctx: &CtrlContext<'_>) -> Option<(NamedQueueId, UnifiedQueueStateData)> {
    let status = ctx.player.status.lock().unwrap().clone();
    let (in_flight, queued_latest) = ctx.owner.core.transitions.summaries();
    let queue_transport = {
        let clients = ctx.ctrl_clients.lock().unwrap();
        clients.queue_transport(ctx.client_id)
    };
    let event = unified_queue_state_for_peer(
        None,
        &status,
        &ctx.owner.core.queue,
        &ctx.owner.core.source,
        *ctx.shared_queue.lineage.lock().unwrap(),
        *ctx.shared_queue.observed_active_slot.lock().unwrap(),
        ctx.shared_queue.start_slot(),
        in_flight,
        queued_latest,
        queue_transport,
    );
    let CtrlEvent::UnifiedQueueState(data) = event else {
        return None;
    };
    Some((ctx.owner.named_queues.current, data))
}

/// Publishes the Named-queue list after a create, rename, close, swap, or edit
/// (design D3, spec `named-queues` "A Client creates and renames queues"): every
/// Named queue in creation order, which is identity order because identities
/// are minted in it, with the current queue named. A Client that did not
/// negotiate `named-queues` receives nothing, and the list also becomes the seed
/// a newly-attaching Client is sent (design D3, "sent on attach").
fn broadcast_list(ctx: &CtrlContext<'_>) {
    let seed = NamedQueueListSeed::new(&ctx.owner.named_queues, ctx.owner.core.queue.len());
    ctx.shared_queue.note_named_queue_list(seed.clone());
    let Some(json) = serialize_ctrl_event(&seed.event()) else {
        return;
    };
    ctx.ctrl_clients
        .lock()
        .unwrap()
        .broadcast_named_queue_event(&json);
}

/// Sends the whole Named-queue state to the daemon's persistence worker
/// (design D6). The event loop owns the worker, so the write never happens on
/// the ctrl path.
fn persist(ctx: &CtrlContext<'_>) {
    let state = named_queues_state(ctx.owner, ctx.shared_queue);
    if ctx
        .merged_tx
        .send(DaemonEvent::SaveNamedQueues(Box::new(state)))
        .is_err()
    {
        tracing::error!(name: "daemon.named_queues_persist.failed", target: "queue", "the daemon event loop is gone; the Named queues were not persisted");
    }
}

/// The Owner's whole Named-queue state as `named_queues_state.json` holds it
/// (design D6): the current queue from the Owner core, the resting queues from
/// the store, and the next identity to mint.
fn named_queues_state(
    owner: &DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
) -> mbv_config::NamedQueuesState {
    let mut queues = vec![mbv_config::NamedQueue {
        id: owner.named_queues.current,
        name: owner.named_queues.current_name.clone(),
        source: owner.core.source.clone(),
        lineage: *shared_queue.lineage.lock().unwrap(),
        start_slot: shared_queue.start_slot(),
        slots: slots_of(&owner.core.queue),
    }];
    for resting in &owner.named_queues.resting {
        queues.push(mbv_config::NamedQueue {
            id: resting.id,
            name: resting.name.clone(),
            source: resting.source.clone(),
            lineage: resting.lineage,
            start_slot: resting.start_slot,
            slots: slots_of(&resting.queue),
        });
    }
    queues.sort_by_key(|queue| queue.id);
    mbv_config::NamedQueuesState {
        current: owner.named_queues.current,
        next_queue_id: owner.named_queues.next_queue_id,
        queues,
    }
}

/// The persisted slots of one queue, with their owner-assigned identities.
fn slots_of(queue: &PlaybackQueue) -> Vec<mbv_config::NamedQueueSlot> {
    queue
        .slots()
        .iter()
        .map(|slot| mbv_config::NamedQueueSlot {
            slot_id: slot.slot_id,
            item: slot.item.clone(),
        })
        .collect()
}
