//! The start-slot rule of the `named-queues` spec, "Each queue has a start
//! slot": one table row per outcome the requirement names.

use super::*;

/// The observation a table row applies, in terms of slot positions in the
/// four-slot queue the row starts from.
#[derive(Clone, Copy)]
enum Case {
    /// The output reports the slot at this position as active.
    Active(usize),
    /// Playback stops on the item it was playing.
    Stopped,
    /// A natural completion advances to the slot at this position, or ends the
    /// queue when it is `None`.
    Completed(Option<usize>),
    /// The start slot, which sits at this position, is removed.
    StartSlotRemoved(usize),
}

/// One materialized table row: the queue as the observation leaves it, the
/// slot identities it started with, the start slot it held, and the
/// observation itself.
struct Row {
    queue: PlaybackQueue,
    slot_ids: Vec<QueueSlotId>,
    start_slot: Option<QueueSlotId>,
    observation: StartSlotObservation,
}

fn four_slot_queue() -> PlaybackQueue {
    let items = (0..4)
        .map(|n| QueueItem::Emby(Box::new(item(&format!("i{n}")))))
        .collect();
    queue_from_queue_items(items, None)
}

fn row(start_index: usize, case: Case) -> Row {
    let mut queue = four_slot_queue();
    let slot_ids: Vec<QueueSlotId> = queue.slots().iter().map(|slot| slot.slot_id).collect();
    let start_slot = slot_ids.get(start_index).copied();
    let observation = match case {
        Case::Active(index) => StartSlotObservation::Active(slot_at(&slot_ids, index)),
        Case::Stopped => StartSlotObservation::Stopped,
        Case::Completed(next) => StartSlotObservation::Completed {
            next: next.map(|index| slot_at(&slot_ids, index)),
        },
        Case::StartSlotRemoved(index) => {
            queue.remove_slot(slot_at(&slot_ids, index));
            StartSlotObservation::StartSlotRemoved { index }
        }
    };
    Row {
        queue,
        slot_ids,
        start_slot,
        observation,
    }
}

fn slot_at(slot_ids: &[QueueSlotId], index: usize) -> QueueSlotId {
    *slot_ids
        .get(index)
        .expect("the table names a slot of its queue")
}

#[rstest::rstest]
#[case::playing_slot_is_the_start_slot(1, Case::Active(2), Some(2))]
#[case::a_stop_keeps_the_slot(1, Case::Stopped, Some(1))]
#[case::a_completion_moves_to_the_next_slot(1, Case::Completed(Some(2)), Some(2))]
#[case::the_last_slot_completes(1, Case::Completed(None), None)]
#[case::removing_the_start_slot_takes_the_slot_now_there(1, Case::StartSlotRemoved(1), Some(2))]
#[case::removing_the_last_start_slot_leaves_none(3, Case::StartSlotRemoved(3), None)]
fn start_slot_rule_follows_the_observation(
    #[case] start_index: usize,
    #[case] case: Case,
    #[case] expected_index: Option<usize>,
) {
    let row = row(start_index, case);
    let expected = expected_index.and_then(|index| row.slot_ids.get(index).copied());

    assert_eq!(
        start_slot_after(&row.queue, row.start_slot, row.observation),
        expected
    );
}
