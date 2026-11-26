//! Headless tests of the Workspace table's row paint: the play state's title
//! tone and resume run (`gui-shared-workspace-row-state` design D5), and the
//! trailing cell the stylesheet swaps for the hover controls (gui-row-hover-actions
//! design D5) — the row's duration rests as the `row-hover-trail`, and the
//! controls paint over it, Play always and Add only where the destination
//! passes `on_queue`. No test opens a window, sleeps, or reaches the core.

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{LibraryWorkspaceTable, WorkspaceRow};
use crate::bridge::frame::library::GuiHomeRowStateFrame;
use crate::window::pointer_target::PointerList;

/// The row id the fixture's case rows carry.
const ROW_ID: &str = "t1";

/// The harness's props: the rows the table paints and whether the destination
/// has an Add path.
#[derive(Clone)]
struct FixtureProps {
    rows: Vec<WorkspaceRow>,
    with_queue: bool,
}

/// The one row every case paints.
fn row() -> WorkspaceRow {
    WorkspaceRow {
        id: ROW_ID.to_owned(),
        number: "1".to_owned(),
        name: "Rhymin & Stealin".to_owned(),
        duration: "3:27".to_owned(),
        playing: false,
        state: None,
    }
}

/// The fixture's row carrying `state`.
fn row_with(state: Option<GuiHomeRowStateFrame>) -> WorkspaceRow {
    WorkspaceRow { state, ..row() }
}

/// The root: paints the table, as the Heroes mount it.
fn table_app(props: FixtureProps) -> Element {
    let on_queue = props.with_queue.then(|| Callback::new(|_: String| {}));
    rsx! {
        LibraryWorkspaceTable {
            root: "data-library-tracklist",
            label: "Tracklist",
            hook: "track",
            noun: "Track",
            pointer_list: PointerList::Tracklist,
            rows: props.rows,
            on_press: Callback::new(|_: String| {}),
            on_play: Callback::new(|_: String| {}),
            on_queue,
        }
    }
}

/// Build the document over the rows and let the first render land (a fixed
/// poll count with a tripwire: never an unbounded wait).
fn built(rows: Vec<WorkspaceRow>, with_queue: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(table_app, FixtureProps { rows, with_queue }),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    for _ in 0..8 {
        if !doc.poll(None) {
            return doc;
        }
    }
    panic!("document did not become idle");
}

/// The `data-row-action` value of every painted control, in paint order.
fn actions(doc: &DioxusDocument) -> Vec<String> {
    let dom = doc.inner();
    dom.query_selector_all(&format!(
        "[data-library-track=\"{ROW_ID}\"] [data-row-action]"
    ))
    .expect("the selector is valid")
    .into_iter()
    .filter_map(|id| {
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(LocalName::from("data-row-action"))
            .map(str::to_owned)
    })
    .collect()
}

/// The inline style of the first node `selector` matches, when it paints.
fn style_of(doc: &DioxusDocument, selector: &str) -> Option<String> {
    let dom = doc.inner();
    let node = dom
        .query_selector(selector)
        .expect("the selector is valid")?;
    dom.get_node(node)
        .expect("the node is in the document")
        .attr(LocalName::from("style"))
        .map(str::to_owned)
}

/// The `data-play-mark` value the fixture row's trailing cluster paints, when
/// the row carries a play state.
fn play_mark_value(doc: &DioxusDocument) -> Option<String> {
    let dom = doc.inner();
    let node = dom
        .query_selector(&format!(
            "[data-library-track=\"{ROW_ID}\"] [data-play-mark]"
        ))
        .expect("the selector is valid")?;
    dom.get_node(node)
        .expect("the node is in the document")
        .attr(LocalName::from("data-play-mark"))
        .map(str::to_owned)
}

/// The width the fixture row's resume run paints, when it paints one.
fn run_width(doc: &DioxusDocument) -> Option<String> {
    let style = style_of(
        doc,
        &format!("[data-library-track=\"{ROW_ID}\"] [data-library-track-progress]"),
    )?;
    style
        .split(';')
        .find_map(|part| part.trim().strip_prefix("width: ").map(str::to_owned))
}

/// Contract (`gui-shared-workspace-row-state` design D5, gui-played-row-marks
/// design D1, D4, the `gui-client` spec's Workspace play-state scenarios): a
/// row's title tone, its play mark, and its resume run come from the one shared
/// derivation — a played row paints the greyed tone and the played mark, a
/// partly played row with a percent paints its run at that percent, and a row
/// with no play state paints the text tone, no mark, and no run.
#[rstest]
#[case(None, "var(--canvas-text)", None, None)]
#[case(
    Some(GuiHomeRowStateFrame::Ordinary),
    "var(--ef-fg)",
    None,
    Some("unplayed")
)]
#[case(
    Some(GuiHomeRowStateFrame::Played),
    "var(--ef-grey1)",
    None,
    Some("played")
)]
#[case(
    Some(GuiHomeRowStateFrame::Active { progress: Some(42) }),
    "var(--ef-fg)",
    Some("42%"),
    Some("unplayed")
)]
#[case(
    Some(GuiHomeRowStateFrame::Active { progress: None }),
    "var(--ef-fg)",
    None,
    Some("unplayed")
)]
fn a_rows_play_state_decides_its_title_tone_and_its_resume_run(
    #[case] state: Option<GuiHomeRowStateFrame>,
    #[case] tone: &str,
    #[case] width: Option<&str>,
    #[case] mark: Option<&str>,
) {
    let doc = built(vec![row_with(state)], false);
    let title = style_of(
        &doc,
        &format!("[data-library-track=\"{ROW_ID}\"] [data-library-track-name] div.truncate"),
    )
    .expect("the row paints its title");

    assert!(
        title.contains(&format!("color: {tone}")),
        "the title paints {tone}: {title}"
    );
    assert_eq!(
        run_width(&doc).as_deref(),
        width,
        "the resume run paints at the row's carried percent"
    );
    assert_eq!(
        play_mark_value(&doc).as_deref(),
        mark,
        "the trailing cluster paints the row's play mark"
    );
}

/// Contract (gui-played-row-marks design D4, gui-hero-design-polish design D6,
/// the `gui-client` spec's "The playing Workspace row keeps its own mark"): a
/// playing row keeps its heading tone and weight 600 whatever state it carries,
/// paints the bars mark in its number cell with no glyph beside its title, and
/// paints the mark's empty slot, so its duration stays aligned with the other
/// rows.
#[test]
fn a_playing_row_keeps_its_heading_tone_and_paints_no_play_mark() {
    let doc = built(
        vec![WorkspaceRow {
            playing: true,
            ..row_with(Some(GuiHomeRowStateFrame::Played))
        }],
        false,
    );
    let title = style_of(
        &doc,
        &format!("[data-library-track=\"{ROW_ID}\"] [data-library-track-name] div.truncate"),
    )
    .expect("the row paints its title");

    assert!(
        title.contains("color: var(--canvas-heading)"),
        "the playing row keeps its heading tone: {title}"
    );
    assert!(
        title.contains("font-weight: 600"),
        "the playing row's title weighs 600: {title}"
    );
    let dom = doc.inner();
    assert!(
        dom.query_selector(&format!(
            "[data-library-track=\"{ROW_ID}\"] [data-library-track-number] svg.playing-bars"
        ))
        .expect("the selector is valid")
        .is_some(),
        "the playing row paints the bars mark in its number cell"
    );
    assert!(
        dom.query_selector(&format!(
            "[data-library-track=\"{ROW_ID}\"] [data-library-track-glyph]"
        ))
        .expect("the selector is valid")
        .is_none(),
        "the playing row paints no glyph beside its title"
    );
    assert_eq!(
        play_mark_value(&doc).as_deref(),
        Some("none"),
        "the playing row paints the mark's empty slot"
    );
}

/// Contract (design D5): the merged trailing cell holds the row's duration as
/// the trail the hover hides, and the controls paint over it — Play always, Add
/// only where the destination passes `on_queue`.
#[rstest]
#[case(true, &["play", "queue"])]
#[case(false, &["play"])]
fn the_trailing_cell_holds_the_duration_and_the_offered_controls(
    #[case] with_queue: bool,
    #[case] expected: &[&str],
) {
    let doc = built(vec![row()], with_queue);

    assert_eq!(actions(&doc), expected);
    let dom = doc.inner();
    assert!(
        dom.query_selector(&format!(
            "[data-library-track=\"{ROW_ID}\"] [data-library-track-duration].row-hover-trail"
        ))
        .expect("the selector is valid")
        .is_some(),
        "the trailing cell paints the row's duration as its hover trail"
    );
}

/// Contract (`gui-pointer-target-interaction` 3.3): the table names its list —
/// the tracklist or a Workspace — and every row names its own id, the pair a
/// key's pointer target reads.
#[test]
fn the_table_and_its_rows_carry_the_pointer_markers() {
    let doc = built(vec![row()], false);
    let dom = doc.inner();
    let list = dom
        .query_selector("[data-pointer-list]")
        .expect("the selector is valid")
        .expect("the table carries the list marker");

    assert_eq!(
        dom.get_node(list)
            .expect("the node is in the document")
            .attr(LocalName::from("data-pointer-list")),
        Some("tracklist")
    );
    let row_node = dom
        .query_selector("[data-pointer-row]")
        .expect("the selector is valid")
        .expect("the row carries its marker");
    assert_eq!(
        dom.get_node(row_node)
            .expect("the node is in the document")
            .attr(LocalName::from("data-pointer-row")),
        Some("t1")
    );
}
