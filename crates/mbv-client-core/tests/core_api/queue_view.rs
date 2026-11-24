//! Owner-snapshot adoption reconciles the selected slot by slot identity.

use mbv_client_core::make_items;
use mbv_client_core::state::queue_view::{AdoptCause, QueueView};
use mbv_ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};
use mbv_queue::{QueueItem, QueueLineage, QueueSource};
use rstest::rstest;

fn snapshot(ids: &[u64], active: Option<u64>, lineage: u64) -> UnifiedQueueStateData {
    let items = make_items(ids.len());
    UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus::default(),
        slots: ids
            .iter()
            .zip(items)
            .map(|(&slot_id, item)| UnifiedQueueSlot {
                slot_id,
                item: QueueItem::Emby(Box::new(item)),
            })
            .collect(),
        active_slot: active,
        revision: 1,
        source: QueueSource::Remote,
        lineage: QueueLineage(lineage),
        in_flight_transition: None,
        queued_latest_transition: None,
    }
}

#[rstest]
#[case::move_keeps_selected_slot(&[1, 2, 3], 1, &[1, 3, 2], None, 7, AdoptCause::OwnAnswer, 2)]
#[case::removal_selects_next(&[1, 2, 3], 1, &[1, 3], None, 7, AdoptCause::OwnAnswer, 1)]
#[case::removing_last_selects_new_last(&[1, 2, 3], 2, &[1, 2], None, 7, AdoptCause::OwnAnswer, 1)]
#[case::background_follows_active(&[1, 2, 3], 0, &[1, 2, 3], Some(3), 7, AdoptCause::Background { held: false }, 2)]
#[case::background_held_keeps_selection(&[1, 2, 3], 1, &[3, 2, 1], Some(3), 7, AdoptCause::Background { held: true }, 1)]
#[case::replacement_selects_start(&[1, 2, 3], 1, &[8, 9], Some(9), 7, AdoptCause::Replacement, 0)]
fn adoption_reconciles_selection_by_slot(
    #[case] old_ids: &[u64],
    #[case] old_cursor: usize,
    #[case] new_ids: &[u64],
    #[case] active: Option<u64>,
    #[case] new_lineage: u64,
    #[case] cause: AdoptCause,
    #[case] expected_cursor: usize,
) {
    let mut view = QueueView::from_snapshot(&snapshot(old_ids, Some(old_ids[0]), 7));
    view.set_cursor(old_cursor);

    view.adopt(&snapshot(new_ids, active, new_lineage), cause);

    assert_eq!(view.cursor(), expected_cursor);
}

#[test]
fn every_adoption_mints_a_new_revision() {
    let state = snapshot(&[1, 2], Some(1), 7);
    let mut view = QueueView::from_snapshot(&state);
    let first = view.revision();

    view.adopt(&state, AdoptCause::OwnAnswer);
    let second = view.revision();
    view.adopt(&state, AdoptCause::Background { held: true });

    assert!(second > first);
    assert!(view.revision() > second);
}
