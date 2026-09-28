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
    let mut queue = queue_from_items(vec![item("same"), item("same")], None);
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
    let mut queue = queue_from_items(vec![item("a")], Some(0));
    let slot = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.record_reported_progress(
            slot,
            20 * TICKS_PER_SECOND,
            false,
            StopReportOutcome::Accepted,
        ),
        QueueMutationResult::Applied(())
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 2, false)]);

    assert!(result.stale_pending_slots.contains(&slot));
    assert_eq!(
        queue.slot(slot).unwrap().item.playback_position_ticks(),
        20 * TICKS_PER_SECOND
    );
    assert!(queue.slot(slot).unwrap().pending_sync().is_some());
}

#[test]
fn active_pending_progress_confirmation_clears_pending_but_keeps_local_progress() {
    let mut queue = queue_from_items(vec![item("a")], Some(0));
    let active = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.record_reported_progress(
            active,
            20 * TICKS_PER_SECOND,
            false,
            StopReportOutcome::Accepted,
        ),
        QueueMutationResult::Applied(())
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 22, false)]);

    assert!(result.pending_confirmed_slots.contains(&active));
    assert!(result.protected_slots.contains(&active));
    assert!(queue.slot(active).unwrap().pending_sync().is_none());
    assert_eq!(
        queue.slot(active).unwrap().item.playback_position_ticks(),
        20 * TICKS_PER_SECOND
    );
}

#[test]
fn pending_progress_sync_clears_when_server_position_matches_within_tolerance() {
    let mut queue = queue_from_items(vec![item("a")], None);
    let slot = queue.slots()[0].slot_id;
    assert!(matches!(
        queue.record_reported_progress(
            slot,
            20 * TICKS_PER_SECOND,
            false,
            StopReportOutcome::Accepted,
        ),
        QueueMutationResult::Applied(())
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 22, false)]);

    assert!(result.pending_confirmed_slots.contains(&slot));
    assert!(queue.slot(slot).unwrap().pending_sync().is_none());
    assert_eq!(
        queue.slot(slot).unwrap().item.playback_position_ticks(),
        22 * TICKS_PER_SECOND
    );
}

#[test]
fn watched_state_confirmation_requires_exact_match() {
    let mut queue = queue_from_items(vec![item("a")], Some(0));
    let slot = queue.active_slot_id().unwrap();
    assert!(matches!(
        queue.record_reported_progress(
            slot,
            20 * TICKS_PER_SECOND,
            true,
            StopReportOutcome::Accepted,
        ),
        QueueMutationResult::Applied(())
    ));

    let result = queue.merge_refresh(vec![item_with_progress("a", 20, false)]);

    assert!(result.stale_pending_slots.contains(&slot));
    assert!(queue.slot(slot).unwrap().pending_sync().is_some());
    assert!(queue.slot(slot).unwrap().item.played());
}

#[test]
fn refresh_prunes_inactive_non_pending_missing_slots() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let pruned = queue.slots()[1].slot_id;

    let result = queue.merge_refresh(vec![item("a"), item("c")]);

    assert_eq!(result.pruned_slots, vec![pruned]);
    assert!(queue.slot(pruned).is_none());
    assert_eq!(queue.slots().len(), 2);
}

#[test]
fn refresh_cannot_prune_active_or_pending_sync_slots() {
    let mut queue = queue_from_items(vec![item("a"), item("b"), item("c")], Some(0));
    let active = queue.slots()[0].slot_id;
    let pending = queue.slots()[1].slot_id;
    assert!(matches!(
        queue.record_reported_progress(
            pending,
            9 * TICKS_PER_SECOND,
            false,
            StopReportOutcome::Accepted,
        ),
        QueueMutationResult::Applied(())
    ));

    let result = queue.merge_refresh(vec![item("c")]);

    assert!(result.protected_slots.contains(&active));
    assert!(result.protected_slots.contains(&pending));
    assert!(queue.slot(active).is_some());
    assert!(queue.slot(pending).is_some());
}

#[rstest::rstest]
#[case::emby_accepted(
    QueueItem::Emby(Box::new(item("emby"))),
    StopReportOutcome::Accepted,
    true
)]
#[case::emby_rejected(
    QueueItem::Emby(Box::new(item("emby"))),
    StopReportOutcome::NotAccepted,
    false
)]
#[case::feed_accepted(QueueItem::Feed(feed("feed")), StopReportOutcome::Accepted, false)]
#[case::abs_episode_accepted(
    QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(audiobookshelf_episode(
        "lib", "episode"
    ))),
    StopReportOutcome::Accepted,
    false
)]
fn record_reported_progress_arms_only_accepted_emby_reports(
    #[case] item: QueueItem,
    #[case] outcome: StopReportOutcome,
    #[case] should_arm: bool,
) {
    let mut queue = queue_from_queue_items(vec![item], None);
    let slot_id = queue.slots()[0].slot_id;
    assert!(matches!(
        queue.record_reported_progress(slot_id, 42, true, outcome),
        QueueMutationResult::Applied(())
    ));

    let slot = queue.slot(slot_id).unwrap();
    let expected = should_arm.then_some(SlotProgress {
        position_ticks: 42,
        played: true,
    });
    assert_eq!(slot.pending_sync(), expected);
    assert_eq!(
        slot.local_progress(),
        SlotProgress {
            position_ticks: 42,
            played: true,
        }
    );
}

#[rstest::rstest]
#[case::same_content(true)]
#[case::changed_content(false)]
fn update_slot_item_keeps_protection_only_for_same_content(#[case] same_content: bool) {
    let mut queue = queue_from_items(vec![item("a")], None);
    let slot_id = queue.slots()[0].slot_id;
    let _ = queue.record_reported_progress(slot_id, 42, false, StopReportOutcome::Accepted);
    let replacement_id = if same_content { "a" } else { "b" };

    let _ = queue.update_slot_item(slot_id, QueueItem::Emby(Box::new(item(replacement_id))));

    assert_eq!(
        queue.slot(slot_id).unwrap().pending_sync().is_some(),
        same_content
    );
}
