//! The Queue header's contracts (`QueuePanel.test.ts`, the header tests,
//! travelling with the header's port): the canvas heading, the finish clock,
//! and the filter paint, and the active filter's text and caret.

use blitz_dom::local_name;

use super::harness::*;
use crate::window::time::format_wall_clock;

/// Contract (`QueuePanel.test.ts`): the header paints the canvas heading, the
/// finish clock, and the filter — the `Up next · 2` heading in its count type,
/// the `queue ends HH:MM` clock in the summary's tail past the transport's
/// finish, and the filter as the canvas's icon-only magnifier button at rest.
#[test]
fn the_header_paints_the_canvas_heading_the_finish_clock_and_the_filter() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), summary(), None);

    assert_eq!(fixture.text_of("[data-queue-heading]"), "Up next · 2");
    // The queue's finish is the transport's finish (the wall clock plus the
    // active item's remaining 140 s) plus the tail's 43 minutes. The
    // expectation derives from the same fixed clock with the shared
    // formatter, so the assertion holds in every zone.
    let expected_finish = format_wall_clock(WALL_MS.div_euclid(1000) + 140 + 43 * 60);
    assert_eq!(
        fixture.text_of("[data-finish-clock]"),
        format!("queue ends {expected_finish}")
    );
    assert_eq!(
        fixture
            .attr_of("[data-filter]", local_name!("aria-label"))
            .as_deref(),
        Some("Filter queue")
    );
    assert!(
        fixture.presents("[data-filter] svg"),
        "the magnifier paints"
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
