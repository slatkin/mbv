//! The Queue panel's report contracts (`QueuePanel.test.ts`): the region
//! targets the panel reports in paint order, the report's capacity and
//! container-size shape, and the shared whole-rows rule.

use super::harness::*;
use crate::bridge::layout::{ListId, ListRows, QueueTargetId, RegionTarget};

/// Contract (`QueuePanel.test.ts`): the regions carry the header, slot,
/// grip, actions, and menu-entry ids, menu last — the header's `filter` target
/// leads, the rows follow, the hovered row's chrome trails them, and the
/// menu's entries report last.
#[test]
fn the_regions_carry_the_header_slot_grip_actions_and_menu_entry_ids_menu_last() {
    let rows = vec![item(1), item(2)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), Some(row_menu()));

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::Queue(QueueTargetId::Slot(1)),
            RegionTarget::Queue(QueueTargetId::Slot(2)),
            RegionTarget::Queue(QueueTargetId::RowGrip(2)),
            RegionTarget::Queue(QueueTargetId::RowActions(2)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(0)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(1)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(2)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(3)),
            RegionTarget::Queue(QueueTargetId::RowMenuEntry(4)),
        ]
    );
}

/// Contract (`QueuePanel.test.ts`): the report carries the queue list's
/// capacity and the painted regions, with the panel container's size — the
/// shape the core's queue viewport and hit map read (D4, D9). The headless
/// document lays no pixels out, so the capacity is the zero-height list's.
#[test]
fn the_report_carries_the_queue_lists_capacity_shape() {
    // The zero viewport lays the list out at zero height: the capacity is
    // its zero-height value.
    let mut fixture = built_unsized(queue_panel_app);
    fixture.show(queue(vec![untimed(item(7))]), summary(), None);

    let report = fixture.last_report().expect("the paint reports");
    // The capacity is the shared whole-rows rule over the list box's
    // measured height (the zero viewport lays it out small).
    let list_height = {
        let dom = fixture.doc.inner.borrow();
        let id = dom
            .query_selector("[data-queue-list]")
            .expect("the selector is valid")
            .expect("the list box paints");
        dom.get_client_bounding_rect(id)
            .map(|rect| crate::window::input::f32_from_f64(rect.height))
            .unwrap_or_default()
    };
    assert_eq!(
        report.lists,
        vec![ListRows {
            list: ListId::Queue,
            whole_rows: super::super::whole_rows(list_height),
        }]
    );
    assert_eq!(
        report
            .regions
            .iter()
            .map(|region| region.target)
            .collect::<Vec<_>>(),
        vec![
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::Queue(QueueTargetId::Slot(7)),
        ]
    );
    assert_eq!(report.container_size.width, 0.0);
    assert_eq!(report.container_size.height, 0.0);
}

/// Contract (task 11.1): the hovered row's grip is laid out where it paints.
/// Blitz lays out without `translate` and applies it at paint only, so a grip
/// centred by `-translate-y-1/2` painted centred while the region the panel
/// reported for `rowGrip` — the region the core's hit map holds — sat half a
/// grip low. The grip centres with an inset, so its laid-out box is the 14 px
/// box it paints, centred in the row.
#[test]
fn the_hover_grip_is_laid_out_where_it_paints() {
    let mut frame = queue(vec![item(1), item(2)]);
    frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, frame, summary(), None);

    let row = fixture.rect_of("[data-row=\"2\"]");
    let grip = fixture.rect_of("[data-grip]");
    assert_eq!(grip.height, 14.0);
    assert_eq!(grip.y - row.y, (row.height - grip.height) / 2.0);
}
