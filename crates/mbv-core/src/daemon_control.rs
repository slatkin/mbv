include!("daemon_control_queue.rs");

/// Plays a resolved-by-id playback intent. Replaces the legacy `PlayItems`
/// wire command, which carried both the wire shape and this internal
/// control-flow re-entry; the wire variant is gone (ADR 0020), so the
/// resolved-play path now lives here as a plain function.
#[allow(clippy::too_many_arguments)]
fn play_resolved_items(
    fetched: Vec<EmbyItem>,
    start_idx: usize,
    start_ticks: i64,
    new_source: crate::config::QueueSource,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    queue: &mut PlaybackQueue,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    let queue_items: Vec<QueueItem> = fetched
        .iter()
        .cloned()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect();
    let start_idx = start_idx.min(queue_items.len().saturating_sub(1));
    *queue = PlaybackQueue::from_queue_items(queue_items, Some(start_idx));
    *source = new_source;
    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
    if fetched.len() == 1 {
        let mut play_item = fetched[0].clone();
        if start_ticks > 0 {
            play_item.playback_position_ticks = start_ticks;
        }
        let c = Arc::new(client.lock().unwrap().clone());
        player.play(&play_item, c, 100);
    } else {
        let mut play_items = fetched;
        if start_ticks > 0 {
            play_items[start_idx].playback_position_ticks = start_ticks;
        }
        let c = Arc::new(client.lock().unwrap().clone());
        player.play_queue(play_items, start_idx, c, 100);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ctrl(
    cmd: CtrlCmd,
    client_id: CtrlClientId,
    request: CtrlRequest<'_>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    queue: &mut PlaybackQueue,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    playback_intents: &mut PlaybackIntentState,
    has_audiobookshelf: bool,
    merged_tx: &mpsc::Sender<DaemonEvent>,
) {
    let has_emby = !client.lock().unwrap().token.is_empty();
    if matches!(cmd, CtrlCmd::RequestShutdown) {
        let is_local = ctrl_clients.lock().unwrap().is_local_client(client_id);
        if !is_local {
            send_to(
                request.reply_tx,
                &CtrlEvent::ShutdownRejected {
                    reason: "lifecycle requests require local transport".to_string(),
                },
            );
            return;
        }

        let player_status = player.status.lock().unwrap().clone();
        let mut queue_state = project_queue_state(queue, source, &player_status);

        if queue_state.items.is_empty() {
            if let Some(existing) = crate::config::load_queue_state() {
                if !existing.items.is_empty() {
                    queue_state = existing;
                }
            }
        }

        if let Err(e) = crate::config::save_queue_state(&queue_state) {
            log::error!(
                target: "daemon",
                "coordinated shutdown rejected: queue persistence failed: {e}"
            );
            send_to(
                request.reply_tx,
                &CtrlEvent::ShutdownRejected {
                    reason: format!("queue persistence failed: {e}"),
                },
            );
            return;
        }

        let mut clients = ctrl_clients.lock().unwrap();
        if clients.authority == AuthorityHolder::EmbyRemote {
            clients.authority = AuthorityHolder::Ctrl;
        }
    }

    match cmd {
        CtrlCmd::Hello(_) => {
            log::warn!(target: "daemon", "unexpected ctrl protocol hello after negotiation");
        }
        CtrlCmd::UnifiedAdoptQueue {
            items,
            cursor,
            source: new_source,
        } => {
            // Adoption only applies to a Cold daemon — one with no queue yet.
            if !queue.is_empty() {
                log::warn!(
                    target: "daemon",
                    "ignoring UnifiedAdoptQueue: daemon already has a queue ({} slot(s))",
                    queue.len()
                );
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    "daemon already has a queue; adoption skipped".to_string(),
                );
                return;
            }
            let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
            if let Some(reason) = abs_queue_transport_rejection(&items, supports_abs_queue) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    reason,
                );
                return;
            }
            let (items, next_cursor) = admit_queue_items(
                items,
                Some(cursor),
                audio_only,
                has_emby,
                has_audiobookshelf,
            );
            player.set_initial_queue(&items, next_cursor);
            *queue = PlaybackQueue::from_queue_items(items, Some(next_cursor));
            *source = new_source;
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
        }
        CtrlCmd::PlayerCmd(pc) => match PlayerCommand::from(pc) {
            PlayerCommand::ReplaceQueue {
                items: new_items,
                start_idx,
            } => {
                let queue_items: Vec<QueueItem> = new_items
                    .iter()
                    .cloned()
                    .map(|item| QueueItem::Emby(Box::new(item)))
                    .collect();
                let next_cursor = if queue_items.is_empty() {
                    0
                } else {
                    start_idx.min(queue_items.len().saturating_sub(1))
                };
                *queue = PlaybackQueue::from_queue_items(queue_items, Some(next_cursor));
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                player.send_command(PlayerCommand::SubmitQueue {
                    items: queue.slots().iter().map(|s| s.item.clone()).collect(),
                    start_idx: next_cursor,
                });
            }
            PlayerCommand::QueueAppend { items: new_items } => {
                if !new_items.is_empty() {
                    for item in new_items {
                        queue.append(item);
                    }
                    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                    player.send_command(PlayerCommand::SubmitQueue {
                        items: queue.slots().iter().map(|s| s.item.clone()).collect(),
                        start_idx: queue.active_index().unwrap_or(0),
                    });
                }
            }
            PlayerCommand::QueueMove(from, to) => {
                if from >= queue.len() || to >= queue.len() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        queue,
                        source,
                        "remote queue changed; move skipped".to_string(),
                    );
                } else if from != to {
                    let slot_id = queue.slots()[from].slot_id;
                    queue.move_slot(slot_id, to);
                    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                    player.send_command(PlayerCommand::QueueMove(from, to));
                }
            }
            PlayerCommand::QueueRemove(index) => {
                if index >= queue.len() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        queue,
                        source,
                        "remote queue changed; remove skipped".to_string(),
                    );
                } else {
                    let slot_id = queue.slots()[index].slot_id;
                    queue.consume_slot(slot_id);
                    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                    player.send_command(PlayerCommand::QueueRemove(index));
                }
            }
            other => {
                player.send_command(other);
            }
        },
        CtrlCmd::Stop => {
            player.stop();
        }
        CtrlCmd::PlaybackIntent(intent) => {
            let pipe_output = client.lock().unwrap().config.audio_pipe_enabled;
            let accepted = playback_intents.accept(client_id, intent.clone(), pipe_output);
            let coalesced = accepted.iter().any(|event| {
                matches!(
                    event.outcome,
                    crate::ctrl::PlaybackIntentOutcome::Coalesced { .. }
                )
            });
            for event in accepted {
                send_to(request.reply_tx, &CtrlEvent::PlaybackIntent(event));
            }
            if let Some(status) = playback_intents.pipe_status() {
                log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, playback_intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                send_to(request.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
            }
            if coalesced {
                return;
            }
            match intent.action {
                crate::ctrl::PlaybackIntentAction::Play {
                    item_ids,
                    start_idx,
                    start_ticks,
                    source: intent_source,
                } => {
                    if !has_emby {
                        return;
                    }
                    playback_intents.mark_resolving(intent.request_id);
                    if let Some(status) = playback_intents.pipe_status() {
                        log::info!(target: "pipe_latency", "request={} generation={} phase={:?} elapsed_ms={}", status.request_id, status.generation, status.phase, playback_intents.current.as_ref().map(|current| current.accepted_at.elapsed().as_millis()).unwrap_or_default());
                        send_to(request.reply_tx, &CtrlEvent::PipePlaybackStatus(status));
                    }
                    let tx = merged_tx.clone();
                    let lookup_client = client.lock().unwrap().clone();
                    let request_id = intent.request_id;
                    let generation = intent.generation;
                    std::thread::spawn(move || {
                        let fetched = lookup_client.get_items_by_ids(&item_ids);
                        let _ = tx.send(DaemonEvent::PlaybackResolved {
                            start_idx,
                            start_ticks,
                            source: intent_source,
                            client_id,
                            request_id,
                            generation,
                            fetched,
                        });
                    });
                }
                crate::ctrl::PlaybackIntentAction::Stop => player.stop(),
                crate::ctrl::PlaybackIntentAction::SetPaused { paused } => {
                    if player.status.lock().unwrap().paused != paused {
                        player.send_command(PlayerCommand::TogglePause);
                    }
                }
                crate::ctrl::PlaybackIntentAction::Next => {
                    if let Some(idx) = player.status.lock().unwrap().next_idx() {
                        playback_intents.set_target_idx(intent.request_id, idx);
                        player.send_command(PlayerCommand::JumpTo(idx));
                    }
                }
                crate::ctrl::PlaybackIntentAction::Previous => {
                    if let Some(idx) = player.status.lock().unwrap().previous_idx() {
                        playback_intents.set_target_idx(intent.request_id, idx);
                        player.send_command(PlayerCommand::JumpTo(idx));
                    }
                }
            }
        }
        CtrlCmd::RequestShutdown => {
            send_to(request.reply_tx, &CtrlEvent::ShutdownAccepted);
            let _ = merged_tx.send(DaemonEvent::Shutdown);
        }
        CtrlCmd::ApplyServiceSetup { .. } => {}
        // ── Unified queue commands ──────────────────────────────────────
        CtrlCmd::UnifiedQueueReplace { items, start_idx } => {
            let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
            if let Some(reason) = abs_queue_transport_rejection(&items, supports_abs_queue) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    reason,
                );
                return;
            }
            let (items, next_cursor) =
                admit_queue_items(items, start_idx, audio_only, has_emby, has_audiobookshelf);
            if items.is_empty() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    "Playback owner rejected the queue replacement".to_string(),
                );
                return;
            }
            // Audio-only admission: reject if the daemon is in audio-only
            // mode and any item is non-audio.
            if let Some(reason) = audio_only_rejection(audio_only, &items) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    reason,
                );
                return;
            }
            *queue = PlaybackQueue::from_queue_items(items, Some(next_cursor));
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
            // `send_command` alone only reaches an already-running mpv
            // thread; on a freshly started daemon no thread exists yet, so
            // route through `submit_queue`, which cold-starts one when
            // needed (matches the legacy PlayItems path via play/play_queue).
            let queue_items: Vec<QueueItem> =
                queue.slots().iter().map(|s| s.item.clone()).collect();
            let all_audio = queue_items.iter().all(|item| item.is_audio());
            let c = Arc::new(client.lock().unwrap().clone());
            let headless = player.headless_for(&c, all_audio);
            player.submit_queue(queue_items, next_cursor, Some(c), headless, 100);
        }
        CtrlCmd::UnifiedQueueAppend { items } => {
            if items.is_empty() {
                return;
            }
            let supports_abs_queue = ctrl_clients.lock().unwrap().supports_abs_queue(client_id);
            if let Some(reason) = abs_queue_transport_rejection(&items, supports_abs_queue) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    reason,
                );
                return;
            }
            let mut items = items;
            items.retain(|item| daemon_admits(item, audio_only, has_emby, has_audiobookshelf));
            if items.is_empty() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    "Playback owner rejected the queue append".to_string(),
                );
                return;
            }
            // Audio-only admission: reject if any appended item is non-audio.
            if let Some(reason) = audio_only_rejection(audio_only, &items) {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    reason,
                );
                return;
            }
            let items_for_player = items.clone();
            for item in items {
                queue.append(item);
            }
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
            // Append to the player's queue rather than replacing the whole queue.
            player.send_command(PlayerCommand::QueueAppend {
                items: items_for_player,
            });
        }
        CtrlCmd::UnifiedQueueRemoveSlot { slot_id } => {
            let sid = QueueSlotId::from_raw(slot_id);
            if queue.slot(sid).is_none() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    "slot not found; remove skipped".to_string(),
                );
            } else if queue.active_slot_id() == Some(sid) {
                let idx = queue.slot_index(sid).unwrap_or(0);
                queue.remove_active_slot_confirmed(sid);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                if queue.is_empty() {
                    // Clear the player's queue and stop.
                    player.send_command(PlayerCommand::SubmitQueue {
                        items: Vec::new(),
                        start_idx: 0,
                    });
                    player.stop();
                } else {
                    player.send_command(PlayerCommand::QueueRemove(idx));
                }
            } else {
                let idx = queue.slot_index(sid).unwrap_or(0);
                queue.remove_slot(sid);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                player.send_command(PlayerCommand::QueueRemove(idx));
            }
        }
        CtrlCmd::UnifiedQueueMoveSlot { slot_id, to_index } => {
            let sid = QueueSlotId::from_raw(slot_id);
            if queue.slot(sid).is_none() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    "slot not found; move skipped".to_string(),
                );
            } else {
                let from_idx = queue.slot_index(sid).unwrap_or(0);
                queue.move_slot(sid, to_index);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                player.send_command(PlayerCommand::QueueMove(from_idx, to_index));
            }
        }
        CtrlCmd::UnifiedQueuePlaySlot { slot_id } => {
            let sid = QueueSlotId::from_raw(slot_id);
            match queue.set_active_slot(sid) {
                crate::playback_queue::QueueMutationResult::Applied(()) => {
                    broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                    if let Some(idx) = queue.active_index() {
                        player.send_command(PlayerCommand::JumpTo(idx));
                    }
                }
                crate::playback_queue::QueueMutationResult::NotFound => {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        queue,
                        source,
                        "slot not found; play skipped".to_string(),
                    );
                }
            }
        }
        CtrlCmd::UnifiedQueueClear => {
            queue.clear();
            player.send_command(PlayerCommand::SubmitQueue {
                items: Vec::new(),
                start_idx: 0,
            });
            player.stop();
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
        }
    }
}
