/// Builds a `QueueState` from the daemon's canonical queue and player status.
/// Used for coordinated shutdown persistence.
fn project_queue_state(
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
    player_status: &crate::player::PlayerStatus,
) -> crate::config::QueueState {
    use std::collections::HashMap;

    let slots = queue.slots();
    let active_idx = queue.active_index().unwrap_or(0);

    // Collect positions for non-audio items that have progress.
    let mut positions: HashMap<String, i64> = HashMap::new();
    for slot in slots {
        if slot.item.is_video() {
            let pos = slot.progress_state.local.position_ticks;
            if pos > 0 {
                positions.insert(slot.item.id().to_string(), pos);
            }
        }
    }

    // Incorporate the latest valid position for the active video item.
    if player_status.active && player_status.video_height > 0 && active_idx < slots.len() {
        if let Some(slot) = slots.get(active_idx) {
            if slot.item.is_video() {
                let position = if player_status.last_valid_pos > 0 {
                    player_status.last_valid_pos
                } else {
                    player_status.position_ticks
                };
                positions.insert(slot.item.id().to_string(), position);
            }
        }
    }

    let last_played_item_id = if player_status.active && active_idx < slots.len() {
        Some(slots[active_idx].item.id().to_string())
    } else {
        None
    };

    let queue_items: Vec<QueueItem> = slots.iter().map(|s| s.item.clone()).collect();

    crate::config::QueueState {
        source: source.clone(),
        items: queue_items,
        cursor: active_idx,
        last_played_content_id: if player_status.active && active_idx < slots.len() {
            Some(slots[active_idx].item.content_id())
        } else {
            None
        },
        last_played_item_id,
        last_played_completed: false,
        positions,
    }
}

/// Projects the canonical queue into `UnifiedQueueStateData` for one
/// connection. Audiobookshelf slots are included only when the peer
/// negotiated the matching capability (`abs-queue` for episodes,
/// `abs-book-queue` for books); when a slot is dropped, `active_slot` is
/// cleared too if the active slot was itself dropped, so a peer never
/// receives an `active_slot` pointing at a slot missing from `slots`.
fn unified_queue_state_for_peer(
    status: &crate::player::PlayerStatus,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
    supports_abs_queue: bool,
    supports_abs_book_queue: bool,
) -> CtrlEvent {
    let slots: Vec<crate::ctrl::UnifiedQueueSlot> = queue
        .slots()
        .iter()
        .filter(|s| match &s.item {
            QueueItem::Audiobookshelf(_) => supports_abs_queue,
            QueueItem::AudiobookshelfBook(_) => supports_abs_book_queue,
            QueueItem::Emby(_) | QueueItem::Feed(_) => true,
        })
        .map(|s| crate::ctrl::UnifiedQueueSlot {
            slot_id: crate::ctrl::slot_id_to_u64(s.slot_id),
            item: s.item.clone(),
        })
        .collect();
    let active_slot = queue
        .active_slot_id()
        .map(crate::ctrl::slot_id_to_u64)
        .filter(|active_id| slots.iter().any(|s| s.slot_id == *active_id));
    CtrlEvent::UnifiedQueueState(crate::ctrl::UnifiedQueueStateData {
        status: status.clone(),
        slots,
        active_slot,
        revision: queue.revision().raw(),
        source: source.clone(),
    })
}

/// Broadcasts a queue snapshot to clients and shared state.
fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
) {
    let status = player.status.lock().unwrap().clone();

    // ── Unified-queue peers, gate ABS episodes and books independently ──
    let unified_full_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, true, true,
    ));
    let unified_abs_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, true, false,
    ));
    let unified_book_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, false, true,
    ));
    let unified_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, false, false,
    ));

    if let (
        Some(unified_full_json),
        Some(unified_abs_json),
        Some(unified_book_json),
        Some(unified_json),
    ) = (
        unified_full_json,
        unified_abs_json,
        unified_book_json,
        unified_json,
    ) {
        ctrl_clients.lock().unwrap().broadcast_state_gated(
            unified_full_json,
            unified_abs_json,
            unified_book_json,
            unified_json,
        );
    }

    // Update the reconnect snapshot.
    *shared_queue.queue.lock().unwrap() = queue.clone();
    *shared_queue.source.lock().unwrap() = source.clone();
}

fn admit_queue_items(
    original: Vec<QueueItem>,
    requested_cursor: Option<usize>,
    audio_only: bool,
    has_emby: bool,
    has_audiobookshelf: bool,
) -> (Vec<QueueItem>, usize) {
    let requested = requested_cursor.unwrap_or(0);
    let rebased = original
        .iter()
        .take(requested)
        .filter(|item| daemon_admits(item, audio_only, has_emby, has_audiobookshelf))
        .count();
    let items: Vec<_> = original
        .into_iter()
        .filter(|item| daemon_admits(item, audio_only, has_emby, has_audiobookshelf))
        .collect();
    let cursor = if items.is_empty() {
        0
    } else {
        rebased.min(items.len() - 1)
    };
    (items, cursor)
}

fn daemon_admits(
    item: &QueueItem,
    audio_only: bool,
    has_emby: bool,
    has_audiobookshelf: bool,
) -> bool {
    if item.is_emby() && !has_emby {
        return false;
    }
    item.admissible_for_owner_with_audiobookshelf(
        audio_only,
        |kind| kind != crate::config::ServiceKind::Emby || has_emby,
        has_audiobookshelf,
    )
}

/// Returns a rejection reason when `items` contains an Audiobookshelf item
/// submitted by a peer that did not negotiate the matching queue transport
/// (`abs-queue` for episodes, `abs-book-queue` for books). Checked ahead of
/// queue mutation so an incapable peer's operation is refused outright rather
/// than silently dropping the unsupported item.
fn abs_queue_transport_rejection(
    items: &[QueueItem],
    supports_abs_queue: bool,
    supports_abs_book_queue: bool,
) -> Option<String> {
    if !supports_abs_queue && items.iter().any(QueueItem::is_audiobookshelf) {
        Some("peer did not negotiate Audiobookshelf queue transport".to_string())
    } else if !supports_abs_book_queue && items.iter().any(QueueItem::is_audiobookshelf_book) {
        Some("peer did not negotiate Audiobookshelf book queue transport".to_string())
    } else {
        None
    }
}

/// Rejects a queue command with `reason` and echoes the daemon's
/// authoritative state back to the requester.
fn reject_command(
    reply_tx: &CtrlSender,
    ctrl_clients: &ClientRegistry,
    client_id: CtrlClientId,
    player: &Player,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
    reason: String,
) {
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
            supports_abs_queue,
            supports_abs_book_queue,
        ),
    );
}
