use crate::config::{EmbySetup, QueueSource};
use crate::ctrl::ServiceSetupRejection;

pub const EMBY_REPLACEMENT_FINALIZE_HARD_BOUND: Duration = Duration::from_secs(5);

fn normalized_server_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn same_server(old: &EmbyOwnerContext, new_setup: &EmbySetup) -> bool {
    normalized_server_url(&old.client.lock().unwrap().config.server_url)
        == normalized_server_url(&new_setup.server_url)
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn stop_old_emby_run(player: &Player) -> bool {
    let deadline = Instant::now() + EMBY_REPLACEMENT_FINALIZE_HARD_BOUND;
    player.stop_for_shutdown(remaining(deadline));
    player.join_or_timeout(remaining(deadline));
    !player.status.lock().unwrap().active
}

fn purge_emby_queue(queue: &mut PlaybackQueue) -> Vec<QueueItem> {
    let active = queue.active_slot_id();
    let retained: Vec<_> = queue
        .slots()
        .iter()
        .filter(|slot| !matches!(slot.item, QueueItem::Emby(_)))
        .map(|slot| (slot.slot_id, slot.item.clone()))
        .collect();
    let active = active.filter(|id| retained.iter().any(|(slot_id, _)| slot_id == id));
    let revision = queue.revision();
    *queue = PlaybackQueue::from_slot_items(retained.clone(), active, revision);
    retained.into_iter().map(|(_, item)| item).collect()
}

fn update_player_queue(
    player: &Player,
    items: Vec<QueueItem>,
    active_index: Option<usize>,
    client: &Arc<Mutex<crate::api::EmbyClient>>,
) {
    let Some(active_index) = active_index else {
        return;
    };
    if items.is_empty() {
        player.stop();
        return;
    }
    let client = Arc::new(client.lock().unwrap().clone());
    let all_audio = items.iter().all(QueueItem::is_audio);
    let headless = player.headless_for(&client, all_audio);
    let _ = player.submit_queue(items, active_index, Some(client), headless, 100);
}

#[allow(clippy::too_many_arguments)]
fn reconcile_packaged_emby(
    requested_revision: u64,
    current: &mut Option<EmbyOwnerContext>,
    ws_send_tx: &mut Option<crate::ws::WsSender>,
    client: &Arc<Mutex<crate::api::EmbyClient>>,
    player: &Player,
    queue: &mut PlaybackQueue,
    source: &mut QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    merged_tx: &std::sync::mpsc::Sender<DaemonEvent>,
    direct_commands: &[String],
    audio_only: bool,
) -> Result<(), ServiceSetupRejection> {
    let owner_config =
        crate::config::load_config().map_err(|_| ServiceSetupRejection::StorageUnavailable)?;
    let setup = owner_config
        .emby_setup
        .as_ref()
        .ok_or(ServiceSetupRejection::StorageUnavailable)?;
    if setup.revision != requested_revision {
        return Err(ServiceSetupRejection::RevisionMismatch);
    }
    let next = EmbyOwnerContext::from_packaged_storage_result(&owner_config)
        .map_err(|_| ServiceSetupRejection::StorageUnavailable)?;

    let is_replacement = current.as_ref().is_some_and(|old| !same_server(old, setup));
    if is_replacement {
        let active_old_emby = queue
            .active_slot()
            .is_some_and(|slot| matches!(slot.item, QueueItem::Emby(_)));
        if active_old_emby && !stop_old_emby_run(player) {
            return Err(ServiceSetupRejection::TransitionRejected);
        }
        let items = purge_emby_queue(queue);
        let active_index = queue.active_index();
        *source = if items.is_empty() {
            QueueSource::Unknown
        } else {
            source.clone()
        };
        *shared_queue.queue.lock().unwrap() = queue.clone();
        *shared_queue.source.lock().unwrap() = source.clone();
        broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
        *client.lock().unwrap() = next.client.lock().unwrap().clone();
        update_player_queue(player, items, active_index, client);
    } else {
        *client.lock().unwrap() = next.client.lock().unwrap().clone();
    }

    let generation = current
        .as_ref()
        .map(|old| crate::service_runtime::SetupGeneration::new(old.generation.value() + 1))
        .unwrap_or_default();
    let mut next = next;
    next.generation = generation;
    next.revision = requested_revision;
    *current = Some(next.clone());

    if let Some(previous_ws) = ws_send_tx.take() {
        previous_ws.shutdown();
    }
    let (ws_tx, ws_rx) = std::sync::mpsc::channel();
    let ws_sender = crate::ws::start(next.client.lock().unwrap().ws_url(), ws_tx);
    *ws_send_tx = Some(ws_sender.clone());
    player.update_emby_runtime(
        setup.server_url.clone(),
        next.client.lock().unwrap().token.clone(),
        ws_sender.clone(),
    );
    std::thread::spawn({
        let direct_commands = direct_commands.to_vec();
        let next_client = next.client.lock().unwrap().clone();
        move || register_capabilities(&next_client, &direct_commands, audio_only)
    });
    let merged_tx = merged_tx.clone();
    std::thread::spawn(move || {
        for event in ws_rx {
            let _ = merged_tx.send(DaemonEvent::Ws { generation, event });
        }
    });
    let _ = ws_sender.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
    Ok(())
}

/// Reconcile owner-local Audiobookshelf state by rereading owner storage.
/// A removal (no persisted setup) drops the context; a matching revision
/// installs a fresh context with an advanced generation; a mismatched
/// revision or unreadable storage rejects without changing the runtime.
/// Audiobookshelf admission and playback stay disabled regardless.
fn reconcile_packaged_audiobookshelf(
    requested_revision: u64,
    current: &mut Option<AudiobookshelfOwnerContext>,
) -> Result<(), ServiceSetupRejection> {
    let owner_config =
        crate::config::load_config().map_err(|_| ServiceSetupRejection::StorageUnavailable)?;
    let Some(setup) = owner_config.audiobookshelf_setup.as_ref() else {
        *current = None;
        return Ok(());
    };
    if setup.revision != requested_revision {
        return Err(ServiceSetupRejection::RevisionMismatch);
    }
    let next = AudiobookshelfOwnerContext::from_packaged_storage_result(&owner_config)
        .map_err(|_| ServiceSetupRejection::StorageUnavailable)?;
    let generation = current
        .as_ref()
        .map(|old| crate::service_runtime::SetupGeneration::new(old.generation.value() + 1))
        .unwrap_or_default();
    let mut next = next;
    next.generation = generation;
    *current = Some(next);
    Ok(())
}
