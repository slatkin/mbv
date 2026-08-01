#!/usr/bin/env python3
"""Apply all daemon changes atomically."""
import sys, re

ROOT = "/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model"
DCORE = f"{ROOT}/crates/mbv-core/src/daemon_core.rs"
DCTRL = f"{ROOT}/crates/mbv-core/src/daemon_control.rs"
DTEST = f"{ROOT}/crates/mbv-core/src/daemon_tests.rs"

# ================================================================
# 1. daemon_core.rs
# ================================================================
with open(DCORE, "r") as f:
    dc = f.read()

# 1a. SharedQueueState migration
old_share = "struct SharedQueueState {\n    items: Arc<Mutex<Vec<MediaItem>>>,\n    cursor: Arc<Mutex<usize>>,\n    source: Arc<Mutex<crate::config::QueueSource>>,\n}"
new_share = "struct SharedQueueState {\n    queue: Arc<Mutex<crate::playback_queue::PlaybackQueue>>,\n    source: Arc<Mutex<crate::config::QueueSource>>,\n}"
assert old_share in dc, "SharedQueueState not found"
dc = dc.replace(old_share, new_share)

# 1b. CtrlClient: add peer_version
dc = dc.replace(
    "struct CtrlClient {\n    id: CtrlClientId,\n    tx: CtrlSender,\n}",
    "struct CtrlClient {\n    id: CtrlClientId,\n    tx: CtrlSender,\n    peer_version: u32,\n}")
assert "peer_version" in dc, "peer_version not added"

# 1c. connect() signature
dc = dc.replace(
    "fn connect(&mut self, tx: CtrlSender) -> CtrlClientId {",
    "fn connect(&mut self, tx: CtrlSender, peer_version: u32) -> CtrlClientId {")

# 1d. connect() body
dc = dc.replace(
    "self.connection.push(CtrlClient { id, tx });",
    "self.connection.push(CtrlClient { id, tx, peer_version });")

# 1e. Capture peer_version from hello match
old_match = """        match serde_json::from_str::<CtrlCmd>(&line) {
            Ok(CtrlCmd::Hello(info)) => {
                if let Err(e) = info.validate_peer() {
                    log::warn!(target: "daemon", "rejecting ctrl client: {e}");
                    return;
                }
                let Some(auth_token) = info.auth_token.as_deref() else {
                    log::warn!(target: "daemon", "rejecting ctrl client: missing Emby auth token");
                    return;
                };
                let validate_client = client.lock().unwrap().clone();
                if let Err(e) = validate_client.validate_presented_token(auth_token) {
                    log::warn!(
                        target: "daemon",
                        "rejecting ctrl client: presented Emby token validation failed: {e}"
                    );
                    return;
                }
            }
            Ok(_) => {
                log::warn!(target: "daemon", "rejecting ctrl client: missing protocol hello");
                return;
            }
            Err(e) => {
                log::warn!(target: "daemon", "rejecting ctrl client: invalid protocol hello: {e}");
                return;
            }
        }"""

new_match = """        let peer_version = match serde_json::from_str::<CtrlCmd>(&line) {
            Ok(CtrlCmd::Hello(info)) => {
                if let Err(e) = info.validate_peer() {
                    log::warn!(target: "daemon", "rejecting ctrl client: {e}");
                    return;
                }
                let Some(auth_token) = info.auth_token.as_deref() else {
                    log::warn!(target: "daemon", "rejecting ctrl client: missing Emby auth token");
                    return;
                };
                let validate_client = client.lock().unwrap().clone();
                if let Err(e) = validate_client.validate_presented_token(auth_token) {
                    log::warn!(
                        target: "daemon",
                        "rejecting ctrl client: presented Emby token validation failed: {e}"
                    );
                    return;
                }
                info.protocol_version
            }
            Ok(_) => {
                log::warn!(target: "daemon", "rejecting ctrl client: missing protocol hello");
                return;
            }
            Err(e) => {
                log::warn!(target: "daemon", "rejecting ctrl client: invalid protocol hello: {e}");
                return;
            }
        };"""
assert old_match in dc, "hello match block not found"
dc = dc.replace(old_match, new_match)

# 1f. Initial state: version-gated
old_init = """        if let Ok(init_json) = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: player_status.lock().unwrap().clone(),
            items: shared_queue.items.lock().unwrap().clone(),
            cursor: *shared_queue.cursor.lock().unwrap(),
            source: shared_queue.source.lock().unwrap().clone(),
        })) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }"""

new_init = """        let init_state = {
            let q = shared_queue.queue.lock().unwrap();
            let status = player_status.lock().unwrap().clone();
            let items = q.items_snapshot();
            let cursor = q.current_index().unwrap_or(0);
            let src = shared_queue.source.lock().unwrap().clone();
            if peer_version >= 8 {
                CtrlState {
                    status,
                    items,
                    cursor,
                    source: src,
                    slot_ids: q.slot_ids(),
                    revision: q.revision(),
                    active_slot_id: q.active_slot_id(),
                }
            } else {
                CtrlState::v7(status, items, cursor, src)
            }
        };
        if let Ok(init_json) = serde_json::to_string(&CtrlEvent::State(init_state)) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }"""
assert old_init in dc, f"init state block not found. Near: {dc[max(0,dc.find('if let Ok(init_json)')-20):dc.find('if let Ok(init_json)')+220]}"
dc = dc.replace(old_init, new_init)

# 1g. connect() call in spawn_ctrl_client
dc = dc.replace(
    "let client_id = ctrl_clients.lock().unwrap().connect(ev_tx);",
    "let client_id = ctrl_clients.lock().unwrap().connect(ev_tx, peer_version);")

with open(DCORE, "w") as f:
    f.write(dc)
print("daemon_core.rs: 7 changes applied")

# ================================================================
# 2. daemon_control.rs — write complete replacement
# ================================================================
with open(DCTRL, "w") as f:
    from textwrap import dedent
    f.write(dedent(r"""
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
                            send_to(request.reply_tx, &CtrlEvent::CommandRejected("stale revision".to_string()));
                            let q = shared_queue.queue.lock().unwrap();
                            let status = player.status.lock().unwrap().clone();
                            let items = q.items_snapshot();
                            let cursor = q.current_index().unwrap_or(0);
                            let sids = q.slot_ids();
                            let rev = q.revision();
                            let a_sid = q.active_slot_id();
                            drop(q);
                            send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                                status, items, cursor, source: source.clone(),
                                slot_ids: sids, revision: rev, active_slot_id: a_sid,
                            }));
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
                            send_to(request.reply_tx, &CtrlEvent::CommandRejected("stale revision".to_string()));
                            let q = shared_queue.queue.lock().unwrap();
                            let status = player.status.lock().unwrap().clone();
                            let items = q.items_snapshot();
                            let cursor = q.current_index().unwrap_or(0);
                            let sids = q.slot_ids();
                            let rev = q.revision();
                            let a_sid = q.active_slot_id();
                            drop(q);
                            send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                                status, items, cursor, source: source.clone(),
                                slot_ids: sids, revision: rev, active_slot_id: a_sid,
                            }));
                            return;
                        }
                        let mut q = shared_queue.queue.lock().unwrap();
                        let from_idx = q.slot_index(*slot_id).unwrap_or(0);
                        let _ = q.move_slot(*slot_id, *to_position);
                        drop(q);
                        broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                        player.send_command(PlayerCommand::QueueMove(from_idx, *to_position));
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
                            send_to(request.reply_tx, &CtrlEvent::CommandRejected("stale revision".to_string()));
                            let q = shared_queue.queue.lock().unwrap();
                            let status = player.status.lock().unwrap().clone();
                            let items = q.items_snapshot();
                            let cursor = q.current_index().unwrap_or(0);
                            let sids = q.slot_ids();
                            let rev = q.revision();
                            let a_sid = q.active_slot_id();
                            drop(q);
                            send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                                status, items, cursor, source: source.clone(),
                                slot_ids: sids, revision: rev, active_slot_id: a_sid,
                            }));
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
                            send_to(request.reply_tx, &CtrlEvent::CommandRejected("stale revision".to_string()));
                            let q = shared_queue.queue.lock().unwrap();
                            let status = player.status.lock().unwrap().clone();
                            let items = q.items_snapshot();
                            let cursor = q.current_index().unwrap_or(0);
                            let sids = q.slot_ids();
                            let rev = q.revision();
                            let a_sid = q.active_slot_id();
                            drop(q);
                            send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                                status, items, cursor, source: source.clone(),
                                slot_ids: sids, revision: rev, active_slot_id: a_sid,
                            }));
                            return;
                        }
                        // Capture progress context from active slot before removal
                        let _progress_ctx = {
                            let q = shared_queue.queue.lock().unwrap();
                            q.active_slot().map(|slot| (slot.progress_state.clone(), slot.slot_id))
                        };
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
    """).lstrip('\n'))
print("daemon_control.rs: complete rewrite done")

# ================================================================
# 3. daemon_tests.rs
# ================================================================
with open(DTEST, "r") as f:
    dt = f.read()

# Fix shared_queue_state helper
old_sq = """fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        items: Arc::new(Mutex::new(Vec::new())),
        cursor: Arc::new(Mutex::new(0)),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
    }
}"""
new_sq = """fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        queue: Arc::new(Mutex::new(crate::playback_queue::PlaybackQueue::default())),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
    }
}"""
assert old_sq in dt, f"shared_queue_state helper not found. Found: {old_sq[:50]}..."
dt = dt.replace(old_sq, new_sq)

# Remove &mut items, &mut cursor, from handle_ctrl calls
dt = dt.replace("&mut items,\n                    &mut cursor,\n", "")
dt = dt.replace("&mut items,\n                        &mut cursor,\n", "")
dt = dt.replace("&mut items,\n                &mut cursor,\n", "")
dt = dt.replace("&mut items,\n            &mut cursor,\n", "")
dt = dt.replace("&mut items, &mut cursor,", "")

# Fix shared_queue.items → shared_queue.queue
dt = dt.replace("shared_queue.items.lock().unwrap()", "shared_queue.queue.lock().unwrap()")
dt = dt.replace("*shared_queue.cursor.lock().unwrap()", "shared_queue.queue.lock().unwrap().current_index().unwrap_or(0)")

# Append v8 tests
dt += """

// === v8 slot-aware command tests (tasks 4.1-4.6) ===========================

#[test]
fn v8_queue_remove_by_slot_revision_mismatch_rejects_and_resyncs() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    shared_queue.queue.lock().unwrap().append(item("item-0", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-1", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().set_active_at(1);
    let slot_ids = shared_queue.queue.lock().unwrap().slot_ids();
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let stale_rev = crate::playback_queue::QueueRevision(99);
    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueRemoveBySlot {
            slot_id: slot_ids[0],
            revision: stale_rev,
        }),
        1,
        CtrlRequest { reply_tx: &reply_tx },
        &client, &player, false, &mut source, &shared_queue, &registry,
        &mut PlaybackIntentState::default(), None, &dummy_merged_tx,
    );

    assert_eq!(shared_queue.queue.lock().unwrap().len(), 2);
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => assert!(reason.contains("stale revision"), "{reason}"),
        other => panic!("expected CommandRejected, got: {other:?}"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::State(state) => assert_eq!(state.items.len(), 2),
        other => panic!("expected State resync, got: {other:?}"),
    }
}

#[test]
fn v8_queue_remove_by_slot_succeeds_with_matching_revision() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    shared_queue.queue.lock().unwrap().append(item("item-0", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-1", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-2", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().set_active_at(1);
    let slot_ids = shared_queue.queue.lock().unwrap().slot_ids();
    let current_rev = shared_queue.queue.lock().unwrap().revision();
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueRemoveBySlot {
            slot_id: slot_ids[1],
            revision: current_rev,
        }),
        1,
        CtrlRequest { reply_tx: &reply_tx },
        &client, &player, false, &mut source, &shared_queue, &registry,
        &mut PlaybackIntentState::default(), None, &dummy_merged_tx,
    );

    assert!(matches!(player_cmd_rx.try_recv(), Ok(PlayerCommand::QueueRemove(_))));
    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        q.items_snapshot().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2"]
    );
    assert!(q.revision().raw() > current_rev.raw());
    drop(q);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => assert_eq!(state.items.len(), 2),
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}

#[test]
fn v8_queue_move_by_slot_updates_authoritative_queue() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    shared_queue.queue.lock().unwrap().append(item("item-0", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-1", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-2", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().set_active_at(1);
    let slot_ids = shared_queue.queue.lock().unwrap().slot_ids();
    let current_rev = shared_queue.queue.lock().unwrap().revision();
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueMoveBySlot {
            slot_id: slot_ids[1], to_position: 2, revision: current_rev,
        }),
        1,
        CtrlRequest { reply_tx: &reply_tx },
        &client, &player, false, &mut source, &shared_queue, &registry,
        &mut PlaybackIntentState::default(), None, &dummy_merged_tx,
    );

    assert!(matches!(player_cmd_rx.try_recv(), Ok(PlayerCommand::QueueMove(_, 2))));
    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(
        q.items_snapshot().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2", "item-1"]
    );
    drop(q);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => assert_eq!(state.items.len(), 3),
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}

#[test]
fn v8_jump_to_slot_forwards_correct_index_to_player() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    shared_queue.queue.lock().unwrap().append(item("item-0", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-1", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-2", "Video", "Movie"));
    let slot_ids = shared_queue.queue.lock().unwrap().slot_ids();
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::JumpToSlot { slot_id: slot_ids[2] }),
        1,
        CtrlRequest { reply_tx: &reply_tx },
        &client, &player, false, &mut source, &shared_queue, &registry,
        &mut PlaybackIntentState::default(), None, &dummy_merged_tx,
    );

    assert!(matches!(player_cmd_rx.try_recv(), Ok(PlayerCommand::JumpTo(2))));
}

#[test]
fn v8_queue_remove_active_removes_slot_and_stops_player() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    shared_queue.queue.lock().unwrap().append(item("item-0", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-1", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-2", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().set_active_at(1);
    let current_rev = shared_queue.queue.lock().unwrap().revision();
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueRemoveActive { revision: current_rev }),
        1,
        CtrlRequest { reply_tx: &reply_tx },
        &client, &player, false, &mut source, &shared_queue, &registry,
        &mut PlaybackIntentState::default(), None, &dummy_merged_tx,
    );

    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        q.items_snapshot().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2"]
    );
    assert!(q.active_slot_id().is_none(), "active slot should be cleared after remove-active");
    drop(q);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => assert_eq!(state.items.len(), 2),
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}

#[test]
fn legacy_v7_queue_remove_still_works() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    shared_queue.queue.lock().unwrap().append(item("item-0", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-1", "Video", "Movie"));
    shared_queue.queue.lock().unwrap().append(item("item-2", "Video", "Movie"));
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueRemove(1)),
        1,
        CtrlRequest { reply_tx: &reply_tx },
        &client, &player, false, &mut source, &shared_queue, &registry,
        &mut PlaybackIntentState::default(), None, &dummy_merged_tx,
    );

    assert!(matches!(player_cmd_rx.try_recv(), Ok(PlayerCommand::QueueRemove(1))));
    assert_eq!(shared_queue.queue.lock().unwrap().len(), 2);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => assert_eq!(state.items.len(), 2),
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}
"""

with open(DTEST, "w") as f:
    f.write(dt)
print("daemon_tests.rs: done")

print("\n=== ALL CHANGES APPLIED ===")
