// Shuffle reorders the whole queue in one edit, keeps the active slot first,
// and publishes the result as one snapshot (queue-content-shuffle task 3.1).

use std::collections::HashSet;

use mbv_player::owner_state::StepTarget;
use rand::SeedableRng;

use super::*;
use crate::emby_session::{EmbySessionLoadedItem, EmbySessionOutput};

/// The contract for the forwarded reorder (queue-content-shuffle task 3.3):
/// the run receives exactly one command — a `QueueReorder` carrying the full
/// shuffled order — and no per-slot `QueueMove`, so the reorder applies
/// inside one command handler and no mpv `EndFile` can land against a
/// partially applied order.
fn assert_one_queue_reorder_carries_the_shuffled_order(
    cmd_rx: &mpsc::Receiver<PlayerCommand>,
    after_order: &[mbv_queue::QueueSlotId],
) {
    let mut reorder = None;
    while let Ok(cmd) = cmd_rx.try_recv() {
        match cmd {
            PlayerCommand::QueueReorder(order) => reorder = Some(order),
            PlayerCommand::QueueMove(..) => {
                panic!("a shuffle must not forward per-slot QueueMove commands")
            }
            other => panic!("expected a QueueReorder, got {other:?}"),
        }
    }
    assert_eq!(reorder.as_deref(), Some(after_order));
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
    // A playing queue: the shuffle pins the playing slot first (spec
    // scenario "Shuffle with a playing item").
    player.status.lock().unwrap().active = true;
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

    // The run receives one QueueReorder carrying the shuffled order (task
    // 3.3); the helper owns the full contract.
    let after_order: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    assert_one_queue_reorder_carries_the_shuffled_order(&cmd_rx, &after_order);

    // A Next after the shuffle follows the new order (task 3.3): the owner
    // resolves the step against the reordered queue, so it jumps to the slot
    // now at index 1.
    assert_eq!(
        owner.core.relative_step_target(mbv_ctrl::Direction::Next),
        StepTarget::Jump(after_order[1]),
        "a Next after the shuffle follows the new order"
    );
}

/// Contract (live-test finding, queue-content-shuffle task 6.4): an idle
/// queue keeps its active marker as the resolved-play anchor — an idle queue
/// load and an owner restore both set it — and Shuffle must not pin that
/// anchor first. Every slot reorders while the anchor keeps its slot identity
/// (spec scenario "Shuffle with nothing playing").
#[test]
fn an_idle_shuffle_reorders_every_slot_and_keeps_the_play_anchor() {
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
        0,
    );
    // The Owner's own shuffle rng, seeded (queue-content-shuffle design D1):
    // the dispatch path stays deterministic, and the permutation-level
    // no-pin contract itself lives at the mbv-queue layer
    // (shuffled_slot_order_does_not_pin_an_idle_play_anchor).
    owner.rng = rand::rngs::SmallRng::seed_from_u64(5);
    let anchor = owner.core.queue.slots()[0].slot_id;
    let before_revision = owner.core.queue.revision().raw();
    let before_ids: HashSet<String> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();

    run_queue_cmd(
        CtrlCmd::UnifiedQueueShuffle {
            op: Some(mbv_ctrl::QueueOpId(7)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    // One revision, every slot still present, and the anchor keeps its slot
    // identity without being pinned first.
    assert_eq!(owner.core.queue.revision().raw(), before_revision + 1);
    let after_ids: HashSet<String> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();
    assert_eq!(after_ids, before_ids);
    assert_eq!(owner.core.queue.active_slot_id(), Some(anchor));
    assert_ne!(
        owner.core.queue.slot_index(anchor),
        Some(0),
        "the seeded shuffle does not leave the anchor first"
    );

    // Exactly one client snapshot, and an idle owner publishes no playing row.
    match recv_event(&observer_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert_eq!(state.active_slot, None),
        other => panic!("expected one UnifiedQueueState, got {other:?}"),
    }
    assert!(observer_rx.try_recv().is_err(), "no second snapshot");

    // The op is answered with the applied state through the dispatch path.
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(7),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } => {
            assert_eq!(state.slots.len(), 6);
            assert_eq!(state.active_slot, None);
        }
        other => panic!("expected an applied QueueOpResult, got {other:?}"),
    }

    // The run receives one QueueReorder carrying the applied order (task 3.3).
    let after_order: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    assert_one_queue_reorder_carries_the_shuffled_order(&cmd_rx, &after_order);
}

/// An Emby Session output that holds the Owner's loaded slot but whose
/// observation presents foreign content: media the Owner did not load. The
/// Session presenter presents any observation with `active` set and no queue
/// identity, leaving the queue's own active marker unchanged (design D10).
fn foreign_content_session_output(
    loaded_slot: mbv_queue::QueueSlotId,
) -> crate::cast::PlaybackOutput {
    let mut foreign = crate::emby_session::fake::session("device-1", "session-1");
    foreign.now_playing = Some("Foreign".to_string());
    foreign.now_playing_item_id = Some("foreign-item".to_string());
    let (worker, _worker_rx) = mpsc::channel::<crate::emby_session::worker::EmbySessionJob>();
    crate::cast::PlaybackOutput::EmbySession(Box::new(EmbySessionOutput {
        worker,
        device_id: "device-1".to_string(),
        name: "Living room TV".to_string(),
        session_id: Some("session-1".to_string()),
        playable_media_types: Vec::new(),
        connection: mbv_ctrl::OutputConnection::Connected,
        misses: 0,
        poll_sent: None,
        last_poll: None,
        next_poll_id: 0,
        last_observation: Some(foreign),
        observed_at: Some(std::time::Instant::now()),
        loaded: Some(EmbySessionLoadedItem {
            slot: loaded_slot,
            item_id: "a".to_string(),
            runtime_ticks: 0,
            loaded_at: std::time::Instant::now(),
            seen_playing: true,
            end_handled: false,
            failure_reported: false,
        }),
        queued_next: None,
        pending_pause: false,
        advance: None,
        continuation: None,
    }))
}

/// Contract (PR #957 review): the shuffle pins the queue's active slot first
/// only when the output presents that queue slot. An Emby Session playing
/// foreign content — media the Owner did not load — presents `active` with no
/// queue identity while the queue's active marker still names the idle play
/// anchor, so pinning it would freeze a stale slot at the head: every slot
/// reorders instead (spec scenario "Shuffle with nothing playing").
#[test]
fn a_shuffle_while_a_session_presents_foreign_content_does_not_pin_the_stale_active_slot() {
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
        0,
    );
    let anchor = owner.core.queue.slots()[0].slot_id;
    owner.output = foreign_content_session_output(anchor);
    // The Owner's own shuffle rng, seeded (queue-content-shuffle design D1):
    // the dispatch path stays deterministic, and the presentation-level
    // no-pin decision itself is covered by the playing and idle cases here
    // plus the presentation predicate's own contract.
    owner.rng = rand::rngs::SmallRng::seed_from_u64(5);
    let before_revision = owner.core.queue.revision().raw();
    let before_ids: HashSet<String> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();
    // The presenter sets `active` for any observation, foreign content
    // included (emby-session-owner-output design D10).
    player.status.lock().unwrap().active = true;

    run_queue_cmd(
        CtrlCmd::UnifiedQueueShuffle {
            op: Some(mbv_ctrl::QueueOpId(11)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    // The shuffle applied as one edit, every slot still present, and the
    // stale anchor is not pinned first.
    assert_eq!(owner.core.queue.revision().raw(), before_revision + 1);
    let after_ids: HashSet<String> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();
    assert_eq!(after_ids, before_ids);
    assert_eq!(owner.core.queue.active_slot_id(), Some(anchor));
    assert_ne!(
        owner.core.queue.slot_index(anchor),
        Some(0),
        "the foreign-content presentation must not pin the stale anchor first"
    );

    // Exactly one client snapshot carries the applied order.
    match recv_event(&observer_rx) {
        CtrlEvent::UnifiedQueueState(state) => assert_eq!(state.slots.len(), 6),
        other => panic!("expected one UnifiedQueueState, got {other:?}"),
    }
    assert!(observer_rx.try_recv().is_err(), "no second snapshot");

    // The op is answered with the applied state through the dispatch path.
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(11),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } => assert_eq!(state.slots.len(), 6),
        other => panic!("expected an applied QueueOpResult, got {other:?}"),
    }

    // The run receives one QueueReorder carrying the applied order.
    let after_order: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    assert_one_queue_reorder_carries_the_shuffled_order(&cmd_rx, &after_order);
}

/// Contract (PR #957 review): a shuffle whose computed order already matches
/// the current one edits nothing — the one-slot queue and a random draw that
/// reproduces the order change nothing, so there is no revision, no client
/// snapshot, and no `QueueReorder` to the run — and the op still answers with
/// the unchanged queue (spec scenario "Queue with one slot").
#[test]
fn a_one_slot_shuffle_changes_nothing_and_still_answers_the_op() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _op_client_rx) = connect_client(&mut registry.lock().unwrap());
    let (_observer_id, observer_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    let before_revision = owner.core.queue.revision().raw();
    let before_ids: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();

    run_queue_cmd(
        CtrlCmd::UnifiedQueueShuffle {
            op: Some(mbv_ctrl::QueueOpId(42)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    // No edit: same revision, same single slot.
    assert_eq!(owner.core.queue.revision().raw(), before_revision);
    let after_ids: Vec<_> = owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect();
    assert_eq!(after_ids, before_ids);

    // No snapshot, and no reorder forwarded to the run.
    assert!(
        observer_rx.try_recv().is_err(),
        "no snapshot for a no-op shuffle"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "no reorder forwarded for a no-op shuffle"
    );

    // The op answers with the applied — unchanged — queue state.
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(42),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(state),
        } => assert_eq!(state.slots.len(), 1),
        other => panic!("expected an applied QueueOpResult, got {other:?}"),
    }
}
