//! The children's seeded painted-content contracts: the compact strip's meta
//! line with and without a next item, the no-image and stopped columns, the
//! folded queue's summary line, and the rail's remaining time, expand label,
//! dim fill, and ring strokes.

use blitz_dom::{Document as _, local_name};
use dioxus_native::DioxusDocument;

use super::harness::*;
use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::GuiQueuePlayback;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::time::format_wall_clock;

/// Contract (`Panel.test.ts`): the compact strip paints the meta line with
/// the next item — the parent line and `Next: <title>` joined as one
/// truncating line — and no queue row paints (the next item is a summary
/// datum, not a painted row of its own).
#[test]
fn the_compact_strip_paints_the_meta_line_with_the_next_item() {
    let mut fixture = built(SHORT);
    fixture.set_playback(playback());
    fixture.seed(summary_with_next(Some("Second track")));

    assert!(fixture.text().contains("The Show · Next: Second track"));
    assert!(
        !fixture.presents("[data-slot]"),
        "the strip paints no queue row"
    );
}

/// Contract (`Panel.test.ts`): the strip paints the parent line alone while
/// no slot follows the playhead.
#[test]
fn the_strip_paints_the_parent_line_alone_while_no_slot_follows() {
    let mut fixture = built(SHORT);
    fixture.set_playback(playback());
    fixture.seed(summary_with_next(None));

    let text = fixture.text();
    assert!(text.contains("The Show"), "{text}");
    assert!(!text.contains("Next:"), "{text}");
}

/// The no-image column's paint (`gui-art-player-overlay` D6, task 3.4b): no
/// stage paints, and the chips line paints before the control rows.
fn assert_no_image_column(artwork: Option<GuiArtRef>) {
    let mut fixture = built(TALL);
    fixture.set_playback(GuiQueuePlayback {
        artwork,
        track_chips: Some(chips()),
        ..playback()
    });
    fixture.seed(summary());

    assert!(
        !fixture.presents("[data-target=\"visualSlot\"]"),
        "the no-image column paints no stage"
    );
    assert!(fixture.presents("[data-chip]"), "the chips line paints");
    assert!(
        fixture.presents("[data-target=\"playPause\"]"),
        "the control rows paint"
    );
    let text = fixture.text();
    let chips_at = text.find("1080p").expect("the video chip paints");
    let rows_at = text
        .find("+30")
        .expect("the control rows' forward label paints");
    assert!(
        chips_at < rows_at,
        "the chips line paints before the control rows:\n{text}"
    );
}

/// Contract (`gui-art-player-overlay` D6, task 3.4b): an item with no image
/// paints the chips-and-badge line before the control rows and no stage — for
/// the bundled placeholder and for a frame with no artwork at all.
#[test]
fn a_placeholder_or_no_artwork_paints_the_chips_line_before_the_control_rows() {
    assert_no_image_column(Some(GuiArtRef::Placeholder {
        src: "mbv-art://localhost/placeholder".to_owned(),
    }));
    assert_no_image_column(None);
}

/// Contract (`gui-art-player-overlay` D7, task 3.4c): the real stopped frame —
/// no title, no artwork, the controls behind `show_controls`, and a shown local
/// queue that can start — paints the two control rows and no stage, no chips
/// line, and no title block.
#[test]
fn the_stopped_player_paints_its_control_rows_and_no_stage_and_no_title() {
    let mut fixture = built(TALL);
    fixture.set_playback(stopped(true));
    fixture.seed(summary());

    assert!(
        fixture.presents("[data-target=\"playPause\"]"),
        "the first control row paints"
    );
    assert!(
        fixture.presents("[data-target=\"previous\"]"),
        "the second control row paints"
    );
    assert!(
        !fixture.presents("[data-target=\"visualSlot\"]"),
        "no stage paints"
    );
    assert!(!fixture.presents("[data-chip]"), "no chips line paints");
    assert!(
        !fixture.text().contains("The Show"),
        "the title block paints no parent line"
    );
}

/// Contract (`gui-art-player-overlay` D7, task 3.4d): the same stopped frame
/// while the shown queue cannot start — a shown peer queue or an empty shown
/// local queue — paints no control rows.
#[test]
fn a_stopped_player_that_cannot_start_paints_no_control_rows() {
    let mut fixture = built(TALL);
    fixture.set_playback(stopped(false));
    fixture.seed(summary());

    assert!(
        !fixture.presents("[data-target=\"playPause\"]"),
        "no first control row paints"
    );
    assert!(
        !fixture.presents("[data-target=\"previous\"]"),
        "no second control row paints"
    );
    assert!(
        !fixture.presents("[data-target=\"visualSlot\"]"),
        "no stage paints"
    );
}

/// Contract (gui-player-overlay-menu): the open player menu paints the menu
/// card at the frame's anchor, its single auto-hide entry checked from the
/// playback section's fact, and the entry's region reports last — the menu
/// card wins the hit map where it paints over the panel.
#[test]
fn the_open_player_menu_paints_the_card_at_the_anchor_with_the_checked_entry() {
    let mut fixture = built(TALL);
    fixture.set_playback(GuiQueuePlayback {
        controls: crate::bridge::frame::playback::GuiPlayerControls {
            auto_hide: true,
            can_start: false,
        },
        ..playback()
    });
    fixture.set_overlay(Some(GuiOverlayFrame::PlayerMenu {
        cursor: 0,
        x: 30.0,
        y: 40.0,
    }));

    assert!(fixture.presents("[data-row-menu]"), "the menu card paints");
    let text = fixture.text();
    assert!(text.contains("Auto-hide controls"), "{text}");
    assert!(
        fixture.presents("[role=menuitemcheckbox]"),
        "the check entry paints"
    );
    assert_eq!(
        fixture
            .attr_of("[role=menuitemcheckbox]", local_name!("aria-checked"))
            .as_deref(),
        Some("true"),
        "the entry paints the playback section's checked fact"
    );
    let style = fixture.style_of("[data-row-menu]");
    assert!(
        style.contains("top: 40px;") && style.contains("left: 30px;"),
        "the card paints at the frame's anchor: {style}"
    );
    assert_eq!(
        fixture.last_reported_targets().last(),
        Some(&RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::PlayerMenuEntry
        )),
        "the entry's region reports last"
    );
}

/// Contract (gui-player-overlay-menu): a closed player menu paints no card,
/// and the menu's key retracts — the newest report carries no entry region.
#[test]
fn a_closed_player_menu_paints_no_card_and_retracts_the_entry_region() {
    let mut fixture = built(TALL);
    fixture.set_overlay(Some(GuiOverlayFrame::PlayerMenu {
        cursor: 0,
        x: 30.0,
        y: 40.0,
    }));
    fixture.set_overlay(None);

    assert!(!fixture.presents("[data-row-menu]"), "no card paints");
    assert!(
        !fixture
            .last_reported_targets()
            .contains(&RegionTarget::QueuePlayback(
                QueuePlaybackTargetId::PlayerMenuEntry
            )),
        "the newest report carries no entry region"
    );
}

/// Contract (`Panel.test.ts`): the folded queue paints the summary line —
/// the remaining count, the finish clock at the summary's tail past the
/// transport's finish, and the `Expand queue` control.
#[test]
fn the_folded_queue_paints_the_summary_line() {
    let mut fixture = built(TALL);
    fixture.seed(summary());
    fixture.set_toggles(false, true);

    assert_eq!(fixture.text_of("[data-remaining]"), "1");
    // The queue's finish is the transport's finish (the wall clock plus the
    // active item's remaining 140 s) plus the tail's 43 minutes; the
    // expectation derives from the same fixed clock with the shared
    // formatter, so the assertion holds in every zone.
    let expected_finish = format_wall_clock(WALL_MS.div_euclid(1000) + 140 + 43 * 60);
    assert_eq!(fixture.text_of("[data-finish-clock]"), expected_finish);
    assert_eq!(
        fixture
            .attr_of("[data-target=\"queueFold\"]", local_name!("aria-label"))
            .as_deref(),
        Some("Expand queue")
    );
}

/// Contract (`Panel.test.ts`): the rail paints the remaining time, the
/// expand control, and the dim canvas fill (the TUI's `BgDim` slot).
#[test]
fn the_rail_paints_the_remaining_time_the_expand_control_and_the_dim_fill() {
    let mut fixture = built(TALL);
    fixture.seed(summary());
    fixture.set_toggles(true, false);

    // The frozen prediction leaves 140 s of the 200 s item: the same
    // `-M:SS` the expanded scrubber's right end paints.
    assert!(fixture.text().contains("-2:20"));
    assert_eq!(
        fixture
            .attr_of("[data-target=\"collapse\"]", local_name!("aria-label"))
            .as_deref(),
        Some("Expand panel")
    );
    let rail = fixture.style_of("[data-panel-state=\"rail\"]");
    assert!(rail.contains("background: var(--canvas-dim)"), "{rail}");
}

/// Contract (live run of PR 1003, the live-run defect of
/// `replace-tauri-with-blitz` §11): the ring strokes the played-fraction
/// track and arc in their own roles. Blitz paints an inline `svg` through
/// usvg, which cannot parse a `var()` in a `stroke` attribute — it drops the
/// stroke, so both circles painted nothing. Each circle names its role in the
/// computed `color` instead, and the serialiser writes it into the attribute.
#[test]
fn the_rail_ring_strokes_both_circles_in_their_theme_roles() {
    let mut fixture = built(TALL);
    fixture.seed(summary());
    fixture.set_toggles(true, false);

    assert_eq!(
        svg_strokes(&fixture.doc, "[data-target=\"visualSlot\"] svg"),
        vec![Some((17, 34, 51)), Some((68, 85, 102))]
    );
}

/// The stroke colour of every path in the svg the selector names, in document
/// order: the `usvg` tree Blitz parsed from the serialised subtree, which is
/// what the painter paints. A stroke usvg could not parse reads as `None`.
fn svg_strokes(doc: &DioxusDocument, selector: &str) -> Vec<Option<Rgb>> {
    let dom = doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    let tree = dom
        .get_node(id)
        .and_then(|node| node.element_data())
        .and_then(|element| element.svg_data())
        .unwrap_or_else(|| panic!("{selector} paints as an svg tree"));
    let mut strokes = Vec::new();
    collect_strokes(tree.root(), &mut strokes);
    strokes
}

/// A paint's colour, as rgb bytes.
type Rgb = (u8, u8, u8);

/// Push the stroke of every path of `group` and its nested groups, in
/// document order: usvg groups the view box transform's children.
fn collect_strokes(group: &usvg::Group, strokes: &mut Vec<Option<Rgb>>) {
    for child in group.children() {
        match child {
            usvg::Node::Path(path) => strokes.push(path.stroke().map(|stroke| rgb(stroke.paint()))),
            usvg::Node::Group(nested) => collect_strokes(nested, strokes),
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

/// A flat paint's colour.
fn rgb(paint: &usvg::Paint) -> Rgb {
    match paint {
        usvg::Paint::Color(color) => (color.red, color.green, color.blue),
        _ => panic!("the ring paints in a flat colour"),
    }
}
