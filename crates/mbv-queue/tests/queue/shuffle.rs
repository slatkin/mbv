use std::collections::HashSet;

use rand::SeedableRng;

use super::*;

fn seeded_rng() -> rand::rngs::SmallRng {
    rand::rngs::SmallRng::seed_from_u64(7)
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
fn shuffled_slot_order_puts_the_playing_slot_first() {
    let queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], Some(2));
    let active = queue.active_slot_id().unwrap();
    let expected: HashSet<QueueSlotId> = slot_ids(&queue).into_iter().collect();

    let order = queue.shuffled_slot_order(&mut seeded_rng());

    assert_eq!(order[0], active);
    assert_eq!(order.len(), expected.len());
    assert_eq!(order.into_iter().collect::<HashSet<_>>(), expected);
}

/// The contract: without a playing slot, Shuffle reorders every slot
/// (spec scenario "Shuffle with nothing playing").
#[test]
fn shuffled_slot_order_without_a_playing_slot_shuffles_all_slots() {
    let queue = queue_from_items(vec![item("a"), item("b"), item("c"), item("d")], None);
    let before = slot_ids(&queue);
    let expected: HashSet<QueueSlotId> = before.iter().copied().collect();

    let order = queue.shuffled_slot_order(&mut seeded_rng());

    assert_eq!(order.len(), expected.len());
    assert_eq!(order.iter().copied().collect::<HashSet<_>>(), expected);
    assert_ne!(order, before, "the seeded order is not the queue order");
}

/// The contract: a queue with one slot does not change (spec scenario
/// "Queue with one slot").
#[test]
fn shuffled_slot_order_with_one_slot_keeps_that_slot() {
    let queue = queue_from_items(vec![item("a")], Some(0));

    let order = queue.shuffled_slot_order(&mut seeded_rng());

    assert_eq!(order, slot_ids(&queue));
}
