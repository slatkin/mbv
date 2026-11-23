//! The Queue panel's row-paint contracts (`QueuePanel.test.ts`): the frame's
//! viewport rows, the empty-queue state, the watched, active, and
//! partial-progress states, the progress ring, the right column's duration, the
//! canvas one-liner, the four row looks, and the hovered row's chrome.

use blitz_dom::local_name;

use super::harness::*;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueRow, GuiQueueScroll, GuiSemanticState};
use crate::window::theme::{EfToken, ef_var};

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

/// Contract (spec `gui-client` "The right column is switched", design D3 of
/// `gui-delineated-rows`): every row state paints the item's duration in its
/// right column, and a stopped queue's start slot paints no start mark — no
/// row carries a state glyph before its title.
#[test]
fn every_row_paints_its_duration_and_no_row_paints_a_start_mark() {
    let rows = vec![
        renamed(item(1), "New"),
        in_state(
            renamed(item(2), "Partial"),
            GuiSemanticState::Active { progress: Some(42) },
        ),
        in_state(renamed(item(3), "Watched"), GuiSemanticState::Played),
        in_state(
            renamed(item(4), "Playing"),
            GuiSemanticState::NowPlaying {
                tracks_progress: true,
            },
        ),
    ];
    let fixture = built_with_frame(queue_panel_app, queue(rows), summary(), None);

    assert_eq!(durations(&fixture), vec![Some("3:12".to_owned()); 4]);
    assert!(
        !fixture.presents("[data-slot=\"1\"] svg"),
        "no row paints a start mark"
    );
}

/// The right column's text of each of the four fixture rows, `None` where the
/// row paints no duration.
fn durations(fixture: &Fixture) -> Vec<Option<String>> {
    (1..=4_u64)
        .map(|slot| {
            fixture
                .presents(&format!("[data-slot=\"{slot}\"] [data-duration]"))
                .then(|| fixture.text_of(&format!("[data-slot=\"{slot}\"] [data-duration]")))
        })
        .collect()
}

/// Contract (spec `gui-client` "Rows show their state", design D3): the row's
/// progress ring paints in the right column, directly before the duration —
/// on a partial row and on the
/// now-playing row, which follows the playhead — and a new or watched row
/// carries none. The `data-progress` value is the percent the ring fills to.
#[test]
fn only_a_row_with_progress_paints_the_ring() {
    let rows = vec![
        renamed(item(1), "New"),
        in_state(
            renamed(item(2), "Partial"),
            GuiSemanticState::Active { progress: Some(42) },
        ),
        in_state(renamed(item(3), "Watched"), GuiSemanticState::Played),
        in_state(
            renamed(item(4), "Playing"),
            GuiSemanticState::NowPlaying {
                tracks_progress: true,
            },
        ),
    ];
    let fixture = built_with_anchor(queue_panel_app, queue(rows), summary(), 4);

    assert_eq!(
        ring_percents(&fixture),
        vec![None, Some("42".to_owned()), None, Some("30".to_owned())]
    );

    // The ring and the duration paint as one cluster in the right column
    // (user request 2026-11-23): the ring's parent carries the column's fixed
    // width, and the ring paints before the duration in it.
    let dom = fixture.doc.inner.borrow();
    let ring_id = *fixture
        .all("[data-slot=\"2\"] [data-progress]")
        .first()
        .expect("the partial row paints its ring");
    let duration_id = *fixture
        .all("[data-slot=\"2\"] [data-duration]")
        .first()
        .expect("the partial row paints its duration");
    let ring = dom.get_node(ring_id).expect("the ring node paints");
    let column = dom
        .get_node(ring.parent.expect("the ring paints in the column"))
        .expect("the column paints");
    assert!(
        column
            .attr(local_name!("style"))
            .unwrap_or_default()
            .contains("width: 96px"),
        "{}",
        column.attr(local_name!("style")).unwrap_or_default()
    );
    let ring_position = column
        .children
        .iter()
        .position(|id| *id == ring_id)
        .expect("the ring paints in the column");
    let duration_position = column
        .children
        .iter()
        .position(|id| *id == duration_id)
        .expect("the duration paints in the column");
    assert!(
        ring_position < duration_position,
        "the ring leads the duration"
    );
}

/// The `data-progress` value of each of the four fixture rows' ring, `None`
/// where the row paints no ring.
fn ring_percents(fixture: &Fixture) -> Vec<Option<String>> {
    (1..=4_u64)
        .map(|slot| {
            fixture.attr_of(
                &format!("[data-slot=\"{slot}\"] [data-progress]"),
                blitz_dom::LocalName::from("data-progress"),
            )
        })
        .collect()
}

/// Contract (`QueuePanel.test.ts`, `gui-queue-panel-redesign` design D9): the
/// rows paint the watched and now-playing states — the watched row paints its
/// title grey and crosses it with the darker strike colour, and the now-playing
/// row fills green and paints its title green at weight 600.
#[test]
fn the_rows_paint_the_watched_and_now_playing_states() {
    let rows = vec![
        in_state(renamed(item(1), "Watched"), GuiSemanticState::Played),
        in_state(
            renamed(item(2), "Playing"),
            GuiSemanticState::NowPlaying {
                tracks_progress: true,
            },
        ),
    ];
    let fixture = built_with_frame(queue_panel_app, queue(rows), summary(), None);

    let watched = fixture.attr_of("[data-slot=\"1\"] .truncate", local_name!("class"));
    assert!(watched.is_some_and(|class| class.contains("line-through")));
    let watched_style = fixture.style_of("[data-slot=\"1\"] .truncate");
    assert!(
        watched_style.contains(&format!("text-decoration-color: {}", ef_var(EfToken::Bg5))),
        "{watched_style}"
    );
    assert!(
        watched_style.contains(&format!("color: {}", ef_var(EfToken::Grey1))),
        "{watched_style}"
    );

    let playing_row = fixture.style_of("[data-slot=\"2\"]");
    assert!(
        playing_row.contains(&format!("background: {}", ef_var(EfToken::BgGreen))),
        "{playing_row}"
    );
    assert!(
        playing_row.contains("border-radius: 0px"),
        "the now-playing row is as square as every row: {playing_row}"
    );
    let playing_title = fixture.style_of("[data-slot=\"2\"] .truncate");
    assert!(
        playing_title.contains("font-weight: 600"),
        "{playing_title}"
    );
    assert!(
        playing_title.contains(&format!("color: {}", ef_var(EfToken::Green))),
        "{playing_title}"
    );
}

/// Contract (`QueuePanel.test.ts`): the row paints the canvas one-liner —
/// its padded ordinal, one title line, and the right column's duration; no
/// second line, no date gutter, and no state glyph.
#[test]
fn the_row_paints_the_canvas_one_liner() {
    let row = with_details(renamed(item(1), "Nether Zone"), "Peep Show", "mon 08");
    let fixture = built_with_frame(queue_panel_app, queue(vec![row]), summary(), None);

    assert!(fixture.presents("button[data-slot=\"1\"]"));
    assert_eq!(fixture.text_of("[data-slot=\"1\"] [data-ordinal]"), "01");
    assert_eq!(fixture.text_of("[data-slot=\"1\"] [data-duration]"), "3:12");
    let text = fixture.text();
    assert!(text.contains("Nether Zone"), "{text}");
    assert!(!text.contains("Peep Show"), "{text}");
    assert!(!text.contains("mon 08"), "{text}");
    assert!(!fixture.presents("[data-slot=\"1\"] svg"));
}

/// Regression (2026-11-18 queue title truncation): after a narrow measure
/// cuts a long title, a wider row paints it whole again. Without grow, the
/// cut box keeps its shrunken width and the title never regrows.
#[test]
fn a_long_title_regrows_when_the_row_gets_room() {
    let title = "Dedication (To Daddy Trane and Brother Shorter)";
    let mut fixture = built_at_with_real_title_width(queue_panel_app, 240, 900);
    fixture.show(queue(vec![renamed(item(1), title)]), summary(), None);

    assert!(
        fixture.text_of("[data-slot=\"1\"] .truncate").contains('…'),
        "the narrow row cuts the title"
    );
    fixture.resize(640, 900);
    assert_eq!(fixture.text_of("[data-slot=\"1\"] .truncate"), title);
}

/// Contract (`gui-client` "The Queue is a ledger"): a marked row paints the
/// raised fill with no focus fact — the window holds none — and the cursor row
/// paints no selection fill, because only a multi-selection shows as selected.
#[test]
fn only_a_marked_row_paints_the_raised_fill() {
    let mut rows_frame = queue(vec![item(1), item(2)]);
    rows_frame.cursor = Some(1);
    rows_frame.marks = vec![2];
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    let raised = format!("background: {}", ef_var(EfToken::Bg3));
    assert!(fixture.style_of("[data-slot=\"2\"]").contains(&raised));
    assert!(!fixture.style_of("[data-slot=\"1\"]").contains("background"));
    assert!(!fixture.presents("[data-active-edge]"));
}

/// Contract (`gui-queue-panel-redesign` design D9): a hovered row paints the
/// hover fill, while a marked row keeps its raised fill under the pointer
/// resting on its neighbour. The hover paints no chrome of its own (user
/// request 2026-11-23): the hovered row keeps its ordinal, its cells, and its
/// own insets, and the row's drag starts on motion after a press.
#[test]
fn the_hovered_row_paints_the_hover_fill() {
    let mut rows_frame = queue(vec![item(1), item(2)]);
    rows_frame.marks = vec![1];
    rows_frame.hover = Some(2);
    let fixture = built_with_frame(queue_panel_app, rows_frame, summary(), None);

    let hovered = fixture.style_of("[data-slot=\"2\"]");
    assert!(
        hovered.contains(&format!("background: {}", ef_var(EfToken::Bg0))),
        "{hovered}"
    );
    assert!(
        fixture
            .style_of("[data-slot=\"1\"]")
            .contains(&format!("background: {}", ef_var(EfToken::Bg3))),
        "the marked row keeps its fill"
    );
    assert!(fixture.presents("[data-row=\"2\"] [data-ordinal]"));
    assert!(hovered.contains("padding-inline: 10px"), "{hovered}");
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

/// Contract (user report 2026-11-19, after `gui-queue-panel-redesign`): the
/// queue header and the first queue row both paint. The size of the gap
/// between them is design presentation, not a contract: no test asserts
/// geometry (user decision 2026-11-19).
#[test]
fn the_queue_header_and_the_first_row_paint() {
    let fixture = built_with_frame(queue_panel_app, queue(vec![item(1)]), summary(), None);

    assert!(fixture.presents("[data-queue-header]"));
    assert!(fixture.presents("[data-slot]"));
}
