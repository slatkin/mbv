use std::collections::HashSet;

use rand::SeedableRng;

use super::*;

fn seeded_rng() -> rand::rngs::SmallRng {
    rand::rngs::SmallRng::seed_from_u64(1)
}

/// The contract: a reorder replaces the slot sequence in one revision and
/// keeps every slot identity, including the active one (design D2).
#[test]
fn reorder_slots_bumps_revision_once_and_keeps_slot_ids() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], Some(2));
    let before_revision = queue.revision().raw();
    let ids = slot_ids(&queue);
    let active = queue.active_slot_id().unwrap();

    let reordered = vec![ids[2], ids[0], ids[3], ids[1]];
    assert!(matches!(
        queue.reorder_slots(&reordered),
        QueueMutationResult::Applied(())
    ));

    assert_eq!(queue.revision().raw(), before_revision + 1);
    assert_eq!(slot_ids(&queue), reordered);
    assert_eq!(queue.active_slot_id(), Some(active));
    assert_eq!(queue.slot_index(active), Some(0));
}

/// The contract: the Shuffle move list reaches the target with moves that
/// never disturb the positions already placed, and the active slot moves
/// first so a run applying the list holds it at index 0 from the first move
/// on (design D3).
#[test]
fn reorder_move_plan_applies_as_incremental_moves_and_reaches_the_target() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], Some(2));
    let before = slot_ids(&queue);
    let active = queue.active_slot_id().unwrap();
    let target = vec![active, before[3], before[1], before[0]];

    let plan = reorder_move_plan(&before, &target);
    for (slot_id, to) in &plan {
        queue.move_slot(*slot_id, *to);
    }

    assert_eq!(slot_ids(&queue), target);
    assert_eq!(plan[0], (active, 0), "the active slot moves first");
    assert!(
        plan.windows(2).all(|pair| pair[0].1 < pair[1].1),
        "targets go from 0 up, so no move disturbs the placed positions"
    );
}

/// The contract: an order that is not exactly the current slots is rejected
/// and leaves the queue untouched (design D2).
#[test]
fn reorder_slots_with_ids_that_are_not_the_current_slots_leaves_the_queue_untouched() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let before_ids = slot_ids(&queue);
    let before_revision = queue.revision();

    // A repeated id is not a permutation of the current slots.
    let repeated = vec![before_ids[0], before_ids[0], before_ids[1]];
    assert!(matches!(
        queue.reorder_slots(&repeated),
        QueueMutationResult::NotFound
    ));
    assert_eq!(slot_ids(&queue), before_ids);
    assert_eq!(queue.revision(), before_revision);

    // An unknown id is not one of the current slots.
    let foreign = vec![before_ids[0], before_ids[1], QueueSlotId::from_raw(99)];
    assert!(matches!(
        queue.reorder_slots(&foreign),
        QueueMutationResult::NotFound
    ));
    assert_eq!(slot_ids(&queue), before_ids);
    assert_eq!(queue.revision(), before_revision);
}

/// The contract: Shuffle keeps the playing slot first and shuffles the rest
/// (spec scenario "Shuffle with a playing item").
#[test]
fn shuffled_slot_order_pins_the_playing_slot_first_when_playback_is_active() {
    let queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], Some(2));
    let active = queue.active_slot_id().unwrap();
    let expected: HashSet<QueueSlotId> = slot_ids(&queue).into_iter().collect();

    let order = queue.shuffled_slot_order(&mut seeded_rng(), true);

    assert_eq!(order[0], active);
    assert_eq!(order.len(), expected.len());
    assert_eq!(order.into_iter().collect::<HashSet<_>>(), expected);
}

/// The contract: with no active slot, Shuffle reorders every slot
/// (spec scenario "Shuffle with nothing playing").
#[test]
fn shuffled_slot_order_without_an_active_slot_shuffles_all_slots() {
    let queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], None);
    let before = slot_ids(&queue);
    let expected: HashSet<QueueSlotId> = before.iter().copied().collect();

    let order = queue.shuffled_slot_order(&mut seeded_rng(), false);

    assert_eq!(order.len(), expected.len());
    assert_eq!(order.iter().copied().collect::<HashSet<_>>(), expected);
    assert_ne!(order, before, "the seeded order is not the queue order");
}

/// The contract: an idle queue keeps its active marker as the resolved-play
/// anchor (idle load and owner restore set it), and Shuffle must not pin that
/// anchor first — every slot reorders, and the anchor keeps its slot identity
/// (spec scenario "Shuffle with nothing playing"; live-test finding,
/// queue-content-shuffle task 6.4).
#[test]
fn shuffled_slot_order_does_not_pin_an_idle_play_anchor() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], Some(0));
    let anchor = queue.active_slot_id().unwrap();
    let before = slot_ids(&queue);
    let expected: HashSet<QueueSlotId> = before.iter().copied().collect();

    let order = queue.shuffled_slot_order(&mut seeded_rng(), false);
    assert!(matches!(
        queue.reorder_slots(&order),
        QueueMutationResult::Applied(())
    ));

    assert_eq!(order.iter().copied().collect::<HashSet<_>>(), expected);
    assert_eq!(queue.active_slot_id(), Some(anchor));
    assert_ne!(
        queue.slot_index(anchor),
        Some(0),
        "the seeded shuffle does not pin the anchor"
    );
}

/// The contract: an order that already matches the current sequence is no
/// edit — the queue applies nothing and keeps the revision, the same no-change
/// rule `update_slot_item` and `apply_progress` apply (spec scenario "Queue
/// with one slot"; PR #957 review).
#[test]
fn reorder_slots_with_the_current_order_keeps_the_revision() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let before_revision = queue.revision().raw();
    let ids = slot_ids(&queue);

    assert!(matches!(
        queue.reorder_slots(&ids),
        QueueMutationResult::Applied(())
    ));

    assert_eq!(queue.revision().raw(), before_revision);
    assert_eq!(slot_ids(&queue), ids);
}

/// The contract: a queue with one slot does not change (spec scenario
/// "Queue with one slot").
#[test]
fn shuffled_slot_order_with_one_slot_keeps_that_slot() {
    let queue = queue_from_items(vec![item("a")], Some(0));

    let order = queue.shuffled_slot_order(&mut seeded_rng(), true);

    assert_eq!(order, slot_ids(&queue));
}
