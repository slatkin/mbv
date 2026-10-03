use super::core::{
    DaemonEvent, broadcast_audiobookshelf_book_progress, broadcast_audiobookshelf_progress,
};
use super::{AudiobookshelfOwnerContext, ClientRegistry};
use mbv_audiobookshelf::{AudiobookshelfBookProgress, AudiobookshelfProgress};
use mbv_player::Player;
use mbv_queue::PlaybackQueue;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

/// Install (or clear) the daemon player's Audiobookshelf context from the
/// owner runtime, wiring the player's acknowledged-progress sender into the
/// daemon event loop. Mirrors the bare-mode install in the TUI app.
pub(super) fn install_daemon_audiobookshelf_context(
    player: &Player,
    runtime: Option<&AudiobookshelfOwnerContext>,
    merged_tx: &mpsc::Sender<DaemonEvent>,
) {
    let Some(runtime) = runtime else {
        player.update_audiobookshelf_context(None);
        return;
    };
    let Some(api_key) = mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf)
    else {
        player.update_audiobookshelf_context(None);
        return;
    };
    let Some(context) = mbv_player::AudiobookshelfPlayerContext::new(
        runtime.generation,
        runtime.setup.clone(),
        api_key,
        runtime.device_id.clone(),
    ) else {
        player.update_audiobookshelf_context(None);
        return;
    };
    let (progress_tx, progress_rx) = std::sync::mpsc::channel();
    let (book_progress_tx, book_progress_rx) = std::sync::mpsc::channel();
    player.update_audiobookshelf_context(Some(
        context
            .with_progress_updates(progress_tx)
            .with_book_progress_updates(book_progress_tx),
    ));
    let merged_tx = merged_tx.clone();
    let book_merged_tx = merged_tx.clone();
    std::thread::spawn(move || {
        for update in progress_rx {
            if merged_tx
                .send(DaemonEvent::AudiobookshelfProgress(update))
                .is_err()
            {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        for update in book_progress_rx {
            if book_merged_tx
                .send(DaemonEvent::AudiobookshelfBookProgress(update))
                .is_err()
            {
                break;
            }
        }
    });
}

/// Fetch current Audiobookshelf progress for the queue's episode and book
/// slots off the daemon event-loop thread, filter it to exactly those items,
/// and send one `DaemonEvent::AudiobookshelfProgressRefreshed`. A failed fetch
/// is logged and ignored, the same as an Emby enrichment failure.
pub(crate) fn spawn_audiobookshelf_progress_refresh(
    runtime: &AudiobookshelfOwnerContext,
    episode_keys: HashSet<(String, String)>,
    book_ids: HashSet<String>,
    merged_tx: &mpsc::Sender<DaemonEvent>,
) {
    let setup = runtime.setup.clone();
    let generation = runtime.generation;
    let tx = merged_tx.clone();
    std::thread::spawn(mbv_core::applog::carry_context(move || {
        let client = match mbv_audiobookshelf::AudiobookshelfClient::new(&setup.server_url) {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(name: "daemon.queue_enrichment.failed", target: "queue", error = %error, "Audiobookshelf progress refresh could not build a client");
                return;
            }
        };
        let Some(key) = mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf)
        else {
            tracing::warn!(name: "daemon.queue_enrichment.failed", target: "queue", "Audiobookshelf progress refresh has no Service secret");
            return;
        };
        // ponytail: `progress_bounded` and `book_progress_bounded` are two
        // identical `GET /api/me/progress` requests; merge them into one call
        // if the request count ever matters.
        let result = (|| {
            let progress = client.progress_bounded(
                &key,
                mbv_audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
            )?;
            let book_progress = client.book_progress_bounded(
                &key,
                mbv_audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
            )?;
            Ok::<_, mbv_audiobookshelf::AudiobookshelfError>((progress, book_progress))
        })();
        let (progress, book_progress) = match result {
            Ok(maps) => maps,
            Err(error) => {
                tracing::warn!(name: "daemon.queue_enrichment.failed", target: "queue", error = %error, "Audiobookshelf progress refresh failed");
                return;
            }
        };
        let progress = progress
            .into_iter()
            .filter(|(key, _)| episode_keys.contains(key))
            .collect();
        let book_progress = book_progress
            .into_iter()
            .filter(|(item_id, _)| book_ids.contains(item_id))
            .collect();
        let _ = tx.send(DaemonEvent::AudiobookshelfProgressRefreshed {
            generation,
            progress,
            book_progress,
        });
    }));
}

/// Apply one bulk Audiobookshelf progress refresh to the canonical Bound
/// queue. Unlike [`apply_audiobookshelf_progress`] this targets every matching
/// **non-active** slot, never overwrites the playing row, and sends no
/// per-item progress broadcast — the caller broadcasts the whole owner queue
/// once. Drops a stale setup generation without any side effect.
pub(crate) fn apply_audiobookshelf_progress_refresh(
    progress: &HashMap<(String, String), AudiobookshelfProgress>,
    book_progress: &HashMap<String, AudiobookshelfBookProgress>,
    update_generation: mbv_core::service_runtime::SetupGeneration,
    current_generation: Option<mbv_core::service_runtime::SetupGeneration>,
    queue: &mut PlaybackQueue,
) -> bool {
    if current_generation != Some(update_generation) {
        return false;
    }
    let active_id = queue.active_slot_id();
    let updates: Vec<(mbv_queue::QueueSlotId, i64, bool)> = queue
        .slots()
        .iter()
        .filter(|slot| Some(slot.slot_id) != active_id)
        .filter_map(|slot| {
            let (seconds, finished) = if let Some(episode) = slot.item.as_audiobookshelf() {
                progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .map_or((0.0, false), |fetched| {
                        (fetched.current_time_seconds, fetched.is_finished)
                    })
            } else {
                let book = slot.item.as_audiobookshelf_book()?;
                book_progress
                    .get(&book.library_item_id)
                    .map_or((0.0, false), |fetched| {
                        (fetched.current_time_seconds, fetched.is_finished)
                    })
            };
            Some((
                slot.slot_id,
                mbv_emby_model::seconds_to_ticks(seconds),
                finished,
            ))
        })
        .collect();
    let before = queue.revision();
    for (slot_id, position_ticks, is_finished) in updates {
        let _ = queue.apply_progress(slot_id, position_ticks, is_finished);
    }
    queue.revision() != before
}

/// Apply an acknowledged Audiobookshelf progress update to the canonical Bound
/// queue (matched by provider-qualified identity), then broadcast the redacted
/// progress to capable clients. Drops updates from a stale setup generation
/// without either side effect.
pub(crate) fn apply_audiobookshelf_progress(
    update: &mbv_player::AudiobookshelfProgressUpdate,
    current_generation: Option<mbv_core::service_runtime::SetupGeneration>,
    queue: &mut PlaybackQueue,
    ctrl_clients: &ClientRegistry,
) -> bool {
    let Some(current) = current_generation else {
        return false;
    };
    if current != update.generation {
        return false;
    }
    let position_ticks = mbv_emby_model::seconds_to_ticks(update.current_time_seconds);
    let matching_slot_ids: Vec<_> = queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item.as_audiobookshelf().and_then(|episode| {
                (episode.library_item_id == update.library_item_id
                    && episode.episode_id == update.episode_id)
                    .then_some(slot.slot_id)
            })
        })
        .collect();
    let active_id = queue.active_slot_id();
    let target = matching_slot_ids
        .iter()
        .copied()
        .find(|id| Some(*id) == active_id)
        .or_else(|| matching_slot_ids.first().copied());
    if let Some(slot_id) = target {
        let _ = queue.apply_progress(slot_id, position_ticks, update.is_finished);
    }
    broadcast_audiobookshelf_progress(
        ctrl_clients,
        mbv_ctrl::AudiobookshelfProgressEvent {
            library_item_id: update.library_item_id.clone(),
            episode_id: update.episode_id.clone(),
            position_ticks,
            is_finished: update.is_finished,
            setup_generation: update.generation.value(),
        },
    );
    target.is_some()
}

/// Apply an acknowledged Audiobookshelf book progress update to the canonical
/// Bound queue (matched by `library_item_id` only), then broadcast the
/// redacted book progress to capable clients. Drops updates from a stale
/// setup generation without either side effect.
pub(crate) fn apply_audiobookshelf_book_progress(
    update: &mbv_player::AudiobookshelfBookProgressUpdate,
    current_generation: Option<mbv_core::service_runtime::SetupGeneration>,
    queue: &mut PlaybackQueue,
    ctrl_clients: &ClientRegistry,
) -> bool {
    let Some(current) = current_generation else {
        return false;
    };
    if current != update.generation {
        return false;
    }
    let position_ticks = mbv_emby_model::seconds_to_ticks(update.current_time_seconds);
    let matching_slot_ids: Vec<_> = queue
        .slots()
        .iter()
        .filter_map(|slot| {
            slot.item.as_audiobookshelf_book().and_then(|book| {
                (book.library_item_id == update.library_item_id).then_some(slot.slot_id)
            })
        })
        .collect();
    let active_id = queue.active_slot_id();
    let target = matching_slot_ids
        .iter()
        .copied()
        .find(|id| Some(*id) == active_id)
        .or_else(|| matching_slot_ids.first().copied());
    if let Some(slot_id) = target {
        let _ = queue.apply_progress(slot_id, position_ticks, update.is_finished);
    }
    broadcast_audiobookshelf_book_progress(
        ctrl_clients,
        mbv_ctrl::AudiobookshelfBookProgressEvent {
            library_item_id: update.library_item_id.clone(),
            position_ticks,
            is_finished: update.is_finished,
            setup_generation: update.generation.value(),
        },
    );
    target.is_some()
}
