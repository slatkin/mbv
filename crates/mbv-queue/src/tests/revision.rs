use super::*;

#[test]
fn same_owner_queue_replacements_mint_distinct_revisions() {
    let mint = std::sync::Arc::new(crate::QueueRevisionMint::default());
    let first = PlaybackQueue::from_queue_items(Vec::new(), None, std::sync::Arc::clone(&mint));
    let second = PlaybackQueue::from_queue_items(Vec::new(), None, mint);

    assert_ne!(first.revision(), second.revision());
}

#[test]
fn structural_mutations_bump_revision() {
    let mut queue = queue_from_items(vec![item("a"), item("b")], Some(0));
    let initial = queue.revision();

    let inserted = queue.append(QueueItem::Emby(Box::new(item("c"))));
    assert!(queue.revision() > initial);
    let after_insert = queue.revision();

    assert!(matches!(
        queue.move_slot(inserted, 0),
        QueueMutationResult::Applied(())
    ));
    assert!(queue.revision() > after_insert);
    let after_move = queue.revision();

    assert!(matches!(
        queue.consume_slot(inserted),
        QueueMutationResult::Applied(_)
    ));
    assert!(queue.revision() > after_move);
}
