use super::{
    abs_queue_transport_rejection, admit_queue_items, admit_queue_slots, audio_only_rejection,
    broadcast_queue_state, daemon_admits, mint_queue_lineage, reject_command, reset_slot_jumps,
    send_to, CtrlContext, DaemonEvent, DaemonPlayerOwner, EmbyItem, ExecSlot, PlaybackQueue,
    PlayerCommand, PlayerOwnerState, QueueItem, QueueSlotId, RejectContext,
};
use crate::api::EmbyClient;
use crate::ctrl::CtrlEvent;
use std::sync::Arc;

/// `CtrlCmd::UnifiedAdoptQueue`: a Client seeds a cold daemon's queue.
pub(super) fn handle_adopt_queue(
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
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "daemon already has a queue; adoption skipped",
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
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            &reason,
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

    enrich_adopted_emby_slots(queue, ctx.client, ctx.merged_tx);
}

fn enrich_adopted_emby_slots(
    queue: &PlaybackQueue,
    client: &Arc<std::sync::Mutex<EmbyClient>>,
    merged_tx: &std::sync::mpsc::Sender<DaemonEvent>,
) {
    let adopted_slots: Vec<(QueueSlotId, String)> = queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item
                .as_emby()
                .map(|item| (slot.slot_id, item.id.clone()))
        })
        .collect();
    if adopted_slots.is_empty() {
        return;
    }
    let item_ids: Vec<String> = adopted_slots
        .iter()
        .map(|(_, item_id)| item_id.clone())
        .collect();
    super::playback::spawn_item_lookup(client, merged_tx, item_ids, move |result| match result {
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
    });
}

/// `CtrlCmd::UnifiedQueueSourceUpdate`: update only the source of the owner
/// queue if its lineage still matches.
pub(super) fn handle_queue_source_update(
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
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &ctx.owner.core.queue,
                source: &ctx.owner.core.source,
                lineage,
            },
            "queue source update rejected: owner queue lineage changed",
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

fn prepare_replacement_slots(
    ctx: &CtrlContext<'_>,
    items: Vec<QueueItem>,
    slots: Vec<crate::ctrl::UnifiedQueueSlot>,
    start_idx: Option<usize>,
    has_emby: bool,
) -> Result<(Vec<(QueueSlotId, QueueItem)>, usize), String> {
    let submitted_slots: Vec<(QueueSlotId, QueueItem)> = if slots.is_empty() {
        items
            .into_iter()
            .enumerate()
            .map(|(index, item)| (QueueSlotId::from_raw((index + 1) as u64), item))
            .collect()
    } else {
        slots
            .into_iter()
            .map(|slot| (QueueSlotId::from_raw(slot.slot_id), slot.item))
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
        return Err(reason);
    }
    let (slots, next_cursor) = admit_queue_slots(
        submitted_slots,
        start_idx,
        ctx.audio_only,
        has_emby,
        ctx.has_audiobookshelf,
    );
    if slots.is_empty() {
        return Err("Playback owner rejected the queue replacement".to_string());
    }
    let admitted_items: Vec<QueueItem> = slots.iter().map(|(_, item)| item.clone()).collect();
    if let Some(reason) = audio_only_rejection(ctx.audio_only, &admitted_items) {
        return Err(reason);
    }
    Ok((slots, next_cursor))
}

/// `CtrlCmd::UnifiedQueueReplace`: replace the entire queue with item-generic
/// slots and optionally begin playback from `start_idx`.
pub(super) fn handle_queue_replace(
    ctx: &mut CtrlContext<'_>,
    lineage: crate::ctrl::QueueLineage,
    items: Vec<QueueItem>,
    slots: Vec<crate::ctrl::UnifiedQueueSlot>,
    start_idx: Option<usize>,
    new_source: crate::config::QueueSource,
) {
    let has_emby = ctx.has_emby();
    let (slots, next_cursor) =
        match prepare_replacement_slots(ctx, items, slots, start_idx, has_emby) {
            Ok(admitted) => admitted,
            Err(reason) => {
                reject_command(&ctx.rejection_context(lineage), &reason);
                return;
            }
        };
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
pub(super) fn handle_queue_append(
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
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            &reason,
        );
        return;
    }
    let mut items = items;
    items.retain(|item| daemon_admits(item, ctx.audio_only, has_emby, ctx.has_audiobookshelf));
    if items.is_empty() {
        reject_command(
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            "Playback owner rejected the queue append",
        );
        return;
    }
    // Audio-only admission: reject if any appended item is non-audio.
    if let Some(reason) = audio_only_rejection(ctx.audio_only, &items) {
        reject_command(
            &RejectContext {
                reply_tx: ctx.reply_tx,
                ctrl_clients: ctx.ctrl_clients,
                client_id: ctx.client_id,
                player: ctx.player,
                queue: &*queue,
                source: &*source,
                lineage,
            },
            &reason,
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
