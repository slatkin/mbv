/// Builds a `QueueState` from the daemon's authoritative queue state and player status.
/// This is used for coordinated shutdown to persist the daemon's view of the queue,
/// not the client's potentially stale shadow.
fn project_queue_state(
    items: &[EmbyItem],
    feed_items: &[FeedEntry],
    cursor: usize,
    source: &crate::config::QueueSource,
    player_status: &crate::player::PlayerStatus,
) -> crate::config::QueueState {
    use std::collections::HashMap;

    let mut positions: HashMap<String, i64> = items
        .iter()
        .filter(|i| i.playback_position_ticks > 0 && !i.is_audio())
        .map(|i| (i.id.clone(), i.playback_position_ticks))
        .collect();

    // Incorporate the latest valid position for the active non-audio item.
    // Only persist positions for non-audio items (video requires resume).
    let total_len = items.len() + feed_items.len();
    if player_status.active
        && player_status.video_height > 0
        && player_status.current_idx < total_len
    {
        if player_status.current_idx < items.len() {
            if let Some(item) = items.get(player_status.current_idx) {
                let position = if player_status.last_valid_pos > 0 {
                    player_status.last_valid_pos
                } else {
                    player_status.position_ticks
                };
                positions.insert(item.id.clone(), position);
            }
        }
    }

    let last_played_item_id = if player_status.active && player_status.current_idx < total_len {
        if player_status.current_idx < items.len() {
            items
                .get(player_status.current_idx)
                .map(|item| item.id.clone())
        } else {
            feed_items
                .get(player_status.current_idx - items.len())
                .map(|entry| entry.guid.clone())
        }
    } else {
        None
    };

    let mut queue_items: Vec<crate::playback_queue::QueueItem> = items
        .iter()
        .cloned()
        .map(|item| crate::playback_queue::QueueItem::Emby(Box::new(item)))
        .collect();
    for entry in feed_items {
        queue_items.push(crate::playback_queue::QueueItem::Feed(entry.clone()));
    }

    crate::config::QueueState {
        source: source.clone(),
        items: queue_items,
        cursor,
        last_played_item_id,
        // Deliberately false: the daemon doesn't track completion, and
        // `actions.rs` only advances the restore cursor past an item when
        // this is true, so a false-but-wrong value merely restores onto the
        // last-played item instead of incorrectly skipping past it.
        last_played_completed: false,
        positions,
    }
}

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
    items: &[EmbyItem],
    cursor: usize,
    source: &crate::config::QueueSource,
    feed_items: &[FeedEntry],
) {
    let status = player.status.lock().unwrap().clone();
    // Two variants of the same State event, differing only in feed_items:
    // capable peers (advertised `feed-playback` at Hello) get the real Feed
    // tail; legacy peers get an empty one (#5.1). Each is serialized once
    // and fanned out per-client by `broadcast_state_gated`.
    let capable_json = serialize_ctrl_event(&CtrlEvent::State(CtrlState {
        status: status.clone(),
        items: items.to_vec(),
        cursor,
        source: source.clone(),
        feed_items: feed_items.to_vec(),
    }));
    let legacy_json = serialize_ctrl_event(&CtrlEvent::State(CtrlState {
        status,
        items: items.to_vec(),
        cursor,
        source: source.clone(),
        feed_items: Vec::new(),
    }));
    if let (Some(capable_json), Some(legacy_json)) = (capable_json, legacy_json) {
        ctrl_clients
            .lock()
            .unwrap()
            .broadcast_state_gated(capable_json, legacy_json);
    }
    *shared_queue.cursor.lock().unwrap() = cursor;
    *shared_queue.source.lock().unwrap() = source.clone();
    *shared_queue.feed_items.lock().unwrap() = feed_items.to_vec();
    *shared_queue.items.lock().unwrap() = items.to_vec();
}

/// Feed tail to echo back to a single requester in a per-client rejection
/// `State` response (#5.1): empty unless that specific client advertised
/// `feed-playback` at Hello, mirroring the gating `broadcast_queue_state`
/// applies to the fan-out broadcast.
fn requester_feed_items(
    ctrl_clients: &ClientRegistry,
    client_id: CtrlClientId,
    feed_items: &[FeedEntry],
) -> Vec<FeedEntry> {
    if ctrl_clients
        .lock()
        .unwrap()
        .supports_feed_playback(client_id)
    {
        feed_items.to_vec()
    } else {
        Vec::new()
    }
}

/// Rejects a queue command with `reason` and echoes the daemon's
/// authoritative State back to the requester (existing rejection pattern,
/// previously hand-rolled at each call site). The requester's `feed_items`
/// tail is gated per #5.1. Used both by the pre-existing index-out-of-range
/// guards and the #5.3 Emby-then-Feed tail invariant guards.
#[allow(clippy::too_many_arguments)]
fn reject_command(
    reply_tx: &CtrlSender,
    ctrl_clients: &ClientRegistry,
    client_id: CtrlClientId,
    player: &Player,
    items: &[EmbyItem],
    cursor: usize,
    source: &crate::config::QueueSource,
    feed_items: &[FeedEntry],
    reason: String,
) {
    send_to(reply_tx, &CtrlEvent::CommandRejected(reason));
    send_to(
        reply_tx,
        &CtrlEvent::State(CtrlState {
            status: player.status.lock().unwrap().clone(),
            items: items.to_vec(),
            cursor,
            source: source.clone(),
            feed_items: requester_feed_items(ctrl_clients, client_id, feed_items),
        }),
    );
}

/// Removes a consumed `PlayerEvent::FeedConsumed` entry from the daemon's
/// Feed tail and rebroadcasts the updated state, per design.md: "A player
/// Feed-removal event updates the daemon's tail and the reconnect snapshot
/// before it broadcasts the next state." Reuses `broadcast_queue_state` so
/// the reconnect snapshot (`shared_queue`) updates atomically with the
/// broadcast, same as every other queue-mutating command in this file.
#[allow(clippy::too_many_arguments)]
fn handle_feed_consumed(
    guid: &str,
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    items: &[EmbyItem],
    cursor: usize,
    source: &crate::config::QueueSource,
    feed_items: &mut Vec<FeedEntry>,
) {
    feed_items.retain(|entry| entry.guid != guid);
    broadcast_queue_state(
        ctrl_clients,
        player,
        shared_queue,
        items,
        cursor,
        source,
        feed_items.as_slice(),
    );
}

#[allow(clippy::too_many_arguments)]
fn handle_ctrl(
    cmd: CtrlCmd,
    client_id: CtrlClientId,
    request: CtrlRequest<'_>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    audio_only: bool,
    items: &mut Vec<EmbyItem>,
    cursor: &mut usize,
    source: &mut crate::config::QueueSource,
    feed_items: &mut Vec<FeedEntry>,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
    playback_intents: &mut PlaybackIntentState,
    mut resolved_items: Option<Result<Vec<EmbyItem>, String>>,
    merged_tx: &mpsc::Sender<DaemonEvent>,
) {
    // Handle lifecycle requests before the authority transition.
    // RequestShutdown must be authorized by transport before any state changes.
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

        // Build authoritative queue state for persistence.
        let player_status = player.status.lock().unwrap().clone();
        let mut queue_state =
            project_queue_state(items, feed_items, *cursor, source, &player_status);

        // Preserve existing non-empty snapshot when authoritative queue is empty.
        // This matches the no-clear-on-quit rule: quitting is not an explicit Clear Queue action.
        if queue_state.items.is_empty() {
            if let Some(existing) = crate::config::load_queue_state() {
                if !existing.items.is_empty() {
                    queue_state = existing;
                }
            }
        }

        // Persist before acceptance. On failure, reject and return.
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

        // Send acceptance, then enqueue shutdown through merged event channel.
        send_to(request.reply_tx, &CtrlEvent::ShutdownAccepted);
        let _ = merged_tx.send(DaemonEvent::Shutdown);
        return;
    }

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
            // #5.3: while the Feed tail is nonempty, capable clients address
            // a mixed Emby-then-Feed queue, so an Emby mutation that could
            // place Emby content after a Feed item is rejected outright.
            if !feed_items.is_empty() {
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    items,
                    *cursor,
                    source,
                    feed_items,
                    "queue has an active Feed tail; adoption skipped".to_string(),
                );
                return;
            }
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
                // With multiple concurrent connections, reconciliation
                // pushes the daemon's authoritative State so the sending
                // client overwrites its optimistic mutation instead of
                // lingering diverged from what the daemon holds.
                reject_command(
                    request.reply_tx,
                    ctrl_clients,
                    client_id,
                    player,
                    items,
                    *cursor,
                    source,
                    feed_items,
                    "daemon already has a queue; adoption skipped".to_string(),
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
                feed_items.as_slice(),
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
                // #5.3: see the AdoptQueue guard above for rationale.
                if !feed_items.is_empty() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        items,
                        *cursor,
                        source,
                        feed_items,
                        "queue has an active Feed tail; replace skipped".to_string(),
                    );
                    return;
                }
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
                    feed_items.as_slice(),
                );
                player.send_command(PlayerCommand::ReplaceQueue {
                    items: new_items,
                    start_idx,
                });
            }
            PlayerCommand::QueueAppend { items: new_items } => {
                // #5.3: see the AdoptQueue guard above for rationale.
                if !feed_items.is_empty() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        items,
                        *cursor,
                        source,
                        feed_items,
                        "queue has an active Feed tail; append skipped".to_string(),
                    );
                    return;
                }
                if !new_items.is_empty() {
                    items.extend(new_items.clone());
                    broadcast_queue_state(
                        ctrl_clients,
                        player,
                        shared_queue,
                        items,
                        *cursor,
                        source,
                        feed_items.as_slice(),
                    );
                    player.send_command(PlayerCommand::QueueAppend { items: new_items });
                }
            }
            PlayerCommand::QueueMove(from, to) => {
                // #5.3: see the AdoptQueue guard above for rationale.
                if !feed_items.is_empty() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        items,
                        *cursor,
                        source,
                        feed_items,
                        "queue has an active Feed tail; move skipped".to_string(),
                    );
                } else if from >= items.len() || to >= items.len() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        items,
                        *cursor,
                        source,
                        feed_items,
                        "remote queue changed; move skipped".to_string(),
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
                        feed_items.as_slice(),
                    );
                    player.send_command(PlayerCommand::QueueMove(from, to));
                }
            }
            PlayerCommand::QueueRemove(index) => {
                if index >= items.len() {
                    reject_command(
                        request.reply_tx,
                        ctrl_clients,
                        client_id,
                        player,
                        items,
                        *cursor,
                        source,
                        feed_items,
                        "remote queue changed; remove skipped".to_string(),
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
                        feed_items.as_slice(),
                    );
                    player.send_command(PlayerCommand::QueueRemove(index));
                }
            }
            PlayerCommand::LoadFeed { entry } => {
                // No owner/availability rejection here (settled design
                // decision, see #5.2): the daemon always records the entry
                // into its Feed tail and forwards it to its own Player.
                feed_items.push(entry.clone());
                broadcast_queue_state(
                    ctrl_clients,
                    player,
                    shared_queue,
                    items,
                    *cursor,
                    source,
                    feed_items.as_slice(),
                );
                // Use the lifecycle-capable play_feed path rather than a
                // bare send_command: on a cold daemon (no player thread
                // yet), send_command returns false and the failure was
                // silently ignored.  play_feed handles both the fast path
                // (active mpv with matching headless state → reuses the
                // window) and the cold path (stop/join/spawn fresh mpv).
                // The daemon always runs headless.
                let volume = player.status.lock().unwrap().volume.clamp(0, 200) as u8;
                player.play_feed(entry, true, volume);
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
                *items = vec![item.clone()];
                *cursor = 0;
                *source = new_source;
                broadcast_queue_state(
                    ctrl_clients,
                    player,
                    shared_queue,
                    items,
                    0,
                    source,
                    feed_items.as_slice(),
                );
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
                    feed_items.as_slice(),
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
        // RequestShutdown is handled before the authority transition above,
        // so this arm is unreachable in normal flow.
        CtrlCmd::RequestShutdown => {
            log::warn!(target: "daemon", "RequestShutdown reached the command match; handled earlier")
        }
    }
}
