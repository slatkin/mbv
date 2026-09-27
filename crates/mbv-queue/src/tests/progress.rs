use super::*;

#[test]
fn progress_applies_to_intended_slot_after_index_shifts() {
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(2));
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
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(1));
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
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
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
    let mut queue = PlaybackQueue::from_items(vec![item("same"), item("same")], Some(0));
    let duplicate = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![item_with_progress("same", 5, false)]);

    assert!(result.pruned_slots.is_empty());
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
    let mut queue = PlaybackQueue::from_items(vec![item("same"), item("same")], None);
    let first = queue.slots()[0].slot_id;
    let second = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![
        item_with_progress("same", 5, false),
        item_with_progress("same", 9, false),
    ]);

    assert!(result.pruned_slots.is_empty());
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
fn pending_progress_sync_blocks_stale_server_userdata() {
    let mut queue = PlaybackQueue::from_items(vec![item("a")], Some(0));
    let slot = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.apply_progress(slot, 20 * TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    assert!(matches!(
        queue.mark_progress_sync_pending(slot),
        QueueMutationResult::Applied(_)
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 2, false)]);

    assert!(result.stale_pending_slots.contains(&slot));
    assert_eq!(
        queue.slot(slot).unwrap().item.playback_position_ticks(),
        20 * TICKS_PER_SECOND
    );
    assert!(queue
        .slot(slot)
        .unwrap()
        .progress_state
        .pending_sync
        .is_some());
}

#[test]
fn active_pending_progress_confirmation_clears_pending_but_keeps_local_progress() {
    let mut queue = PlaybackQueue::from_items(vec![item("a")], Some(0));
    let active = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.apply_progress(active, 20 * TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    assert!(matches!(
        queue.mark_progress_sync_pending(active),
        QueueMutationResult::Applied(_)
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 22, false)]);

    assert!(result.pending_confirmed_slots.contains(&active));
    assert!(result.protected_slots.contains(&active));
    assert!(queue
        .slot(active)
        .unwrap()
        .progress_state
        .pending_sync
        .is_none());
    assert_eq!(
        queue.slot(active).unwrap().item.playback_position_ticks(),
        20 * TICKS_PER_SECOND
    );
}

#[test]
fn pending_progress_sync_clears_when_server_position_matches_within_tolerance() {
    let mut queue = PlaybackQueue::from_items(vec![item("a")], None);
    let slot = queue.slots()[0].slot_id;
    assert!(matches!(
        queue.apply_progress(slot, 20 * TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    assert!(matches!(
        queue.mark_progress_sync_pending(slot),
        QueueMutationResult::Applied(_)
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 22, false)]);

    assert!(result.pending_confirmed_slots.contains(&slot));
    assert!(queue
        .slot(slot)
        .unwrap()
        .progress_state
        .pending_sync
        .is_none());
    assert_eq!(
        queue.slot(slot).unwrap().item.playback_position_ticks(),
        22 * TICKS_PER_SECOND
    );
}

#[test]
fn watched_state_confirmation_requires_exact_match() {
    let mut queue = PlaybackQueue::from_items(vec![item("a")], Some(0));
    let slot = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.apply_progress(slot, 20 * TICKS_PER_SECOND, true),
        QueueMutationResult::Applied(())
    ));
    assert!(matches!(
        queue.mark_progress_sync_pending(slot),
        QueueMutationResult::Applied(_)
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 20, false)]);

    assert!(result.stale_pending_slots.contains(&slot));
    assert!(queue
        .slot(slot)
        .unwrap()
        .progress_state
        .pending_sync
        .is_some());
    assert!(queue.slot(slot).unwrap().item.played());
}

#[test]
fn refresh_prunes_inactive_non_pending_missing_slots() {
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let pruned = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![item("a"), item("c")]);

    assert_eq!(result.pruned_slots, vec![pruned]);
    assert!(queue.slot(pruned).is_none());
    assert_eq!(queue.slots().len(), 2);
}

#[test]
fn refresh_cannot_prune_active_or_pending_sync_slots() {
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let active = queue.slots()[0].slot_id;
    let pending = queue.slots()[1].slot_id;
    assert!(matches!(
        queue.apply_progress(pending, 9 * TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    assert!(matches!(
        queue.mark_progress_sync_pending(pending),
        QueueMutationResult::Applied(_)
    ));

    let result = queue.merge_refresh(vec![item("c")]);

    assert!(result.protected_slots.contains(&active));
    assert!(result.protected_slots.contains(&pending));
    assert!(queue.slot(active).is_some());
    assert!(queue.slot(pending).is_some());
}

#[test]
fn active_slot_removal_requires_confirmation_decision() {
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
    let active = queue.active_slot_id().unwrap();

    assert!(matches!(
        queue.remove_slot(active),
        RemoveSlotResult::RequiresActiveConfirmation(slot_id) if slot_id == active
    ));
    assert!(queue.slot(active).is_some());
}

#[test]
fn confirmed_active_slot_removal_clears_active_identity() {
    let mut queue = PlaybackQueue::from_items(vec![item("a"), item("b")], Some(0));
    let active = queue.active_slot_id().unwrap();

    assert!(matches!(
        queue.remove_active_slot_confirmed(active),
        RemoveSlotResult::Removed(_)
    ));

    assert!(queue.slot(active).is_none());
    assert_eq!(queue.active_slot_id(), None);
}

#[test]
fn projected_row_mutation_matrix_tracks_revision_without_noop_bumps() {
    let mut queue = PlaybackQueue::from_queue_items_with_revision(
        vec![
            QueueItem::Emby(Box::new(item("a"))),
            QueueItem::Emby(Box::new(item("b"))),
        ],
        Some(0),
        QueueRevision::from_raw(40),
    );
    let first = queue.slots()[0].slot_id;
    let second = queue.slots()[1].slot_id;

    let before = queue.revision();
    assert!(matches!(
        queue.apply_progress(first, 0, false),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), before);

    assert!(matches!(
        queue.apply_progress(first, TICKS_PER_SECOND, false),
        QueueMutationResult::Applied(())
    ));
    let after_progress = queue.revision();
    assert!(after_progress > before);

    assert!(matches!(
        queue.update_slot_item(
            first,
            QueueItem::Emby(Box::new(item_with_progress("a", 1, false,)))
        ),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), after_progress);

    assert!(matches!(
        queue.set_active_slot(second),
        QueueMutationResult::Applied(())
    ));
    let after_active = queue.revision();
    assert!(after_active > after_progress);
    assert!(matches!(
        queue.set_active_slot(second),
        QueueMutationResult::Applied(())
    ));
    assert_eq!(queue.revision(), after_active);

    assert!(matches!(
        queue.mark_progress_sync_pending(first),
        QueueMutationResult::Applied(_)
    ));
    assert_eq!(queue.revision(), after_active);

    let before_refresh = queue.revision();
    let result = queue.merge_refresh(vec![item_with_progress("a", 1, false), item("b")]);
    assert!(result.pruned_slots.is_empty());
    assert_eq!(queue.revision(), before_refresh);

    queue.clear_active_slot();
    assert!(queue.revision() > before_refresh);
}
