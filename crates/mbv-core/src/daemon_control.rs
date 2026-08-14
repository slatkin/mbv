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
    let unified_json = serialize_ctrl_event(&CtrlEvent::UnifiedQueueState(
        crate::ctrl::UnifiedQueueStateData {
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
        },
    ));

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
        send_to(
            reply_tx,
            &CtrlEvent::UnifiedQueueState(crate::ctrl::UnifiedQueueStateData {
                status,
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
            }),
        );
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
    mut resolved_items: Option<Result<Vec<EmbyItem>, String>>,
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
        CtrlCmd::AdoptQueue {
            items: new_items,
            cursor: new_cursor,
            source: new_source,
        } => {
            // Adoption only applies to a Cold daemon — one with no queue yet.
            if !queue.is_empty() {
                log::warn!(
                    target: "daemon",
                    "ignoring AdoptQueue: daemon already has a queue ({} slot(s))",
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
            let queue_items: Vec<QueueItem> = new_items
                .into_iter()
                .map(|item| QueueItem::Emby(Box::new(item)))
                .collect();
            let next_cursor = if queue_items.is_empty() {
                0
            } else {
                new_cursor.min(queue_items.len().saturating_sub(1))
            };
            player.set_initial_queue(&queue_items, next_cursor);
            *queue = PlaybackQueue::from_queue_items(queue_items, Some(next_cursor));
            *source = new_source;
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
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
            let (items, next_cursor) = admit_queue_items(items, Some(cursor), audio_only, has_emby);
            player.set_initial_queue(&items, next_cursor);
            *queue = PlaybackQueue::from_queue_items(items, Some(next_cursor));
            *source = new_source;
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
        }
        CtrlCmd::PlayerCmd(pc) => {
            // Legacy wire-compat adapter: intercept LoadFeed before
            // converting to PlayerCommand so it routes through the
            // unified queue path.  Old peers send WireCommand::LoadFeed;
            // current peers never emit it.
            if let WireCommand::LoadFeed { entry } = &pc {
                let slot_id = queue.append(QueueItem::Feed(entry.clone()));
                let _ = queue.set_active_slot(slot_id);
                broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
                player.send_command(PlayerCommand::SubmitQueue {
                    items: queue.slots().iter().map(|s| s.item.clone()).collect(),
                    start_idx: queue.active_index().unwrap_or(0),
                });
                return;
            }
            match PlayerCommand::from(pc) {
                PlayerCommand::ReplaceQueue {
                    items: new_items,
                    start_idx,
                } => {
                    if !ctrl_clients
                        .lock()
                        .unwrap()
                        .supports_unified_queue(client_id)
                        && queue.has_feed_entries()
                    {
                        reject_command(
                            request.reply_tx,
                            ctrl_clients,
                            client_id,
                            player,
                            queue,
                            source,
                            "queue contains Feed entries invisible to legacy peer; replace skipped"
                                .to_string(),
                        );
                        return;
                    }
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
            }
        }
        CtrlCmd::PlayItems {
            item_ids,
            start_idx,
            start_ticks,
            source: new_source,
        } => {
            if !has_emby {
                return;
            }
            let fetched = match resolved_items.take() {
                Some(Ok(v)) => v,
                Some(Err(e)) => {
                    log::warn!(target: "daemon", "ctrl play error: {e}");
                    send_to(request.reply_tx, &CtrlEvent::CommandRejected(e));
                    return;
                }
                None => {
                    let c = client.lock().unwrap();
                    match c.get_items_by_ids(&item_ids) {
                        Ok(v) => v,
                        Err(e) => {
                            log::warn!(target: "daemon", "ctrl play error: {e}");
                            return;
                        }
                    }
                }
            };
            if fetched.is_empty() {
                log::warn!(
                    target: "daemon",
                    "ctrl play: fetched items are empty, discarding request"
                );
                return;
            }
            if let Some(reason) = audio_only_rejection(
                audio_only,
                &fetched
                    .iter()
                    .cloned()
                    .map(|e| QueueItem::Emby(Box::new(e)))
                    .collect::<Vec<_>>(),
            ) {
                log::warn!(target: "daemon", "rejecting ctrl play request: {reason}");
                send_to(request.reply_tx, &CtrlEvent::CommandRejected(reason));
                return;
            }
            if !ctrl_clients
                .lock()
                .unwrap()
                .supports_unified_queue(client_id)
                && queue.has_feed_entries()
            {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    queue,
                    source,
                    "queue contains Feed entries invisible to legacy peer; play skipped"
                        .to_string(),
                );
                return;
            }
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
                    let command = CtrlCmd::PlayItems {
                        item_ids: item_ids.clone(),
                        start_idx,
                        start_ticks,
                        source: intent_source,
                    };
                    let tx = merged_tx.clone();
                    let lookup_client = client.lock().unwrap().clone();
                    let request_id = intent.request_id;
                    let generation = intent.generation;
                    let reply_tx = (*request.reply_tx).clone();
                    std::thread::spawn(move || {
                        let fetched = lookup_client.get_items_by_ids(&item_ids);
                        let _ = tx.send(DaemonEvent::PlaybackResolved {
                            command,
                            client_id,
                            reply_tx,
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
        CtrlCmd::RequestShutdown => {}
        CtrlCmd::ApplyServiceSetup { .. } => {}
        // ── Unified queue commands ──────────────────────────────────────
        CtrlCmd::UnifiedQueueReplace { items, start_idx } => {
            let (items, next_cursor) = admit_queue_items(items, start_idx, audio_only, has_emby);
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
            let mut items = items;
            items.retain(|item| daemon_admits(item, audio_only, has_emby));
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
