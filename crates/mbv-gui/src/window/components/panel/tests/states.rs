//! The panel-state and region contracts of `Panel.test.ts`: the state each
//! container size and toggle picks, and the region targets each state
//! reports.

use super::harness::*;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};

/// Contract (`Panel.test.ts`): a tall container renders the expanded
/// artboard — the queue rows (not their folded summary) and the footer line
/// mount under the `expanded` state. The artboard's own painted content
/// (the title, the rows, the footer pills) asserts with its row's landing.
#[test]
fn a_tall_container_renders_the_expanded_artboard() {
    let fixture = built(TALL);
    assert!(
        fixture.presents("[data-panel-state=\"expanded\"]"),
        "the expanded state paints"
    );
    assert!(
        fixture.presents("[data-queue-list]"),
        "the expanded artboard paints the queue rows"
    );
    assert!(
        fixture.presents("[data-footer-line]"),
        "the expanded artboard paints the footer line"
    );
    assert!(
        !fixture.presents("[data-panel-child=\"FoldedQueue\"]"),
        "the expanded artboard does not fold the queue"
    );
}

/// Contract (`Panel.test.ts`): a container shorter than 520 px renders the
/// compact strip — the strip mounts under the `compactStrip` state and the
/// expanded artboard does not.
#[test]
fn a_container_shorter_than_520px_renders_the_compact_strip() {
    let fixture = built(SHORT);
    assert!(
        fixture.presents("[data-panel-state=\"compactStrip\"]"),
        "the compact strip state paints"
    );
    assert!(
        fixture.presents("[data-target=\"visualSlot\"]"),
        "the compact strip paints the strip"
    );
    assert!(
        !fixture.presents("[data-target=\"playbackTarget\"]"),
        "the compact strip paints no expanded artboard"
    );
}

/// Contract (`Panel.test.ts`): the queue-fold toggle replaces the rows with
/// the folded summary — the `expandedWithoutQueue` state paints the folded
/// queue and not the queue rows.
#[test]
fn the_queue_fold_toggle_replaces_the_queue_rows_with_the_folded_summary() {
    let mut fixture = built(TALL);
    fixture.set_toggles(false, true);
    assert!(
        fixture.presents("[data-panel-state=\"expandedWithoutQueue\"]"),
        "the folded state paints"
    );
    assert!(
        fixture.presents("[data-target=\"queueFold\"]"),
        "the folded state paints the one-line summary"
    );
    assert!(
        !fixture.presents("[data-queue-list]"),
        "the folded state paints no queue rows"
    );
}

/// Contract (`Panel.test.ts`): the collapse toggle renders the 72 px rail —
/// the rail mounts with no state wrapper around it (the rail paints its own
/// `data-panel-state`), and no expanded artboard mounts.
#[test]
fn the_collapse_toggle_renders_the_rail() {
    let mut fixture = built(TALL);
    fixture.set_toggles(true, false);
    assert!(
        fixture.presents("[data-panel-state=\"rail\"]"),
        "the collapsed state paints the rail"
    );
    // The rail paints its own state attribute, and no state wrapper mounts
    // around it.
    assert!(
        !fixture.presents("[data-panel-state=\"expanded\"]"),
        "the rail mounts no expanded artboard"
    );
    assert!(
        !fixture.presents("[data-target=\"playbackTarget\"]"),
        "the rail paints no expanded artboard"
    );
}

/// Contract (`Panel.test.ts`): the compact strip reports the still's slot
/// and the transport's two controls — `visualSlot`, `playPause`, `next` —
/// its whole region list, since no expanded artboard mounts beside it.
#[test]
fn the_compact_strip_reports_the_still_and_transport_regions() {
    let fixture = built(SHORT);

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
        ]
    );
}

/// Contract (`Panel.test.ts`): the folded queue reports the expand control's
/// `queueFold` target — the line's one interactive target.
#[test]
fn the_folded_queue_reports_the_expand_control_region() {
    let mut fixture = built(TALL);
    fixture.set_toggles(false, true);

    let targets = fixture.last_reported_targets();
    assert!(
        targets.contains(&RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::QueueFold
        )),
        "{targets:?}"
    );
}

/// Contract (user bug report 2026-11-25): a queue-only window does not
/// collapse. With no Library panel beside the panel, the collapse toggle
/// paints the expanded artboard, not a rail beside a blank window, and the
/// footer paints no collapse control.
#[test]
fn a_queue_only_panel_ignores_the_collapse_toggle_and_paints_no_collapse_control() {
    let mut fixture = built_queue_only(TALL);
    fixture.set_toggles(true, false);
    assert!(
        fixture.presents("[data-panel-state=\"expanded\"]"),
        "the queue-only panel stays expanded"
    );
    assert!(
        !fixture.presents("[data-target=\"collapse\"]"),
        "the queue-only footer paints no collapse control"
    );
}

/// Contract (`Panel.test.ts`): the rail reports the transport's two controls,
/// the artwork slot, and the expand control, in paint order — its whole
/// region list, since the queue and the footer belong to the expanded
/// artboard.
#[test]
fn the_rail_reports_the_transport_artwork_and_expand_regions() {
    let mut fixture = built(TALL);
    fixture.set_toggles(true, false);

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
        ]
    );
}
