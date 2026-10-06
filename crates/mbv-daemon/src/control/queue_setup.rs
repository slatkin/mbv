use super::{
    CtrlContext, DaemonEvent, DaemonPlayerOwner, EmbyItem, ExecSlot, PlaybackQueue, PlayerCommand,
    PlayerOwnerState, QueueItem, QueueSlotId, RejectContext, abs_queue_transport_rejection,
    admit_queue_items, admit_queue_slots, audio_only_rejection, broadcast_queue_state,
    daemon_admits, mint_queue_lineage, reject_command, reset_slot_jumps,
    submit_queue_slots_cold_start,
};
use crate::AudiobookshelfOwnerContext;
use mbv_emby::EmbyClient;
use mbv_player::Player;
use std::collections::HashSet;
use std::sync::Arc;

/// `CtrlCmd::UnifiedAdoptQueue`: a Client seeds a cold daemon's queue.
pub(super) fn handle_adopt_queue(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    items: Vec<QueueItem>,
    cursor: usize,
    new_source: mbv_queue::QueueSource,
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
        tracing::warn!(name: "daemon.queue_adoption.ignored", target: "daemon", slots = queue.len(), "queue adoption ignored: daemon already has a queue");
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
                op: &ctx.op,
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
        ctx.audiobookshelf.is_some(),
    );
    ctx.player.set_initial_queue(&items, next_cursor);
    reset_slot_jumps(transitions, queued_transition_origin);
    let mint = queue.revision_mint();
    *queue = PlaybackQueue::from_queue_items(items, Some(next_cursor), mint);
    *source = new_source;
    mint_queue_lineage(ctx.shared_queue);
    broadcast_queue_state(
        ctx.ctrl_clients,
        ctx.player,
        ctx.shared_queue,
        queue,
        source,
        transitions,
        None,
    );

    start_queue_enrichment(queue, ctx.client, ctx.audiobookshelf, ctx.merged_tx);
}

pub(super) fn handle_queue_refresh(ctx: &mut CtrlContext<'_>) {
    start_queue_enrichment(
        &ctx.owner.core.queue,
        ctx.client,
        ctx.audiobookshelf,
        ctx.merged_tx,
    );
}

/// The one progress refresh behind cold adoption, a manual queue refresh
/// (`CtrlCmd::UnifiedQueueRefresh`), and Owner restore. Fetches Emby items
/// when the queue holds Emby slots and Audiobookshelf progress when it holds
/// episode or book slots, off the event-loop thread.
pub(crate) fn start_queue_enrichment(
    queue: &PlaybackQueue,
    client: &Arc<std::sync::Mutex<EmbyClient>>,
    audiobookshelf: Option<&AudiobookshelfOwnerContext>,
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
    if !adopted_slots.is_empty() {
        let item_ids: Vec<String> = adopted_slots
            .iter()
            .map(|(_, item_id)| item_id.clone())
            .collect();
        super::playback::spawn_item_lookup(
            client,
            merged_tx,
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
                    tracing::warn!(name: "daemon.queue_enrichment.failed", target: "queue", error = %error, "adopted queue enrichment fetch failed");
                    None
                }
            },
        );
    }
    let Some(audiobookshelf) = audiobookshelf else {
        return;
    };
    let episode_keys: HashSet<(String, String)> = queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item
                .as_audiobookshelf()
                .map(|episode| (episode.library_item_id.clone(), episode.episode_id.clone()))
        })
        .collect();
    let book_ids: HashSet<String> = queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item
                .as_audiobookshelf_book()
                .map(|book| book.library_item_id.clone())
        })
        .collect();
    if episode_keys.is_empty() && book_ids.is_empty() {
        return;
    }
    crate::audiobookshelf::spawn_audiobookshelf_progress_refresh(
        audiobookshelf,
        episode_keys,
        book_ids,
        merged_tx,
    );
}

/// `CtrlCmd::UnifiedQueueSourceUpdate`: update only the source of the owner
/// queue if its lineage still matches.
pub(super) fn handle_queue_source_update(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    new_source: mbv_queue::QueueSource,
    cmd_lineage: mbv_queue::QueueLineage,
) {
    let except_op_client = ctx.except_op_client();
    let supports_operation = ctx
        .ctrl_clients
        .lock()
        .unwrap()
        .supports_owner_queue_load(ctx.client_id);
    if !supports_operation {
        reject_command(
            &ctx.rejection_context(lineage),
            "peer did not negotiate owner queue-load capability",
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
                op: &ctx.op,
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
            except_op_client,
        );
    }
}

fn prepare_replacement_slots(
    ctx: &CtrlContext<'_>,
    items: Vec<QueueItem>,
    slots: Vec<mbv_ctrl::UnifiedQueueSlot>,
    start_idx: Option<usize>,
    has_emby: bool,
) -> Result<(Vec<(QueueSlotId, QueueItem)>, usize), crate::DaemonLibError> {
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
        return Err(crate::DaemonLibError::queue_setup(reason));
    }
    let (slots, next_cursor) = admit_queue_slots(
        submitted_slots,
        start_idx,
        ctx.audio_only,
        has_emby,
        ctx.audiobookshelf.is_some(),
    );
    if slots.is_empty() {
        return Err(crate::DaemonLibError::queue_setup(
            "Playback owner rejected the queue replacement",
        ));
    }
    let admitted_items: Vec<QueueItem> = slots.iter().map(|(_, item)| item.clone()).collect();
    if let Some(reason) = audio_only_rejection(ctx.audio_only, &admitted_items) {
        return Err(crate::DaemonLibError::queue_setup(reason));
    }
    Ok((slots, next_cursor))
}

/// `CtrlCmd::UnifiedQueueReplace`: replace the entire queue with item-generic
/// slots and optionally begin playback from `start_idx`.
pub(super) fn handle_queue_replace(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    items: Vec<QueueItem>,
    slots: Vec<mbv_ctrl::UnifiedQueueSlot>,
    start_idx: Option<usize>,
    new_source: mbv_queue::QueueSource,
) {
    let has_emby = ctx.has_emby();
    let (slots, next_cursor) =
        match prepare_replacement_slots(ctx, items, slots, start_idx, has_emby) {
            Ok(admitted) => admitted,
            Err(reason) => {
                reject_command(&ctx.rejection_context(lineage), &reason.to_string());
                return;
            }
        };
    let except_op_client = ctx.except_op_client();
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
    let mint = queue.revision_mint();
    *queue = PlaybackQueue::from_slot_items(slots, active_slot, mint);
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
    // `send_command` alone only reaches an already-running mpv
    // thread; on a freshly started daemon no thread exists yet, so
    // route through `submit_queue_slots`, which cold-starts one when
    // needed (as `play_resolved_items` does).
    let queue_slots = queue.slot_pairs();
    submit_queue_slots_cold_start(ctx.player, queue_slots, next_cursor, ctx.client);
    ctx.owner.core.note_observed_active_slot(None);
    ctx.shared_queue.publish_observed(&ctx.owner.core);
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
        except_op_client,
    );
}

fn append_index(
    ctx: &CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    before: Option<u64>,
) -> Option<usize> {
    before.map_or_else(
        || Some(ctx.owner.core.queue.len()),
        |raw_slot_id| {
            let slot_id = QueueSlotId::from_raw(raw_slot_id);
            ctx.owner.core.queue.slot_index(slot_id).or_else(|| {
                reject_command(
                    &ctx.rejection_context(lineage),
                    "slot not found; append anchor is stale",
                );
                None
            })
        },
    )
}

fn forward_queue_append(player: &Player, items: Vec<ExecSlot>, before: Option<u64>, index: usize) {
    let inserted_slot_ids: Vec<_> = items.iter().map(|slot| slot.slot_id).collect();
    player.send_command(PlayerCommand::QueueAppend { items });
    if before.is_some() {
        for (offset, slot_id) in inserted_slot_ids.into_iter().enumerate() {
            player.send_command(PlayerCommand::QueueMove(slot_id, index + offset));
        }
    }
}

/// `CtrlCmd::UnifiedQueueAppend`: append item-generic values to the tail of
/// the queue.
pub(super) fn handle_queue_append(
    ctx: &mut CtrlContext<'_>,
    lineage: mbv_queue::QueueLineage,
    items: Vec<QueueItem>,
    before: Option<u64>,
) {
    let except_op_client = ctx.except_op_client();
    if items.is_empty() {
        return;
    }
    let Some(index) = append_index(ctx, lineage, before) else {
        return;
    };
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
                op: &ctx.op,
            },
            &reason,
        );
        return;
    }
    let mut items = items;
    items
        .retain(|item| daemon_admits(item, ctx.audio_only, has_emby, ctx.audiobookshelf.is_some()));
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
                op: &ctx.op,
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
                op: &ctx.op,
            },
            &reason,
        );
        return;
    }
    // Allocate the owner slot ids once, in the daemon's canonical
    // queue, and hand the same ids to the Playback run.
    let items_for_player: Vec<ExecSlot> = items
        .into_iter()
        .enumerate()
        .map(|(offset, item)| {
            let slot_id = queue.insert(index + offset, item.clone());
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
        except_op_client,
    );
    // Append to the player's queue rather than replacing the whole queue.
    // A following move keeps the run's order aligned with canonical inserts.
    forward_queue_append(ctx.player, items_for_player, before, index);
}
