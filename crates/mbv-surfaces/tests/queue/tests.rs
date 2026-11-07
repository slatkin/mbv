//! Public-API tests for the Queue Surface state's row drag
//! (add-tauri-gui-client row 6.5): the drag resolves its source and drop
//! indexes against the live rows, so a queue mutation under the drag cannot
//! poison the move request with a start-time index.

use mbv_queue::QueueSlotId;
use mbv_surfaces::queue::{QueueRowInsertionSide, QueueSurface};
use mbv_ui_model::media_list::{MediaKind, MediaListRow, MediaSemanticState};

/// One item row for `slot`.
fn row(slot: u64) -> MediaListRow<QueueSlotId> {
    MediaListRow::Item {
        target: QueueSlotId::from_raw(slot),
        primary: format!("Item {slot}"),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// A queue holding the rows named by `slots`, in order.
fn queue(slots: &[u64]) -> QueueSurface {
    let mut queue = QueueSurface::new();
    queue
        .list_mut()
        .set_content(slots.iter().copied().map(row).collect());
    queue
}

/// A queue mutation under a drag (review of add-tauri-gui-client row 6.5,
/// finding P2-1): after the dragged slot moves below its insertion anchor,
/// the next preview resolves the source's current index, so the stale start
/// index no longer fabricates a move.
#[test]
fn a_drag_reindexes_its_source_after_the_queue_reorders_review_u65_p2_1() {
    let mut queue = queue(&[1, 2, 3, 4]);
    assert!(queue.begin_row_drag(QueueSlotId::from_raw(1)));
    queue.preview_row_drag_drop(QueueSlotId::from_raw(2), QueueRowInsertionSide::After);
    assert_eq!(queue.row_drag().expect("the drag holds").drop_index, 1);

    // Slot 2 moves above slot 1 while the drag runs: slot 1 now sits after
    // the insertion anchor, so the drop lands where it already is.
    queue
        .list_mut()
        .set_content(vec![row(2), row(1), row(3), row(4)]);
    queue.preview_row_drag_drop(QueueSlotId::from_raw(2), QueueRowInsertionSide::After);

    let drag = queue.row_drag().expect("the drag still holds");
    assert_eq!(drag.from_index, 1, "the source index follows the slot");
    assert_eq!(drag.drop_index, 1, "the drop keeps the row in place");
}

/// A vanished dragged slot (review of add-tauri-gui-client row 6.5, finding
/// P2-1): the drag drops on the next preview and the release hands out no
/// request.
#[test]
fn a_drag_drops_when_its_slot_vanishes_review_u65_p2_1() {
    let mut queue = queue(&[1, 2, 3]);
    assert!(queue.begin_row_drag(QueueSlotId::from_raw(2)));
    queue.preview_row_drag_drop(QueueSlotId::from_raw(1), QueueRowInsertionSide::After);

    queue.list_mut().set_content(vec![row(1), row(3)]);
    queue.preview_row_drag_drop(QueueSlotId::from_raw(3), QueueRowInsertionSide::Before);

    assert!(
        queue.row_drag().is_none(),
        "the vanished slot ends the drag"
    );
    assert!(queue.take_row_drag().is_none(), "the release sends no move");
}
