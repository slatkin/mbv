/// Builds a `QueueState` from the daemon's canonical queue and player status.
/// Used for coordinated shutdown persistence. The snapshot is handed to the
/// injected `store`, so tests can observe it without touching real state.
fn persist_stay_alive_owner_queue(
    owner: &DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    store: &mut dyn FnMut(&crate::config::StayAliveQueueState) -> Result<(), String>,
) -> Result<(), String> {
    store(&crate::config::StayAliveQueueState {
        queue: project_queue_state(
            &owner.core.queue,
            &owner.core.source,
            &player.status.lock().unwrap(),
        ),
        lineage: *shared_queue.lineage.lock().unwrap(),
    })
}

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
    lineage: crate::ctrl::QueueLineage,
    observed_active_slot: Option<crate::playback_queue::QueueSlotId>,
    in_flight_transition: Option<crate::ctrl::TransitionSummary>,
    queued_latest_transition: Option<crate::ctrl::TransitionSummary>,
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
    // A Playback-run observation is the authority for the active slot (design
    // D3), but a cold-started queue (`submit_queue_slots` at an index) plays its
    // first track without any track-to-track transition, so no observation is
    // ever emitted and `observed_active_slot` stays `None`. Fall back to the
    // canonical queue's own active slot when the daemon is actually playing,
    // so peers don't strand their now-playing highlight on a stale row.
    let active_slot = observed_active_slot
        .or_else(|| status.active.then(|| queue.active_slot_id()).flatten())
        .map(crate::ctrl::slot_id_to_u64)
        .filter(|active_id| slots.iter().any(|s| s.slot_id == *active_id));
    CtrlEvent::UnifiedQueueState(crate::ctrl::UnifiedQueueStateData {
        status: status.clone(),
        slots,
        active_slot,
        revision: queue.revision().raw(),
        source: source.clone(),
        lineage,
        in_flight_transition,
        queued_latest_transition,
    })
}

/// Broadcasts a queue snapshot to clients and shared state.
fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    queue: &PlaybackQueue,
    source: &crate::config::QueueSource,
    transitions: &crate::playback_transition::OwnerTransitionState,
) {
    let status = player.status.lock().unwrap().clone();
    let (in_flight, queued_latest) = transitions.summaries();
    let observed_active_slot = *shared_queue.observed_active_slot.lock().unwrap();
    let lineage = *shared_queue.lineage.lock().unwrap();

    // ── Unified-queue peers, gate ABS episodes and books independently ──
    let unified_full_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, lineage, observed_active_slot, in_flight.clone(), queued_latest.clone(), true, true,
    ));
    let unified_abs_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, lineage, observed_active_slot, in_flight.clone(), queued_latest.clone(), true, false,
    ));
    let unified_book_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, lineage, observed_active_slot, in_flight.clone(), queued_latest.clone(), false, true,
    ));
    let unified_json = serialize_ctrl_event(&unified_queue_state_for_peer(
        &status, queue, source, lineage, observed_active_slot, in_flight, queued_latest, false, false,
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

/// Filters `original` to the entries the daemon admits, in one pass, and
/// rebases `requested_cursor` past any entries dropped ahead of it.
/// `item_of` projects each entry to the `QueueItem` `daemon_admits` judges,
/// so the same admission/rebase logic serves both bare-item and slot-tagged
/// callers without evaluating the admission predicate twice per entry.
fn admit_queue<T>(
    original: Vec<T>,
    requested_cursor: Option<usize>,
    audio_only: bool,
    has_emby: bool,
    has_audiobookshelf: bool,
    item_of: impl Fn(&T) -> &QueueItem,
) -> (Vec<T>, usize) {
    let requested = requested_cursor.unwrap_or(0);
    let mut rebased = 0;
    let admitted: Vec<T> = original
        .into_iter()
        .enumerate()
        .filter(|(index, entry)| {
            let admitted = daemon_admits(item_of(entry), audio_only, has_emby, has_audiobookshelf);
            if admitted && *index < requested {
                rebased += 1;
            }
            admitted
        })
        .map(|(_, entry)| entry)
        .collect();
    let cursor = if admitted.is_empty() {
        0
    } else {
        rebased.min(admitted.len() - 1)
    };
    (admitted, cursor)
}

fn admit_queue_items(
    original: Vec<QueueItem>,
    requested_cursor: Option<usize>,
    audio_only: bool,
    has_emby: bool,
    has_audiobookshelf: bool,
) -> (Vec<QueueItem>, usize) {
    admit_queue(
        original,
        requested_cursor,
        audio_only,
        has_emby,
        has_audiobookshelf,
        |item| item,
    )
}

pub(super) fn admit_queue_slots(
    original: Vec<(crate::playback_queue::QueueSlotId, QueueItem)>,
    requested_cursor: Option<usize>,
    audio_only: bool,
    has_emby: bool,
    has_audiobookshelf: bool,
) -> (Vec<(crate::playback_queue::QueueSlotId, QueueItem)>, usize) {
    admit_queue(
        original,
        requested_cursor,
        audio_only,
        has_emby,
        has_audiobookshelf,
        |(_, item)| item,
    )
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
fn abs_queue_transport_rejection<'a>(
    items: impl IntoIterator<Item = &'a QueueItem> + Clone,
    supports_abs_queue: bool,
    supports_abs_book_queue: bool,
) -> Option<String> {
    if !supports_abs_queue && items.clone().into_iter().any(QueueItem::is_audiobookshelf) {
        Some("peer did not negotiate Audiobookshelf queue transport".to_string())
    } else if !supports_abs_book_queue && items.into_iter().any(QueueItem::is_audiobookshelf_book) {
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
    lineage: crate::ctrl::QueueLineage,
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
            lineage,
            queue.active_slot_id(),
            None,
            None,
            supports_abs_queue,
            supports_abs_book_queue,
        ),
    );
}
