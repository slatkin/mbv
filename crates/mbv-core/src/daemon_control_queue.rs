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

fn unified_queue_state(
    status: &crate::player::PlayerStatus,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
) -> CtrlEvent {
    CtrlEvent::UnifiedQueueState(crate::ctrl::UnifiedQueueStateData {
        status: status.clone(),
        slots: queue
            .slots()
            .iter()
            .map(|s| crate::ctrl::UnifiedQueueSlot {
                slot_id: crate::ctrl::slot_id_to_u64(s.slot_id),
                item: s.item.clone(),
            })
            .collect(),
        active_slot: queue.active_slot_id().map(crate::ctrl::slot_id_to_u64),
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

    // ── Unified-queue capable peers ────────────────────────────────────
    let unified_json = serialize_ctrl_event(&unified_queue_state(&status, queue, source));

    // ── Legacy peers: derive split items from canonical queue ──
    let (emby_items, feed_items) = split_queue_for_legacy(queue);
    let capable_json = serialize_ctrl_event(&CtrlEvent::State(CtrlState {
        status: status.clone(),
        items: emby_items.clone(),
        cursor: queue.legacy_cursor(true),
        source: source.clone(),
        feed_items: feed_items.clone(),
    }));
    let legacy_json = serialize_ctrl_event(&CtrlEvent::State(CtrlState {
        status,
        items: emby_items,
        cursor: queue.legacy_cursor(false),
        source: source.clone(),
        feed_items: Vec::new(),
    }));

    if let (Some(unified_json), Some(capable_json), Some(legacy_json)) =
        (unified_json, capable_json, legacy_json)
    {
        ctrl_clients
            .lock()
            .unwrap()
            .broadcast_state_gated(unified_json, capable_json, legacy_json);
    }

    // Update the reconnect snapshot.
    *shared_queue.queue.lock().unwrap() = queue.clone();
    *shared_queue.source.lock().unwrap() = source.clone();
}

/// Splits a canonical queue into the legacy `(Vec<EmbyItem>, Vec<FeedEntry>)`
/// shape for `CtrlState` construction.  Feed entries appear at the tail
/// matching the legacy split contract.
fn split_queue_for_legacy(queue: &PlaybackQueue) -> (Vec<EmbyItem>, Vec<FeedEntry>) {
    let mut emby = Vec::new();
    let mut feed = Vec::new();
    for slot in queue.slots() {
        match &slot.item {
            QueueItem::Emby(e) => emby.push((**e).clone()),
            QueueItem::Feed(f) => feed.push(f.clone()),
            QueueItem::Audiobookshelf(_) => {}
        }
    }
    (emby, feed)
}

fn admit_queue_items(
    original: Vec<QueueItem>,
    requested_cursor: Option<usize>,
    audio_only: bool,
    has_emby: bool,
) -> (Vec<QueueItem>, usize) {
    let requested = requested_cursor.unwrap_or(0);
    let rebased = original
        .iter()
        .take(requested)
        .filter(|item| daemon_admits(item, audio_only, has_emby))
        .count();
    let items: Vec<_> = original
        .into_iter()
        .filter(|item| daemon_admits(item, audio_only, has_emby))
        .collect();
    let cursor = if items.is_empty() {
        0
    } else {
        rebased.min(items.len() - 1)
    };
    (items, cursor)
}

fn daemon_admits(item: &QueueItem, audio_only: bool, has_emby: bool) -> bool {
    if item.is_emby() && !has_emby {
        return false;
    }
    item.admissible_for_owner(audio_only, |kind| {
        kind != crate::config::ServiceKind::Emby || has_emby
    })
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
    if ctrl_clients
        .lock()
        .unwrap()
        .supports_unified_queue(client_id)
    {
        send_to(reply_tx, &unified_queue_state(&status, queue, source));
    } else {
        let (emby_items, feed_items) = split_queue_for_legacy(queue);
        let include_feed = ctrl_clients
            .lock()
            .unwrap()
            .supports_feed_playback(client_id);
        let feed = if include_feed { feed_items } else { Vec::new() };
        send_to(
            reply_tx,
            &CtrlEvent::State(CtrlState {
                status,
                items: emby_items,
                cursor: queue.legacy_cursor(include_feed),
                source: source.clone(),
                feed_items: feed,
            }),
        );
    }
}
