/// Install (or clear) the daemon player's Audiobookshelf context from the
/// owner runtime, wiring the player's acknowledged-progress sender into the
/// daemon event loop. Mirrors the bare-mode install in the TUI app.
fn install_daemon_audiobookshelf_context(
    player: &Player,
    runtime: &Option<AudiobookshelfOwnerContext>,
    merged_tx: &mpsc::Sender<DaemonEvent>,
) {
    let Some(runtime) = runtime else {
        player.update_audiobookshelf_context(None);
        return;
    };
    let Some(api_key) =
        crate::config::load_service_secret(crate::config::ServiceKind::Audiobookshelf)
    else {
        player.update_audiobookshelf_context(None);
        return;
    };
    let Some(context) = crate::player::AudiobookshelfPlayerContext::new(
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

/// Apply an acknowledged Audiobookshelf progress update to the canonical Bound
/// queue (matched by provider-qualified identity), then broadcast the redacted
/// progress to capable clients. Drops updates from a stale setup generation
/// without either side effect.
fn apply_audiobookshelf_progress(
    update: crate::player::AudiobookshelfProgressUpdate,
    current_generation: Option<crate::service_runtime::SetupGeneration>,
    queue: &mut PlaybackQueue,
    ctrl_clients: &ClientRegistry,
) {
    let Some(current) = current_generation else {
        return;
    };
    if current != update.generation {
        return;
    }
    let position_ticks = (update.current_time_seconds * crate::api::TICKS_PER_SECOND as f64) as i64;
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
        crate::ctrl::AudiobookshelfProgressEvent {
            library_item_id: update.library_item_id.clone(),
            episode_id: update.episode_id.clone(),
            position_ticks,
            is_finished: update.is_finished,
            setup_generation: update.generation.value(),
        },
    );
}

/// Apply an acknowledged Audiobookshelf book progress update to the canonical
/// Bound queue (matched by `library_item_id` only), then broadcast the
/// redacted book progress to capable clients. Drops updates from a stale
/// setup generation without either side effect.
fn apply_audiobookshelf_book_progress(
    update: crate::player::AudiobookshelfBookProgressUpdate,
    current_generation: Option<crate::service_runtime::SetupGeneration>,
    queue: &mut PlaybackQueue,
    ctrl_clients: &ClientRegistry,
) {
    let Some(current) = current_generation else {
        return;
    };
    if current != update.generation {
        return;
    }
    let position_ticks = (update.current_time_seconds * crate::api::TICKS_PER_SECOND as f64) as i64;
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
        crate::ctrl::AudiobookshelfBookProgressEvent {
            library_item_id: update.library_item_id.clone(),
            position_ticks,
            is_finished: update.is_finished,
            setup_generation: update.generation.value(),
        },
    );
}
