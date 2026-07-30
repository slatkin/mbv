/// Applies a freshly-decided queue snapshot to the cross-thread shared state
/// (used to seed newly-connecting ctrl-socket clients) and broadcasts it to
/// every already-connected client. Centralizes what `CtrlState`'s fields
/// must always carry together, so a future field addition (like `source` in
/// #113) can't land in only some of the call sites — previously this exact
/// shape was hand-rolled inline at five separate command branches.
fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    items: &[MediaItem],
    cursor: usize,
    source: &crate::config::QueueSource,
) {
    let event = CtrlEvent::State(CtrlState {
        status: player.status.lock().unwrap().clone(),
        items: items.to_vec(),
        cursor,
        source: source.clone(),
    });
    broadcast(ctrl_clients, &event);
    *shared_queue.cursor.lock().unwrap() = cursor;
    *shared_queue.source.lock().unwrap() = source.clone();
    if let CtrlEvent::State(state) = event {
        *shared_queue.items.lock().unwrap() = state.items;
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ctrl(
    cmd: CtrlCmd,
    _client_id: CtrlClientId,
    request: CtrlRequest<'_>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    items: &mut Vec<MediaItem>,
    cursor: &mut usize,
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
            // Adoption only ever applies to a Cold daemon (see CONTEXT.md's
            // "Cold daemon" entry) — one with no queue yet. If another
            // client's command already gave this daemon a queue by the time
            // this one arrives (a concurrent first-connect race), the daemon
            // is no longer cold, and a stale saved snapshot must not be
            // allowed to silently clobber whatever is already authoritative.
            if !items.is_empty() {
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
                // With multiple concurrent connections, reconciliation
                // pushes the daemon's authoritative State so the sending
                // client overwrites its optimistic mutation instead of
                // lingering diverged from what the daemon holds.
                send_to(
                    request.reply_tx,
                    &CtrlEvent::State(CtrlState {
                        status: player.status.lock().unwrap().clone(),
                        items: items.clone(),
                        cursor: *cursor,
                        source: source.clone(),
                    }),
                );
                return;
            }
            let next_cursor = if new_items.is_empty() {
                0
            } else {
                new_cursor.min(new_items.len().saturating_sub(1))
            };
            player.set_initial_queue(&new_items, next_cursor);
            broadcast_queue_state(
                ctrl_clients,
                player,
                shared_queue,
                &new_items,
                next_cursor,
                &new_source,
            );
            *items = new_items;
            *cursor = next_cursor;
            *source = new_source;
        }
        CtrlCmd::PlayerCmd(pc) => match PlayerCommand::from(pc) {
            PlayerCommand::ReplaceQueue {
                items: new_items,
                start_idx,
            } => {
                let next_cursor = if new_items.is_empty() {
                    0
                } else {
                    start_idx.min(new_items.len().saturating_sub(1))
                };
                *items = new_items.clone();
                *cursor = next_cursor;
                broadcast_queue_state(
                    ctrl_clients,
                    player,
                    shared_queue,
                    &new_items,
                    next_cursor,
                    source,
                );
                player.send_command(PlayerCommand::ReplaceQueue {
                    items: new_items,
                    start_idx,
                });
            }
            PlayerCommand::QueueAppend { items: new_items } => {
                if !new_items.is_empty() {
                    items.extend(new_items.clone());
                    broadcast_queue_state(
                        ctrl_clients,
                        player,
                        shared_queue,
                        items,
                        *cursor,
                        source,
                    );
                    player.send_command(PlayerCommand::QueueAppend { items: new_items });
                }
            }
            PlayerCommand::QueueMove(from, to) => {
                if from >= items.len() || to >= items.len() {
                    send_to(
                        request.reply_tx,
                        &CtrlEvent::CommandRejected(
                            "remote queue changed; move skipped".to_string(),
                        ),
                    );
                    send_to(
                        request.reply_tx,
                        &CtrlEvent::State(CtrlState {
                            status: player.status.lock().unwrap().clone(),
                            items: items.clone(),
                            cursor: *cursor,
                            source: source.clone(),
                        }),
                    );
                } else if from != to {
                    let item = items.remove(from);
                    items.insert(to, item);
                    *cursor = crate::player::shift_index_for_move(*cursor, from, to);
                    broadcast_queue_state(
                        ctrl_clients,
                        player,
                        shared_queue,
                        items,
                        *cursor,
                        source,
                    );
                    player.send_command(PlayerCommand::QueueMove(from, to));
                }
            }
            PlayerCommand::QueueRemove(index) => {
                if index >= items.len() {
                    send_to(
                        request.reply_tx,
                        &CtrlEvent::CommandRejected(
                            "remote queue changed; remove skipped".to_string(),
                        ),
                    );
                    send_to(
                        request.reply_tx,
                        &CtrlEvent::State(CtrlState {
                            status: player.status.lock().unwrap().clone(),
                            items: items.clone(),
                            cursor: *cursor,
                            source: source.clone(),
                        }),
                    );
                } else {
                    items.remove(index);
                    if items.is_empty() {
                        *cursor = 0;
                    } else if index < *cursor {
                        *cursor -= 1;
                    } else {
                        *cursor = (*cursor).min(items.len() - 1);
                    }
                    broadcast_queue_state(
                        ctrl_clients,
                        player,
                        shared_queue,
                        items,
                        *cursor,
                        source,
                    );
                    player.send_command(PlayerCommand::QueueRemove(index));
                }
            }
            other => {
                player.send_command(other);
            }
        },
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
                        *items = queue.clone();
                        *cursor = 0;
                        *source = new_source;
                        broadcast_queue_state(
                            ctrl_clients,
                            player,
                            shared_queue,
                            &queue,
                            0,
                            source,
                        );
                        let c = Arc::new(client.lock().unwrap().clone());
                        player.play_queue(queue, 0, c, 100);
                        return;
                    }
                }
                *items = vec![item.clone()];
                *cursor = 0;
                *source = new_source;
                broadcast_queue_state(ctrl_clients, player, shared_queue, items, 0, source);
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
                *items = play_items.clone();
                *cursor = start_idx;
                *source = new_source;
                broadcast_queue_state(
                    ctrl_clients,
                    player,
                    shared_queue,
                    &play_items,
                    start_idx,
                    source,
                );
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
