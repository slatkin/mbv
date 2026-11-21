//! The Queue header's contracts (`QueuePanel.test.ts`, the header tests,
//! travelling with the header's port; rewritten for design D7 of
//! `gui-queue-panel-redesign`): the spec scenarios of `gui-queue-chooser` "The
//! queue header holds the chooser and its controls", the bar's order and its
//! switcher count, and the active filter's text and caret.

use blitz_dom::local_name;
use rstest::rstest;

use super::harness::*;
use crate::bridge::frame::queue::{GuiQueueHeaderFacts, GuiQueueSummary};
use crate::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};
use crate::window::components::queue_panel::geometry::HEADER_HEIGHT_PX;

/// The stylesheet the app loads, read so the header's height token must match
/// the constant its own bar and the panel's overlays place against.
const APP_CSS: &str = include_str!("../../../../../assets/app.css");

/// The markers the header's parts carry, in the design's order (D7): the
/// switcher, the new-queue control, the close control, the spacer, and the
/// filter at the trailing edge. The bar paints no `QUEUE` label (user request
/// 2026-11-19).
const HEADER_PARTS: [&str; 5] = [
    "data-queue-chooser",
    "data-new-queue",
    "data-close-queue",
    "data-queue-spacer",
    "data-filter",
];

/// The selector that names every part of the header bar.
const HEADER_PARTS_SELECTOR: &str = "[data-queue-chooser], [data-new-queue], \
                                     [data-close-queue], [data-queue-spacer], [data-filter]";

/// Contract (design D7 of `gui-queue-panel-redesign`, spec `gui-queue-chooser`
/// "One queue and no peer"): with only `Default` holding 26 slots in the local
/// Owner and no peer connected, the header paints the
/// switcher with `Default` and the count 26, the new-queue control, the
/// disabled close control — which claims no pointer — and the filter at the
/// trailing edge, in that order. It paints no remaining count and no finish
/// clock.
#[test]
fn a_one_entry_header_paints_the_switcher_its_count_and_the_design_order() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), summary(), None);

    assert!(
        !fixture.presents("[data-queue-label]"),
        "no QUEUE label paints"
    );
    assert_eq!(
        fixture
            .attr_of("[data-queue-chooser]", local_name!("aria-label"))
            .as_deref(),
        Some("Queue chooser")
    );
    assert_eq!(fixture.text_of("[data-queue-title]"), "Default");
    assert_eq!(fixture.text_of("[data-queue-count]"), "26");
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
        Some("true"),
        "the only local queue has no close"
    );
    assert_eq!(
        fixture.marker_order(HEADER_PARTS_SELECTOR, &HEADER_PARTS),
        HEADER_PARTS.to_vec(),
        "the bar paints its parts in the design's order"
    );
    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::Queue(QueueTargetId::QueueChooser),
            RegionTarget::Queue(QueueTargetId::NewQueue),
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
        ],
        "the switcher always claims a pointer, the disabled close control none"
    );
    assert!(!fixture.presents("[data-queue-heading]"), "no count paints");
    assert!(!fixture.presents("[data-finish-clock]"), "no clock paints");
}

/// Contract (spec `gui-queue-chooser` "Several entries"): with `Default` and
/// `Kids` in the local Owner, the header paints the switcher with the shown
/// queue's name, the new-queue control, the close control — which now acts, so
/// it reports its region too — and the filter at the trailing edge.
#[test]
fn several_entries_paints_the_switcher_with_the_shown_name_and_the_close_control() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), several_entries(), None);

    assert_eq!(fixture.text_of("[data-queue-title]"), "Kids");
    assert_eq!(
        fixture.text_of("[data-queue-count]"),
        "4",
        "the switcher counts the shown queue's slots"
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
            RegionTarget::Queue(QueueTargetId::NewQueue),
            RegionTarget::Queue(QueueTargetId::CloseQueue),
            RegionTarget::Queue(QueueTargetId::RenameQueue),
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
        ]
    );
}

/// Contract (spec `gui-queue-chooser` "The filter narrows the rows"): a filter
/// that narrows the shown queue's rows paints its text and the caret, and the
/// switcher still counts every slot of the shown queue.
#[test]
fn the_active_filter_paints_its_text_and_the_caret_beside_the_whole_count() {
    let mut filtered = summary();
    filtered.filter = "amb".to_owned();
    filtered.filter_active = true;
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), filtered, None);

    assert_eq!(fixture.text_of("[data-filter]"), "amb");
    assert!(fixture.presents("[data-caret]"));
    assert_eq!(
        fixture.text_of("[data-queue-count]"),
        "26",
        "the filter narrows the rows, never the count"
    );
}

/// Contract (user request 2026-11-17, design D7 of `gui-queue-panel-redesign`):
/// the switcher always paints the shown entry's name, and `Queue` while that
/// name is empty.
#[rstest]
#[case::named("Default", "Default")]
#[case::unnamed("", "Queue")]
fn the_switcher_paints_the_shown_name_or_the_default_title(
    #[case] shown_name: &str,
    #[case] title: &str,
) {
    let summary = GuiQueueSummary {
        header: GuiQueueHeaderFacts {
            shown_name: shown_name.to_owned(),
            ..summary().header
        },
        ..summary()
    };
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), summary, None);

    assert_eq!(fixture.text_of("[data-queue-title]"), title);
    assert!(
        fixture.presents("[data-queue-chooser]"),
        "the switcher paints whatever the list holds"
    );
}

/// Contract (design D7 of `gui-queue-panel-redesign`): the stylesheet's
/// `--canvas-queue-header-height` equals `HEADER_HEIGHT_PX`. The bar paints the
/// constant, while the ghost, the scroll bar, and the row menu position below
/// the stylesheet token, so a drift between the two would move those overlays
/// off the bar's bottom edge.
#[test]
fn the_header_height_matches_the_stylesheet_token() {
    assert!(
        APP_CSS.contains(&format!(
            "--canvas-queue-header-height: {HEADER_HEIGHT_PX}px;"
        )),
        "app.css must define --canvas-queue-header-height: {HEADER_HEIGHT_PX}px"
    );
}
