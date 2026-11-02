use super::{
    ClientRegistry, DaemonPlayerOwner, QueueTransport, SharedQueueState, serialize_ctrl_event,
};
use mbv_ctrl::CtrlEvent;
use mbv_player::Player;
use mbv_queue::{AudiobookshelfItem, PlaybackQueue, QueueItem};

/// Builds a `QueueState` from the daemon's canonical queue and player status.
/// Used for coordinated shutdown persistence. The snapshot is handed to the
/// injected `store`, so tests can observe it without touching real state.
pub(crate) fn stay_alive_owner_queue_snapshot(
    owner: &DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
) -> mbv_config::StayAliveQueueState {
    mbv_config::StayAliveQueueState {
        queue: project_queue_state(
            &owner.core.queue,
            &owner.core.source,
            &player.status.lock().unwrap(),
        ),
        lineage: *shared_queue.lineage.lock().unwrap(),
    }
}

pub(crate) fn persist_stay_alive_owner_queue(
    owner: &DaemonPlayerOwner,
    player: &Player,
    shared_queue: &SharedQueueState,
    store: &mut dyn FnMut(&mbv_config::StayAliveQueueState) -> Result<(), crate::DaemonLibError>,
) -> Result<(), crate::DaemonLibError> {
    store(&stay_alive_owner_queue_snapshot(
        owner,
        player,
        shared_queue,
    ))
}

pub(crate) fn project_queue_state(
    queue: &PlaybackQueue,
    source: &mbv_queue::QueueSource,
    player_status: &mbv_ctrl::player::PlayerStatus,
) -> mbv_queue::QueueState {
    use std::collections::HashMap;

    let slots = queue.slots();
    let active_idx = queue.active_index().unwrap_or(0);

    // Collect positions for non-audio items that have progress.
    let mut positions: HashMap<String, i64> = HashMap::new();
    for slot in slots {
        if slot.item.is_video() {
            let pos = slot.local_progress().position_ticks;
            if pos > 0 {
                positions.insert(slot.item.id().to_string(), pos);
            }
        }
    }

    // Incorporate the latest valid position for the active video item.
    if player_status.active
        && player_status.video_height > 0
        && active_idx < slots.len()
        && let Some(slot) = slots.get(active_idx)
        && slot.item.is_video()
    {
        let position = if player_status.last_valid_pos > 0 {
            player_status.last_valid_pos
        } else {
            player_status.position_ticks
        };
        positions.insert(slot.item.id().to_string(), position);
    }

    let last_played_item_id = (player_status.active && active_idx < slots.len())
        .then(|| slots[active_idx].item.id().to_string());

    let queue_items: Vec<QueueItem> = slots.iter().map(|s| s.item.clone()).collect();

    mbv_queue::QueueState {
        source: source.clone(),
        items: queue_items,
        cursor: active_idx,
        last_played_content_id: (player_status.active && active_idx < slots.len())
            .then(|| slots[active_idx].item.content_id()),
        last_played_item_id,
        last_played_completed: false,
        positions,
    }
}

/// Projects the canonical queue into `UnifiedQueueStateData` for one
/// connection. Audiobookshelf slots are included only when the peer
/// negotiated the matching capability (`abs-queue` for episodes,
/// `abs-book-queue` for books), and Twitch slots only with `twitch-queue`
/// (twitch-url-playback design D6); when a slot is dropped, `active_slot` is
/// cleared too if the active slot was itself dropped, so a peer never
/// receives an `active_slot` pointing at a slot missing from `slots`.
pub(crate) fn unified_queue_state_for_peer(
    status: &mbv_ctrl::player::PlayerStatus,
    queue: &PlaybackQueue,
    source: &mbv_queue::QueueSource,
    lineage: mbv_queue::QueueLineage,
    observed_active_slot: Option<mbv_queue::QueueSlotId>,
    in_flight_transition: Option<mbv_ctrl::TransitionSummary>,
    queued_latest_transition: Option<mbv_ctrl::TransitionSummary>,
    queue_transport: QueueTransport,
) -> CtrlEvent {
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = queue
        .slots()
        .iter()
        .filter(|s| match &s.item {
            QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(_)) => {
                queue_transport.abs_episode
            }
            QueueItem::Audiobookshelf(AudiobookshelfItem::Book(_)) => queue_transport.abs_book,
            QueueItem::Twitch(_) => queue_transport.twitch,
            QueueItem::Emby(_) | QueueItem::Feed(_) => true,
        })
        .map(|s| mbv_ctrl::UnifiedQueueSlot {
            slot_id: mbv_ctrl::slot_id_to_u64(s.slot_id),
            item: s.item.clone(),
        })
        .collect();
    // A Playback-run observation is the authority for the active slot (design
    // D3), but a cold-started queue (a fresh run spawned at an index) plays its
    // first track without any track-to-track transition, so no observation is
    // ever emitted and `observed_active_slot` stays `None`. Fall back to the
    // canonical queue's own active slot when the daemon is actually playing,
    // so peers don't strand their now-playing highlight on a stale row.
    let active_slot = observed_active_slot
        .or_else(|| status.active.then(|| queue.active_slot_id()).flatten())
        .map(mbv_ctrl::slot_id_to_u64)
        .filter(|active_id| slots.iter().any(|s| s.slot_id == *active_id));
    CtrlEvent::UnifiedQueueState(mbv_ctrl::UnifiedQueueStateData {
        status: status.clone(),
        slots,
        active_slot,
        // Owner-local diagnostic only; clients mint their own projection revision.
        revision: queue.revision().raw(),
        source: source.clone(),
        lineage,
        in_flight_transition,
        queued_latest_transition,
    })
}

/// Broadcasts a queue snapshot to clients and shared state.
pub(crate) fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    queue: &PlaybackQueue,
    source: &mbv_queue::QueueSource,
    transitions: &mbv_player::transition::OwnerTransitionState,
    except: Option<crate::CtrlClientId>,
) {
    let status = player.status.lock().unwrap().clone();
    let (in_flight, queued_latest) = transitions.summaries();
    let observed_active_slot = *shared_queue.observed_active_slot.lock().unwrap();
    let lineage = *shared_queue.lineage.lock().unwrap();

    // ── Unified-queue peers, gate ABS episodes, books, and Twitch
    //    independently ──. The projection serializes lazily, once per
    //    distinct capability combination among the connected peers.
    let mut projected: std::collections::HashMap<QueueTransport, Option<String>> =
        std::collections::HashMap::new();
    ctrl_clients.lock().unwrap().broadcast_state_gated(
        |queue_transport| {
            projected
                .entry(queue_transport)
                .or_insert_with(|| {
                    serialize_ctrl_event(&unified_queue_state_for_peer(
                        &status,
                        queue,
                        source,
                        lineage,
                        observed_active_slot,
                        in_flight.clone(),
                        queued_latest.clone(),
                        queue_transport,
                    ))
                })
                .clone()
        },
        except,
    );

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

pub(crate) fn admit_queue_items(
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

pub(crate) fn admit_queue_slots(
    original: Vec<(mbv_queue::QueueSlotId, QueueItem)>,
    requested_cursor: Option<usize>,
    audio_only: bool,
    has_emby: bool,
    has_audiobookshelf: bool,
) -> (Vec<(mbv_queue::QueueSlotId, QueueItem)>, usize) {
    admit_queue(
        original,
        requested_cursor,
        audio_only,
        has_emby,
        has_audiobookshelf,
        |(_, item)| item,
    )
}

pub(crate) fn daemon_admits(
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
        |kind| kind != mbv_queue::ServiceKind::Emby || has_emby,
        has_audiobookshelf,
    )
}

/// Returns a rejection reason when `items` contains an Audiobookshelf item
/// submitted by a peer that did not negotiate the matching queue transport
/// (`abs-queue` for episodes, `abs-book-queue` for books). Checked ahead of
/// queue mutation so an incapable peer's operation is refused outright rather
/// than silently dropping the unsupported item.
pub(crate) fn abs_queue_transport_rejection<'a>(
    items: impl IntoIterator<Item = &'a QueueItem> + Clone,
    queue_transport: QueueTransport,
) -> Option<String> {
    if !queue_transport.abs_episode
        && items
            .clone()
            .into_iter()
            .any(|item| item.as_audiobookshelf().is_some())
    {
        Some("peer did not negotiate Audiobookshelf queue transport".to_string())
    } else if !queue_transport.abs_book
        && items
            .into_iter()
            .any(|item| item.as_audiobookshelf_book().is_some())
    {
        Some("peer did not negotiate Audiobookshelf book queue transport".to_string())
    } else {
        None
    }
}

/// Returns a rejection reason when `items` contains a Twitch item submitted
/// by a peer that did not negotiate the `twitch-queue` queue transport
/// (twitch-url-playback design D6, spec *Twitch item transport is additively
/// negotiated*). Checked ahead of queue mutation so an incapable peer's
/// operation is refused outright rather than silently dropping the
/// unsupported item.
pub(crate) fn twitch_queue_transport_rejection<'a>(
    items: impl IntoIterator<Item = &'a QueueItem>,
    queue_transport: QueueTransport,
) -> Option<String> {
    (!queue_transport.twitch
        && items
            .into_iter()
            .any(|item| matches!(item, QueueItem::Twitch(_))))
    .then(|| "peer did not negotiate Twitch queue transport".to_string())
}
