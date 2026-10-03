use super::*;

#[test]
fn progress_applies_to_intended_slot_after_index_shifts() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(2));
    let target = queue.slots()[2].slot_id;
    let removed = queue.slots()[0].slot_id;

    assert!(matches!(
        queue.remove_slot(removed),
        RemoveSlotResult::Removed(_)
    ));
    assert!(matches!(
        queue.apply_progress(target, 12 * TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));

    assert_eq!(
        queue.slot(target).unwrap().item.playback_position_ticks(),
        12 * TICKS_PER_SECOND
    );
}

#[test]
fn progress_for_removed_slot_is_rejected() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(1));
    let removed = queue.slots()[0].slot_id;
    assert!(matches!(
        queue.remove_slot(removed),
        RemoveSlotResult::Removed(_)
    ));

    assert!(matches!(
        queue.apply_progress(removed, 12 * TICKS_PER_SECOND, false),
        QueueMutationResult::NotFound
    ));
}

#[test]
fn active_slot_progress_is_protected_from_server_refresh() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let active = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.apply_progress(active, 20 * TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));

    let result = queue.merge_refresh(vec![
        item_with_progress("a", 3, false),
        item_with_progress("b", 4, false),
    ]);

    assert!(result.protected_slots.contains(&active));
    assert_eq!(
        queue.slot(active).unwrap().item.playback_position_ticks(),
        20 * TICKS_PER_SECOND
    );
}

#[test]
fn refresh_applies_one_fetched_item_to_duplicate_queue_slots() {
    let mut queue = queue_from_items(vec![item("same"), item("same")], Some(0));
    let duplicate = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![item_with_progress("same", 5, false)]);

    assert_eq!(result.pruned_slots, [] as [QueueSlotId; 0]);
    assert!(queue.slot(duplicate).is_some());
    assert_eq!(
        queue
            .slot(duplicate)
            .unwrap()
            .item
            .playback_position_ticks(),
        5 * TICKS_PER_SECOND
    );
}

#[test]
fn refresh_matches_duplicate_fetched_items_in_queue_order() {
    let mut queue = queue_from_items(vec![item("same"), item("same")], None);
    let first = queue.slots()[0].slot_id;
    let second = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![
        item_with_progress("same", 5, false),
        item_with_progress("same", 9, false),
    ]);

    assert_eq!(result.pruned_slots, [] as [QueueSlotId; 0]);
    assert_eq!(
        queue.slot(first).unwrap().item.playback_position_ticks(),
        5 * TICKS_PER_SECOND
    );
    assert_eq!(
        queue.slot(second).unwrap().item.playback_position_ticks(),
        9 * TICKS_PER_SECOND
    );
}

#[test]
fn refresh_prunes_inactive_missing_slots() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let pruned = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![item("a"), item("c")]);

    assert_eq!(result.pruned_slots, vec![pruned]);
    assert!(queue.slot(pruned).is_none());
    assert_eq!(queue.slots().len(), 2);
}

#[test]
fn refresh_cannot_prune_active_slot() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let active = queue.slots()[0].slot_id;
    let inactive = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![item("c")]);

    assert!(result.protected_slots.contains(&active));
    assert_eq!(result.pruned_slots, vec![inactive]);
    assert!(queue.slot(active).is_some());
    assert!(queue.slot(inactive).is_none());
}
