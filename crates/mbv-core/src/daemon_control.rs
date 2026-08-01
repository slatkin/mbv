/// Applies a freshly-decided queue snapshot to the cross-thread shared state
/// (used to seed newly-connecting ctrl-socket clients) and broadcasts it to
/// every already-connected client. Emits slot-aware v8 state to v8 peers
/// and legacy positional state to v7 peers.
fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    source: &crate::config::QueueSource,
) {
    let q = shared_queue.queue.lock().unwrap();
    let status = player.status.lock().unwrap().clone();
    let items = q.items_snapshot();
    let cursor = q.current_index().unwrap_or(0);
    let slot_ids = q.slot_ids();
    let revision = q.revision();
    let active_slot_id = q.active_slot_id();
    drop(q);

    let source_clone = source.clone();
    let v7_event = CtrlEvent::State(CtrlState::v7(
        status.clone(),
        items.clone(),
        cursor,
        source_clone.clone(),
    ));
    let v7_json = serialize_ctrl_event(&v7_event);

    let v8_event = CtrlEvent::State(CtrlState {
        status: status.clone(),
        items: items.clone(),
        cursor,
        source: source_clone.clone(),
        slot_ids: slot_ids.clone(),
        revision,
        active_slot_id,
    });
    let v8_json = serialize_ctrl_event(&v8_event);

    let mut clients = ctrl_clients.lock().unwrap();
    clients.connection.retain(|c| {
        let json = if c.peer_version >= 8 { &v8_json } else { &v7_json };
        json.as_ref()
            .map(|j| c.tx.send(CtrlOutbound::Event(j.clone())).is_ok())
            .unwrap_or(false)
    });

    *shared_queue.source.lock().unwrap() = source_clone;
}

/// Sends a `CommandRejected` reply for `reason`, followed by the daemon's
/// current authoritative queue state, so the rejected caller resyncs
/// instead of drifting from the daemon's real queue.
fn reject_and_resync(
    reply_tx: &CtrlSender,
    player: &Player,
    shared_queue: &SharedQueueState,
    source: &crate::config::QueueSource,
    reason: &str,
) {
    send_to(reply_tx, &CtrlEvent::CommandRejected(reason.to_string()));
    let q = shared_queue.queue.lock().unwrap();
    let status = player.status.lock().unwrap().clone();
    let items = q.items_snapshot();
    let cursor = q.current_index().unwrap_or(0);
    let sids = q.slot_ids();
    let rev = q.revision();
    let a_sid = q.active_slot_id();
    drop(q);
    send_to(
        reply_tx,
        &CtrlEvent::State(CtrlState {
            status,
            items,
            cursor,
            source: source.clone(),
            slot_ids: sids,
            revision: rev,
            active_slot_id: a_sid,
        }),
    );
}

#[allow(clippy::too_many_arguments)]
fn handle_ctrl(
    cmd: CtrlCmd,
    _client_id: CtrlClientId,
    request: CtrlRequest<'_>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    playback_intents: &mut PlaybackIntentState,
    mut resolved_items: Option<Result<Vec<MediaItem>, String>>,
    _merged_tx: &mpsc::Sender<DaemonEvent>,
) {
    // Authority returns to Ctrl on the next ctrl command (not on connect).
    {
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
            if !shared_queue.queue.lock().unwrap().is_empty() {
                let q = shared_queue.queue.lock().unwrap();
                let items = q.items_snapshot();
                let cursor = q.current_index().unwrap_or(0);
                drop(q);
                log::warn!(
                    target: "daemon",
                    "ignoring AdoptQueue: daemon already has a queue ({} item(s))",
                    items.len()
                );
                send_to(
                    request.reply_tx,
                    &CtrlEvent::CommandRejected(
                        "daemon already has a queue; adoption skipped".to_string(),
                    ),
                );
                send_to(
                    request.reply_tx,
                    &CtrlEvent::State(CtrlState::v7(
                        player.status.lock().unwrap().clone(),
                        items,
                        cursor,
                        source.clone(),
                    )),
                );
                return;
            }
            let next_cursor = if new_items.is_empty() {
                0
            } else {
                new_cursor.min(new_items.len().saturating_sub(1))
            };
            player.set_initial_queue(&new_items, next_cursor);
            shared_queue.queue.lock().unwrap().replace_all(new_items, Some(next_cursor));
            broadcast_queue_state(ctrl_clients, player, shared_queue, &new_source);
            *source = new_source;
        }
        CtrlCmd::PlayerCmd(pc) => {
            // v8 slot-aware queue commands (handled before PlayerCommand conversion)
            match &pc {
                WireCommand::QueueRemoveBySlot { slot_id, revision } => {
                    let current_rev = shared_queue.queue.lock().unwrap().revision();
                    if *revision != current_rev {
                        log::warn!(target: "daemon", "QueueRemoveBySlot: stale revision (client={}, daemon={})", revision.raw(), current_rev.raw());
                        reject_and_resync(request.reply_tx, player, shared_queue, source, "stale revision");
                        return;
                    }
                    let mut q = shared_queue.queue.lock().unwrap();
                    let sid = *slot_id;
                    let idx = q.slot_index(sid).unwrap_or(0);
                    match q.remove_slot(sid) {
                        crate::playback_queue::RemoveSlotResult::Removed(_) => {}
                        crate::playback_queue::RemoveSlotResult::RequiresActiveConfirmation(_) => {
                            let _ = q.remove_active_slot_confirmed(sid);
                        }
                        crate::playback_queue::RemoveSlotResult::NotFound => {
                            log::warn!(target: "daemon", "QueueRemoveBySlot: slot {:?} not found", sid);
                            drop(q);
                            reject_and_resync(request.reply_tx, player, shared_queue, source, "slot not found");
                            return;
                        }
                    }
                    drop(q);
                    broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                    player.send_command(PlayerCommand::QueueRemove(idx));
                    return;
                }
                WireCommand::QueueMoveBySlot { slot_id, to_position, revision } => {
                    let current_rev = shared_queue.queue.lock().unwrap().revision();
                    if *revision != current_rev {
                        log::warn!(target: "daemon", "QueueMoveBySlot: stale revision");
                        reject_and_resync(request.reply_tx, player, shared_queue, source, "stale revision");
                        return;
                    }
                    let mut q = shared_queue.queue.lock().unwrap();
                    let from_idx = q.slot_index(*slot_id);
                    let move_result = q.move_slot(*slot_id, *to_position);
                    drop(q);
                    match (from_idx, move_result) {
                        (Some(from_idx), crate::playback_queue::QueueMutationResult::Applied(())) => {
                            broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                            player.send_command(PlayerCommand::QueueMove(from_idx, *to_position));
                        }
                        _ => {
                            log::warn!(target: "daemon", "QueueMoveBySlot: slot {:?} not found", slot_id);
                            reject_and_resync(request.reply_tx, player, shared_queue, source, "slot not found");
                        }
                    }
                    return;
                }
                WireCommand::JumpToSlot { slot_id } => {
                    let q = shared_queue.queue.lock().unwrap();
                    let idx = q.slot_index(*slot_id).unwrap_or(0);
                    drop(q);
                    player.send_command(PlayerCommand::JumpTo(idx));
                    return;
                }
                WireCommand::QueueInsertAt { item, position, revision } => {
                    let current_rev = shared_queue.queue.lock().unwrap().revision();
                    if *revision != current_rev {
                        log::warn!(target: "daemon", "QueueInsertAt: stale revision");
                        reject_and_resync(request.reply_tx, player, shared_queue, source, "stale revision");
                        return;
                    }
                    let mut q = shared_queue.queue.lock().unwrap();
                    let _new_sid = q.insert(*position, item.clone());
                    drop(q);
                    broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                    player.send_command(PlayerCommand::QueueAppend { items: vec![item.clone()] });
                    return;
                }
                WireCommand::QueueRemoveActive { revision } => {
                    let current_rev = shared_queue.queue.lock().unwrap().revision();
                    if *revision != current_rev {
                        log::warn!(target: "daemon", "QueueRemoveActive: stale revision");
                        reject_and_resync(request.reply_tx, player, shared_queue, source, "stale revision");
                        return;
                    }
                    // Remove active slot and clear active marker
                    {
                        let mut q = shared_queue.queue.lock().unwrap();
                        if let Some(active_id) = q.active_slot_id() {
                            let _ = q.remove_active_slot_confirmed(active_id);
                        }
                    }
                    broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                    player.stop();
                    log::info!(target: "daemon", "QueueRemoveActive: removed active slot, stopping player");
                    return;
                }
                _ => {
                    // Fall through to legacy v7 PlayerCommand handling
                }
            }
            match PlayerCommand::from(pc) {
                PlayerCommand::ReplaceQueue {
                    items: new_items,
                    start_idx,
                } => {
                    let next_cursor = if new_items.is_empty() {
                        0
                    } else {
                        start_idx.min(new_items.len().saturating_sub(1))
                    };
                    shared_queue.queue.lock().unwrap().replace_all(new_items.clone(), Some(next_cursor));
                    broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                    player.send_command(PlayerCommand::ReplaceQueue {
                        items: new_items,
                        start_idx,
                    });
                }
                PlayerCommand::QueueAppend { items: new_items } => {
                    if !new_items.is_empty() {
                        shared_queue.queue.lock().unwrap().append_items(new_items.clone());
                        broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                        player.send_command(PlayerCommand::QueueAppend { items: new_items });
                    }
                }
                PlayerCommand::QueueMove(from, to) => {
                    let queue_len = shared_queue.queue.lock().unwrap().len();
                    if from >= queue_len || to >= queue_len {
                        let q = shared_queue.queue.lock().unwrap();
                        let items = q.items_snapshot();
                        let cursor = q.current_index().unwrap_or(0);
                        drop(q);
                        send_to(
                            request.reply_tx,
                            &CtrlEvent::CommandRejected(
                                "remote queue changed; move skipped".to_string(),
                            ),
                        );
                        send_to(
                            request.reply_tx,
                            &CtrlEvent::State(CtrlState::v7(
                                player.status.lock().unwrap().clone(),
                                items,
                                cursor,
                                source.clone(),
                            )),
                        );
                    } else if from != to {
                        shared_queue.queue.lock().unwrap().move_item(from, to);
                        broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                        player.send_command(PlayerCommand::QueueMove(from, to));
                    }
                }
                PlayerCommand::QueueRemove(index) => {
                    let queue_len = shared_queue.queue.lock().unwrap().len();
                    if index >= queue_len {
                        let q = shared_queue.queue.lock().unwrap();
                        let items = q.items_snapshot();
                        let cursor = q.current_index().unwrap_or(0);
                        drop(q);
                        send_to(
                            request.reply_tx,
                            &CtrlEvent::CommandRejected(
                                "remote queue changed; remove skipped".to_string(),
                            ),
                        );
                        send_to(
                            request.reply_tx,
                            &CtrlEvent::State(CtrlState::v7(
                                player.status.lock().unwrap().clone(),
                                items,
                                cursor,
                                source.clone(),
                            )),
                        );
                    } else {
                        shared_queue.queue.lock().unwrap().remove_at(index);
                        broadcast_queue_state(ctrl_clients, player, shared_queue, source);
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
            if let Some(reason) = audio_only_rejection(audio_only, &fetched) {
                log::warn!(target: "daemon", "rejecting ctrl play request: {reason}");
                send_to(request.reply_tx, &CtrlEvent::CommandRejected(reason));
                return;
            }
            if fetched.len() == 1 {
                let item = fetched[0].clone();
                if !item.series_id.is_empty() && player.always_play_next {
                    let queue = client
                        .lock()
                        .unwrap()
                        .get_episodes_from(&item.series_id, &item.id);
                    if queue.len() > 1 {
                        shared_queue.queue.lock().unwrap().replace_all(queue.clone(), Some(0));
                        *source = new_source;
                        broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                        let c = Arc::new(client.lock().unwrap().clone());
                        player.play_queue(queue, 0, c, 100);
                        return;
                    }
                }
                shared_queue.queue.lock().unwrap().replace_all(vec![item.clone()], Some(0));
                *source = new_source;
                broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                let mut play_item = item;
                if start_ticks > 0 {
                    play_item.playback_position_ticks = start_ticks;
                }
                let c = Arc::new(client.lock().unwrap().clone());
                player.play(&play_item, c, 100);
            } else {
                let start_idx = start_idx.min(fetched.len().saturating_sub(1));
                let mut play_items = fetched.clone();
                if start_ticks > 0 {
                    play_items[start_idx].playback_position_ticks = start_ticks;
                }
                shared_queue.queue.lock().unwrap().replace_all(play_items.clone(), Some(start_idx));
                *source = new_source;
                broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                let c = Arc::new(client.lock().unwrap().clone());
                player.play_queue(play_items, start_idx, c, 100);
            }
        }
        CtrlCmd::Stop => {
            player.stop();
        }
        CtrlCmd::PlaybackIntent(intent) => {
            let pipe_output = client.lock().unwrap().config.audio_pipe_enabled;
            let accepted = playback_intents.accept(_client_id, intent.clone(), pipe_output);
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
                    let tx = _merged_tx.clone();
                    let lookup_client = client.lock().unwrap().clone();
                    let request_id = intent.request_id;
                    let generation = intent.generation;
                    let reply_tx = (*request.reply_tx).clone();
                    std::thread::spawn(move || {
                        let fetched = lookup_client.get_items_by_ids(&item_ids);
                        let _ = tx.send(DaemonEvent::PlaybackResolved {
                            command,
                            client_id: _client_id,
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
    }
}
