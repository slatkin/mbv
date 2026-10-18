// Shuffle reorders the whole queue in one edit, keeps the active slot first,
// and publishes the result as one snapshot (queue-content-shuffle task 3.1).

use std::collections::HashSet;

use super::*;

/// The contract for the forwarded reorder (design D3): the run receives the
/// per-slot `QueueMove` list, the active slot moves first, targets go from 0
/// up, and applying the list reaches exactly the shuffled order.
fn assert_forwarded_moves_reach_the_shuffled_order(
    cmd_rx: &mpsc::Receiver<PlayerCommand>,
    before_order: &[mbv_queue::QueueSlotId],
    after_order: &[mbv_queue::QueueSlotId],
    active: mbv_queue::QueueSlotId,
) {
    let mut moves = Vec::new();
    while let Ok(cmd) = cmd_rx.try_recv() {
        match cmd {
            PlayerCommand::QueueMove(slot_id, to) => moves.push((slot_id, to)),
            other => panic!("expected a QueueMove, got {other:?}"),
        }
    }
    assert_eq!(moves[0], (active, 0), "the active slot moves first");
    assert!(
        moves.windows(2).all(|pair| pair[0].1 < pair[1].1),
        "targets go from 0 up"
    );
    let mut forwarded: Vec<mbv_queue::QueueSlotId> = before_order.to_vec();
    for (slot_id, to) in moves {
        let from = forwarded
            .iter()
            .position(|id| *id == slot_id)
            .expect("a forwarded move names a queue slot");
        forwarded.remove(from);
        forwarded.insert(to, slot_id);
    }
    assert_eq!(forwarded, after_order);
}

#[test]
fn unified_queue_shuffle_reorders_once_with_the_active_slot_first_and_the_same_slots() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _op_client_rx) = connect_client(&mut registry.lock().unwrap());
    let (_observer_id, observer_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
            emby_qi("d", "Video", "Movie"),
            emby_qi("e", "Video", "Movie"),
            emby_qi("f", "Video", "Movie"),
        ],
        2,
    );
    let active = owner.core.queue.slots()[2].slot_id;
    let before_revision = owner.core.queue.revision().raw();
    let before_order: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    let before_slots: HashSet<String> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();

    run_queue_cmd(
        CtrlCmd::UnifiedQueueShuffle {
            op: Some(mbv_ctrl::QueueOpId(51)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    // One revision, active slot first, and the same slots in the edit.
    assert_eq!(owner.core.queue.revision().raw(), before_revision + 1);
    assert_eq!(owner.core.queue.slots()[0].slot_id, active);
    assert_eq!(owner.core.queue.active_slot_id(), Some(active));
    let after_slots: HashSet<String> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();
    assert_eq!(after_slots, before_slots);

    // Exactly one client snapshot, and it holds the whole new order.
    match recv_event(&observer_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(state.slots.len(), 6);
            assert_eq!(state.slots[0].slot_id, mbv_ctrl::slot_id_to_u64(active));
        }
        other => panic!("expected one UnifiedQueueState, got {other:?}"),
    }
    assert!(observer_rx.try_recv().is_err(), "no second snapshot");

    // The op is answered with the applied state.
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(51),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } => {
            assert_eq!(state.slots.len(), 6);
            assert_eq!(state.slots[0].slot_id, mbv_ctrl::slot_id_to_u64(active));
        }
        other => panic!("expected an applied QueueOpResult, got {other:?}"),
    }

    // The run receives the reorder as the per-slot QueueMove list (design
    // D3); the helper owns the full contract.
    let after_order: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    assert_forwarded_moves_reach_the_shuffled_order(&cmd_rx, &before_order, &after_order, active);
}
