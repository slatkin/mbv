#!/usr/bin/env bash
set -euo pipefail

ROOT="/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model"
DCORE="$ROOT/crates/mbv-core/src/daemon_core.rs"
DCTRL="$ROOT/crates/mbv-core/src/daemon_control.rs"
DTEST="$ROOT/crates/mbv-core/src/daemon_tests.rs"

echo "=== Phase 4: daemon mutation and broadcast behavior ==="

# ── Step 1: daemon_core.rs ──
# 1a. Add peer_version to CtrlClient struct
sed -i '/^struct CtrlClient {$/,/^}$/{
/    tx: CtrlSender,$/a\    peer_version: u32,
}' "$DCORE"

# 1b. Update connect() signature to accept peer_version
sed -i 's/fn connect(&mut self, tx: CtrlSender) -> CtrlClientId {/fn connect(&mut self, tx: CtrlSender, peer_version: u32) -> CtrlClientId {/' "$DCORE"

# 1c. Add peer_version to CtrlClient construction in connect()
sed -i 's/self\.connection\.push(CtrlClient { id, tx });/self.connection.push(CtrlClient { id, tx, peer_version });/' "$DCORE"

# 1d. Capture peer_version in spawn_ctrl_client before connect() call
sed -i '/let reply_tx = ev_tx.clone();/i\        let peer_version = info.protocol_version;' "$DCORE"

# 1e. Update connect() call in spawn_ctrl_client
sed -i 's/ctrl_clients\.lock()\.unwrap()\.connect(ev_tx);/ctrl_clients.lock().unwrap().connect(ev_tx, peer_version);/' "$DCORE"

# 1f. Update initial state to be version-aware
# Replace the entire init state block
python3 << 'PYEOF'
import re

with open("/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_core.rs", "r") as f:
    content = f.read()

old = '''        let (init_items, init_cursor) = {
            let q = shared_queue.queue.lock().unwrap();
            (q.items_snapshot(), q.current_index().unwrap_or(0))
        };
        if let Ok(init_json) = serde_json::to_string(&CtrlEvent::State(CtrlState::v7(
            player_status.lock().unwrap().clone(),
            init_items,
            init_cursor,
            shared_queue.source.lock().unwrap().clone(),
        ))) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }'''

new = '''        let (init_items, init_cursor, init_slot_ids, init_revision, init_active_slot_id) = {
            let q = shared_queue.queue.lock().unwrap();
            (q.items_snapshot(), q.current_index().unwrap_or(0), q.slot_ids(), q.revision(), q.active_slot_id())
        };
        let init_status = player_status.lock().unwrap().clone();
        let init_source = shared_queue.source.lock().unwrap().clone();
        let init_event = if peer_version >= 8 {
            CtrlEvent::State(CtrlState {
                status: init_status,
                items: init_items,
                cursor: init_cursor,
                source: init_source,
                slot_ids: init_slot_ids,
                revision: init_revision,
                active_slot_id: init_active_slot_id,
            })
        } else {
            CtrlEvent::State(CtrlState::v7(
                init_status,
                init_items,
                init_cursor,
                init_source,
            ))
        };
        if let Ok(init_json) = serde_json::to_string(&init_event) {
            ev_tx.send(CtrlOutbound::Event(init_json)).ok();
        }'''

assert old in content, "Could not find initial state block in daemon_core.rs"
content = content.replace(old, new)

with open("/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_core.rs", "w") as f:
    f.write(content)
print("daemon_core.rs: initial state version-gated")
PYEOF

echo "Step 1 complete: daemon_core.rs"
grep -n "peer_version" "$DCORE" | head -15

# ── Step 2: daemon_control.rs ──
echo ""
echo "=== Step 2: daemon_control.rs ==="

# 2a. Replace broadcast_queue_state with version-aware per-client broadcast
python3 << 'PYEOF'
with open("/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_control.rs", "r") as f:
    content = f.read()

old = '''fn broadcast_queue_state(
    ctrl_clients: &ClientRegistry,
    player: &Player,
    shared_queue: &SharedQueueState,
    source: &crate::config::QueueSource,
) {
    let q = shared_queue.queue.lock().unwrap();
    let event = CtrlEvent::State(CtrlState::v7(
        player.status.lock().unwrap().clone(),
        q.items_snapshot(),
        q.current_index().unwrap_or(0),
        source.clone(),
    ));
    drop(q);
    broadcast(ctrl_clients, &event);
    *shared_queue.source.lock().unwrap() = source.clone();
}'''

new = '''fn broadcast_queue_state(
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
}'''

assert old in content, "Could not find broadcast_queue_state in daemon_control.rs"
content = content.replace(old, new)
print("broadcast_queue_state: replaced with version-aware variant")

with open("/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_control.rs", "w") as f:
    f.write(content)
PYEOF

echo "Step 2a complete: broadcast version-aware"

# 2b. Add v8 command handling in handle_ctrl - intercept before PlayerCommand::from()
python3 << 'PYEOF'
with open("/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_control.rs", "r") as f:
    content = f.read()

old = '        CtrlCmd::PlayerCmd(pc) => match PlayerCommand::from(pc) {'

new = '''        CtrlCmd::PlayerCmd(pc) => {
            // ── v8 slot-aware queue commands (handled before PlayerCommand conversion) ──
            match &pc {
                WireCommand::QueueRemoveBySlot { slot_id, revision } => {
                    let current_rev = shared_queue.queue.lock().unwrap().revision();
                    if *revision != current_rev {
                        log::warn!(target: "daemon", "QueueRemoveBySlot: stale revision (client={}, daemon={})", revision.raw(), current_rev.raw());
                        send_to(request.reply_tx, &CtrlEvent::CommandRejected(
                            "stale revision".to_string(),
                        ));
                        let q = shared_queue.queue.lock().unwrap();
                        let status = player.status.lock().unwrap().clone();
                        let items = q.items_snapshot();
                        let cursor = q.current_index().unwrap_or(0);
                        let slot_ids = q.slot_ids();
                        let rev = q.revision();
                        let a_slot_id = q.active_slot_id();
                        drop(q);
                        send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                            status,
                            items,
                            cursor,
                            source: source.clone(),
                            slot_ids,
                            revision: rev,
                            active_slot_id: a_slot_id,
                        }));
                        return;
                    }
                    let mut q = shared_queue.queue.lock().unwrap();
                    let slot_id_val = *slot_id;
                    let idx = q.slot_index(slot_id_val).unwrap_or(0);
                    match q.remove_slot(slot_id_val) {
                        crate::playback_queue::RemoveSlotResult::Removed(_) => {}
                        crate::playback_queue::RemoveSlotResult::RequiresActiveConfirmation(_) => {
                            let _ = q.remove_active_slot_confirmed(slot_id_val);
                        }
                        crate::playback_queue::RemoveSlotResult::NotFound => {
                            log::warn!(target: "daemon", "QueueRemoveBySlot: slot {:?} not found", slot_id_val);
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
                        log::warn!(target: "daemon", "QueueMoveBySlot: stale revision (client={}, daemon={})", revision.raw(), current_rev.raw());
                        send_to(request.reply_tx, &CtrlEvent::CommandRejected(
                            "stale revision".to_string(),
                        ));
                        let q = shared_queue.queue.lock().unwrap();
                        let status = player.status.lock().unwrap().clone();
                        let items = q.items_snapshot();
                        let cursor = q.current_index().unwrap_or(0);
                        let slot_ids = q.slot_ids();
                        let rev = q.revision();
                        let a_slot_id = q.active_slot_id();
                        drop(q);
                        send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                            status,
                            items,
                            cursor,
                            source: source.clone(),
                            slot_ids,
                            revision: rev,
                            active_slot_id: a_slot_id,
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
                        log::warn!(target: "daemon", "QueueInsertAt: stale revision (client={}, daemon={})", revision.raw(), current_rev.raw());
                        send_to(request.reply_tx, &CtrlEvent::CommandRejected(
                            "stale revision".to_string(),
                        ));
                        let q = shared_queue.queue.lock().unwrap();
                        let status = player.status.lock().unwrap().clone();
                        let items = q.items_snapshot();
                        let cursor = q.current_index().unwrap_or(0);
                        let slot_ids = q.slot_ids();
                        let rev = q.revision();
                        let a_slot_id = q.active_slot_id();
                        drop(q);
                        send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                            status,
                            items,
                            cursor,
                            source: source.clone(),
                            slot_ids,
                            revision: rev,
                            active_slot_id: a_slot_id,
                        }));
                        return;
                    }
                    let mut q = shared_queue.queue.lock().unwrap();
                    let _new_slot_id = q.insert(*position, item.clone());
                    drop(q);
                    broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                    player.send_command(PlayerCommand::QueueAppend { items: vec![item.clone()] });
                    return;
                }
                WireCommand::QueueRemoveActive { revision } => {
                    let current_rev = shared_queue.queue.lock().unwrap().revision();
                    if *revision != current_rev {
                        log::warn!(target: "daemon", "QueueRemoveActive: stale revision (client={}, daemon={})", revision.raw(), current_rev.raw());
                        send_to(request.reply_tx, &CtrlEvent::CommandRejected(
                            "stale revision".to_string(),
                        ));
                        let q = shared_queue.queue.lock().unwrap();
                        let status = player.status.lock().unwrap().clone();
                        let items = q.items_snapshot();
                        let cursor = q.current_index().unwrap_or(0);
                        let slot_ids = q.slot_ids();
                        let rev = q.revision();
                        let a_slot_id = q.active_slot_id();
                        drop(q);
                        send_to(request.reply_tx, &CtrlEvent::State(CtrlState {
                            status,
                            items,
                            cursor,
                            source: source.clone(),
                            slot_ids,
                            revision: rev,
                            active_slot_id: a_slot_id,
                        }));
                        return;
                    }
                    // Capture progress context from active slot before removal
                    let progress_ctx = {
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
                    // Broadcast committed state before async stop
                    broadcast_queue_state(ctrl_clients, player, shared_queue, source);
                    // Dispatch player stop (async)
                    player.stop();
                    log::info!(target: "daemon", "QueueRemoveActive: removed active slot, stopping player");
                    return;
                }
                _ => {
                    // Fall through to legacy v7 PlayerCommand handling below
                }
            }
            match PlayerCommand::from(pc) {'''

assert old in content, "Could not find CtrlCmd::PlayerCmd(pc) => match PlayerCommand::from(pc) { in daemon_control.rs"
content = content.replace(old, new)
print("handle_ctrl: v8 command handling inserted")

with open("/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_control.rs", "w") as f:
    f.write(content)
PYEOF

echo "Step 2b complete: v8 command handling"
grep -c "QueueRemoveBySlot\|QueueMoveBySlot\|QueueRemoveActive" "$DCTRL"

# ── Step 3: daemon_tests.rs ──
echo ""
echo "=== Step 3: daemon_tests.rs ==="

cat >> "$DTEST" << 'TESTEOF'

// ── v8 slot-aware command tests (tasks 4.1-4.6) ──────────────────────────

#[test]
fn v8_queue_remove_by_slot_revision_mismatch_rejects_and_resyncs() {
    let player = cold_player();
    let _player_cmd_rx = player.spy_on_commands();
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
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(q.len(), 2);
    drop(q);

    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert!(reason.contains("stale revision"), "expected stale revision, got: {reason}");
        }
        other => panic!("expected CommandRejected, got: {other:?}"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(state.items.len(), 2);
        }
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
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::QueueRemove(_))
    ));

    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        q.items_snapshot().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2"]
    );
    assert!(q.revision().raw() > current_rev.raw());
    drop(q);

    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(state.items.len(), 2);
        }
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
            slot_id: slot_ids[1],
            to_position: 2,
            revision: current_rev,
        }),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::QueueMove(_, 2))
    ));

    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(
        q.items_snapshot().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2", "item-1"]
    );
    drop(q);

    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(state.items.len(), 3);
        }
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
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::JumpToSlot {
            slot_id: slot_ids[2],
        }),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::JumpTo(2))
    ));
}

#[test]
fn v8_queue_remove_active_removes_slot_and_stops_player() {
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
    let current_rev = shared_queue.queue.lock().unwrap().revision();
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueRemoveActive {
            revision: current_rev,
        }),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
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
        CtrlEvent::State(state) => {
            assert_eq!(state.items.len(), 2);
        }
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}

#[test]
fn v8_queue_insert_at_inserts_item_and_broadcasts() {
    let player = cold_player();
    let _player_cmd_rx = player.spy_on_commands();
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
    let current_rev = shared_queue.queue.lock().unwrap().revision();
    let mut source = QueueSource::Remote;
    let new_item = item("restored", "Video", "Movie");
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::QueueInsertAt {
            item: new_item.clone(),
            position: 1,
            revision: current_rev,
        }),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(q.len(), 3);
    assert_eq!(
        q.items_snapshot().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "restored", "item-1"]
    );
    drop(q);

    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(state.items.len(), 3);
        }
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}

#[test]
fn legacy_v7_queue_remove_still_works_for_v7_peers() {
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
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::QueueRemove(1))
    ));
    let q = shared_queue.queue.lock().unwrap();
    assert_eq!(q.len(), 2);
    drop(q);

    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(state.items.len(), 2);
        }
        other => panic!("expected State broadcast, got: {other:?}"),
    }
}
TESTEOF

echo "Step 3 complete: daemon_tests.rs updated"
wc -l "$DCORE" "$DCTRL" "$DTEST"
echo ""
echo "All source modifications applied."
