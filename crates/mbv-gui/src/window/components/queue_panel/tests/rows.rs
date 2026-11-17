//! The Queue panel's row-paint contracts (`QueuePanel.test.ts`): the frame's
//! viewport rows, the empty-queue state, the watched, active, and
//! partial-progress states, the start mark, the canvas one-liner, the four row
//! looks, and the hovered row's chrome.

use blitz_dom::local_name;
use rstest::rstest;

use super::harness::*;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueRow, GuiQueueScroll, GuiSemanticState};

/// Contract (`QueuePanel.test.ts`): a frame with slots renders its rows — the
/// item rows' titles and the structural heading among them, and one
/// `[data-slot]` button per item row.
#[test]
fn a_frame_with_slots_renders_its_rows() {
    let rows = vec![
        renamed(item(1), "First track"),
        GuiQueueRow::Heading {
            text: "album".to_owned(),
        },
        renamed(item(2), "Second track"),
    ];
    let fixture = built_with_frame(queue_panel_app, queue(rows), summary(), None);

    let text = fixture.text();
    assert!(text.contains("First track"), "{text}");
    assert!(text.contains("album"), "{text}");
    assert!(text.contains("Second track"), "{text}");
    assert_eq!(fixture.all("[data-slot]").len(), 2);
    // The Queue zone (`gui-pointer-target-interaction` 3.3): the recessed box
    // that holds the header, the filter, and these rows names the Queue.
    assert!(
        fixture.presents("[data-pointer-zone=\"queue\"]"),
        "the list box carries the Queue zone marker"
    );
}

/// Contract (`QueuePanel.test.ts`): an empty frame renders the empty-queue
/// state — the shrug placeholder, and no row buttons.
#[test]
fn an_empty_frame_renders_the_empty_queue_state() {
    let fixture = built_with_frame(queue_panel_app, queue(Vec::new()), summary(), None);

    assert!(fixture.text().contains("¯\\_(ツ)_/¯"));
    assert_eq!(fixture.all("[data-slot]").len(), 0);
}

/// Contract (design D9 of `gui-named-queues`, spec `gui-client` "A stopped
/// queue marks its start slot", "Play turns the start mark into the playing
/// mark", "A peer queue marks its playing row", "The right column is
/// switched"): the mark is one row in two states — filled while the queue
/// plays, outlined while a queue that reports its start slot is stopped or
/// resting — and a queue that reports no start slot (a peer without
/// `named-queues`) marks its observed active slot as the playing row while it
/// plays and paints no outlined mark. No other row carries a mark or a
/// runtime.
#[rstest]
#[case::reported_stopped(Some(3), false, true, Some((3, "none")))]
#[case::reported_playing(Some(3), true, true, Some((3, "currentColor")))]
#[case::reported_without_a_slot(None, false, true, None)]
#[case::observed_active_stopped(Some(2), false, false, None)]
#[case::observed_active_playing(Some(2), true, false, Some((2, "currentColor")))]
fn the_start_slot_carries_the_filled_or_outlined_mark(
    #[case] start: Option<u64>,
    #[case] playing: bool,
    #[case] start_slot_reported: bool,
    #[case] expected: Option<(u64, &str)>,
) {
    let mut rows = queue(vec![item(1), item(2), item(3)]);
    rows.start = start;
    rows.playing = playing;
    rows.start_slot_reported = start_slot_reported;
    let fixture = built_with_frame(queue_panel_app, rows, summary(), None);

    assert_eq!(marks(&fixture), expected_fills(expected));
    assert!(
        !fixture.text().contains("3:12"),
        "no row paints its runtime"
    );
}

/// The fill attribute of each of the three fixture rows' mark, `None` where
/// the row paints no mark.
fn marks(fixture: &Fixture) -> Vec<Option<String>> {
    (1..=3_u64)
        .map(|slot| fixture.attr_of(&format!("[data-slot=\"{slot}\"] svg"), local_name!("fill")))
        .collect()
}

/// The fills `marks` expects for a lone mark on `expected`'s slot.
fn expected_fills(expected: Option<(u64, &str)>) -> Vec<Option<String>> {
    (1..=3_u64)
        .map(|slot| {
            expected
                .filter(|(marked, _)| *marked == slot)
                .map(|(_, fill)| fill.to_owned())
        })
        .collect()
}

/// Contract (`QueuePanel.test.ts`): the rows paint the watched, active, and
/// partial-progress states — the watched row strikes its title through in
/// the canvas's strike colour, the playing row keeps its own fill, carries the
/// filled start mark, and weights its title 600, and the track under the title
/// fills to max(percent, 3)%.
#[test]
fn the_rows_paint_the_watched_active_and_partial_progress_states() {
    let rows = vec![
        in_state(renamed(item(1), "Watched"), GuiSemanticState::Played),
        in_state(renamed(item(2), "Playing"), GuiSemanticState::NowPlaying),
        in_state(
            renamed(item(3), "Partial"),
            GuiSemanticState::Active { progress: Some(42) },
        ),
        in_state(
            renamed(item(4), "Started"),
            GuiSemanticState::Active { progress: Some(1) },
        ),
    ];
    let mut rows_frame = queue(rows);
    rows_frame.start = Some(2);
    rows_frame.playing = true;
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    let watched = fixture.attr_of("[data-slot=\"1\"] .truncate", local_name!("class"));
    assert!(watched.is_some_and(|class| class.contains("line-through")));
    let watched_style = fixture.style_of("[data-slot=\"1\"] .truncate");
    assert!(
        watched_style.contains("text-decoration-color: var(--canvas-strike)"),
        "{watched_style}"
    );

    let playing_row = fixture.style_of("[data-slot=\"2\"]");
    assert!(!playing_row.contains("background"), "{playing_row}");
    assert!(fixture.presents("[data-slot=\"2\"] svg"));
    let playing_title = fixture.style_of("[data-slot=\"2\"] .truncate");
    assert!(
        playing_title.contains("font-weight: 600"),
        "{playing_title}"
    );

    let partial = fixture.style_of("[data-slot=\"3\"] [data-progress] span");
    assert!(partial.contains("width: 42%"), "{partial}");
    let started = fixture.style_of("[data-slot=\"4\"] [data-progress] span");
    assert!(started.contains("width: 3%"), "{started}");
}

/// Contract (`QueuePanel.test.ts`): the row paints the canvas one-liner —
/// its padded ordinal, one title line, and the value; no second line, no
/// date gutter, and no play marker.
#[test]
fn the_row_paints_the_canvas_one_liner() {
    let row = with_details(renamed(item(1), "Nether Zone"), "Peep Show", "mon 08");
    let fixture = built_with_frame(queue_panel_app, queue(vec![row]), summary(), None);

    assert!(fixture.presents("button[data-slot=\"1\"]"));
    assert_eq!(fixture.text_of("[data-slot=\"1\"] [data-ordinal]"), "01");
    let text = fixture.text();
    assert!(text.contains("Nether Zone"), "{text}");
    assert!(!text.contains("Peep Show"), "{text}");
    assert!(!text.contains("mon 08"), "{text}");
    assert!(!fixture.presents("[data-slot=\"1\"] svg"));
}

/// Contract (`QueuePanel.test.ts`, design D7): the active row paints the
/// canvas picked fill and its 4 px leading accent edge with no focus fact —
/// the window holds none — and a marked row paints the same fill with no
/// edge, playing or not.
#[test]
fn the_active_and_marked_rows_paint_the_canvas_picked_fill() {
    let rows = vec![item(1), in_state(item(2), GuiSemanticState::NowPlaying)];
    let mut rows_frame = queue(rows);
    rows_frame.active = Some(2);
    rows_frame.cursor = Some(1);
    rows_frame.marks = vec![2];
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    assert!(
        fixture
            .style_of("[data-slot=\"1\"]")
            .contains("background: var(--canvas-selected)")
    );
    let edge = fixture.style_of("[data-active-edge=\"1\"]");
    assert!(edge.contains("width: 4px"), "{edge}");
    assert!(edge.contains("background: var(--canvas-accent)"), "{edge}");
    assert!(
        fixture
            .style_of("[data-slot=\"2\"]")
            .contains("background: var(--canvas-selected)")
    );
    assert!(!fixture.presents("[data-active-edge=\"2\"]"));
}

/// Contract (`QueuePanel.test.ts`, design D7): a hovered row that is not the
/// active row paints no picked fill and no edge, so the active row keeps its
/// treatment while the pointer rests on its neighbour.
#[test]
fn the_hovered_row_paints_no_active_treatment() {
    let mut rows_frame = queue(vec![item(1), item(2)]);
    rows_frame.cursor = Some(1);
    rows_frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    let hovered = fixture.style_of("[data-slot=\"2\"]");
    assert!(
        !hovered.contains("background: var(--canvas-selected)"),
        "{hovered}"
    );
    assert!(!fixture.presents("[data-active-edge=\"2\"]"));
    assert!(
        fixture
            .style_of("[data-slot=\"1\"]")
            .contains("background: var(--canvas-selected)"),
        "the active row keeps its fill"
    );
    assert!(fixture.presents("[data-active-edge=\"1\"]"));
}

/// Contract (`QueuePanel.test.ts`, the Menus artboard B.3): the hovered row
/// alone paints the grip and the actions button — the grip's six dots in the
/// leading slot, the 30 px actions box in the canvas's fill and radius over the
/// empty space at the row's trailing end, the row filled with the hover colour,
/// and its title ellipsised in the hover text.
#[test]
fn the_hovered_row_alone_paints_the_grip_and_the_actions_button() {
    let rows = vec![item(1), item(2), item(3)];
    let mut rows_frame = queue(rows);
    rows_frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    assert_eq!(fixture.all("[data-grip]").len(), 1);
    assert_eq!(fixture.all("[data-actions]").len(), 1);
    assert_eq!(fixture.all("[data-row=\"2\"] [data-grip] circle").len(), 6);
    assert!(!fixture.presents("[data-row=\"2\"] [data-ordinal]"));
    assert!(fixture.presents("[data-row=\"1\"] [data-ordinal]"));
    assert_eq!(
        fixture
            .attr_of("[data-actions]", local_name!("aria-label"))
            .as_deref(),
        Some("Row actions")
    );
    let actions_style = fixture.style_of("[data-actions]");
    assert!(actions_style.contains("width: 30px"), "{actions_style}");
    assert!(actions_style.contains("height: 30px"), "{actions_style}");
    assert!(
        actions_style.contains("border-radius: 4px"),
        "{actions_style}"
    );
    assert!(
        actions_style.contains("background: var(--canvas-action)"),
        "{actions_style}"
    );

    // The hovered row reserves the button's width at its trailing end and
    // paints no fill of its own (design D7): the veil every hovered `button`
    // paints (`assets/app.css`) is its hover look, and the picked fill stays
    // with the active and marked rows.
    let row_style = fixture.style_of("[data-slot=\"2\"]");
    assert!(
        row_style.contains("padding-inline: 4px 34px"),
        "{row_style}"
    );
    assert!(!row_style.contains("background"), "{row_style}");
    assert!(row_style.contains("border-radius: 4px"), "{row_style}");
    let title = fixture.style_of("[data-slot=\"2\"] .truncate");
    assert!(title.contains("color: var(--canvas-heading)"), "{title}");
}

/// Contract (user request 2026-11-17): a list longer than its viewport paints
/// a scroll bar whose thumb is the viewport's share of the list and sits at
/// the viewport's offset.
#[test]
fn a_long_list_paints_a_scroll_bar_thumb_at_the_viewport() {
    let long = GuiQueue {
        scroll: Some(GuiQueueScroll {
            offset: 5,
            visible: 5,
            total: 20,
        }),
        ..queue((6..=10).map(item).collect())
    };
    let fixture = built_with_frame(queue_panel_app, long, summary(), None);

    let thumb = fixture.style_of("[data-scrollbar-thumb]");
    assert!(thumb.contains("top: 25%"), "{thumb}");
    assert!(thumb.contains("height: 25%"), "{thumb}");
}

/// Contract (user request 2026-11-17): a list that fits its viewport paints no
/// scroll bar.
#[test]
fn a_list_that_fits_paints_no_scroll_bar() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue((1..=3).map(item).collect()),
        summary(),
        None,
    );

    assert!(!fixture.presents("[data-scrollbar]"));
}
