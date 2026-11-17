//! The Queue header's contracts (`QueuePanel.test.ts`, the header tests,
//! travelling with the header's port; rewritten for gui-named-queues task
//! 8.2): the two spec scenarios of `gui-queue-chooser` "The queue header holds
//! the chooser and its controls", and the active filter's text and caret.

use blitz_dom::local_name;

use super::harness::*;
use crate::bridge::layout::{QueueTargetId, RegionTarget};

/// Contract (spec `gui-queue-chooser` "One queue and no peer"): with only
/// `Default` in the local Owner and no peer connected, the header paints the
/// filter control and the new-queue control, and no chooser. The close control
/// paints disabled — the only local queue has no close — and claims no pointer,
/// and the header paints neither the remaining count nor the finish clock.
#[test]
fn one_queue_and_no_peer_paints_the_filter_and_new_controls_and_no_chooser() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), summary(), None);

    assert!(
        !fixture.presents("[data-queue-chooser]"),
        "no chooser paints"
    );
    assert_eq!(
        fixture
            .attr_of("[data-filter]", local_name!("aria-label"))
            .as_deref(),
        Some("Filter queue")
    );
    assert_eq!(
        fixture
            .attr_of("[data-new-queue]", local_name!("aria-label"))
            .as_deref(),
        Some("New queue")
    );
    assert_eq!(
        fixture
            .attr_of("[data-close-queue]", local_name!("aria-disabled"))
            .as_deref(),
        Some("true")
    );
    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::Queue(QueueTargetId::NewQueue),
        ]
    );
    assert!(!fixture.presents("[data-queue-heading]"), "no count paints");
    assert!(!fixture.presents("[data-finish-clock]"), "no clock paints");
}

/// Contract (spec `gui-queue-chooser` "Several entries"): with `Default` and
/// `Kids` in the local Owner, the header paints the chooser with the shown
/// queue's name, the filter control, the new-queue control, and the close
/// control — which now acts, so it reports its region too.
#[test]
fn several_entries_paints_the_chooser_with_the_shown_name_and_the_close_control() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), several_entries(), None);

    assert_eq!(fixture.text_of("[data-queue-chooser]"), "Kids");
    assert_eq!(
        fixture
            .attr_of("[data-queue-chooser]", local_name!("aria-label"))
            .as_deref(),
        Some("Queue chooser")
    );
    assert_eq!(
        fixture
            .attr_of("[data-close-queue]", local_name!("aria-disabled"))
            .as_deref(),
        Some("false")
    );
    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::Queue(QueueTargetId::QueueChooser),
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::Queue(QueueTargetId::NewQueue),
            RegionTarget::Queue(QueueTargetId::CloseQueue),
        ]
    );
}

/// Contract (`QueuePanel.test.ts`): the active filter paints its text and
/// the caret.
#[test]
fn the_active_filter_paints_its_text_and_the_caret() {
    let mut filtered = summary();
    filtered.filter = "amb".to_owned();
    filtered.filter_active = true;
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), filtered, None);

    assert_eq!(fixture.text_of("[data-filter]"), "amb");
    assert!(fixture.presents("[data-caret]"));
}
