//! The Queue panel's report contracts (`QueuePanel.test.ts`): the region
//! targets the panel reports in paint order, the report's capacity and
//! container-size shape, and the shared whole-rows rule.

use super::harness::*;
use crate::bridge::layout::{ListId, ListRows, QueueTargetId, RegionTarget};

/// Contract (`QueuePanel.test.ts`): the report carries the queue list's
/// capacity and the painted regions, with the panel container's size — the
/// shape the core's queue viewport and hit map read (D4, D9). The headless
/// document lays no pixels out, so the capacity is the zero-height list's.
#[test]
fn the_report_carries_the_queue_lists_capacity_shape() {
    // The zero viewport lays the list out at zero height: the capacity is
    // its zero-height value.
    let mut fixture = built_unsized(queue_panel_app);
    fixture.show(queue(vec![untimed(ending(item(7), None))]), summary(), None);

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
        vec![RegionTarget::Queue(QueueTargetId::Slot(7))]
    );
    assert_eq!(report.container_size.width, 0.0);
    assert_eq!(report.container_size.height, 0.0);
}
