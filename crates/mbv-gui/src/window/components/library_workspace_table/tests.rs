//! Headless tests of the Workspace table's trailing cell (gui-row-hover-actions
//! design D5): the row's duration rests as the `row-hover-trail` the stylesheet
//! hides while the hover controls show, and the controls paint over it — Play
//! always, Add only where the destination passes `on_queue`. No test opens a
//! window, sleeps, or reaches the core.

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{LibraryWorkspaceTable, WorkspaceRow};
use crate::window::pointer_target::PointerList;

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
        id: "t1".to_owned(),
        number: "1".to_owned(),
        name: "Rhymin & Stealin".to_owned(),
        duration: "3:27".to_owned(),
        playing: false,
        state: None,
        progress: None,
    }
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
fn built(with_queue: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            table_app,
            FixtureProps {
                rows: vec![row()],
                with_queue,
            },
        ),
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
    dom.query_selector_all("[data-library-track=\"t1\"] [data-row-action]")
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
    let doc = built(with_queue);

    assert_eq!(actions(&doc), expected);
    let dom = doc.inner();
    assert!(
        dom.query_selector(
            "[data-library-track=\"t1\"] [data-library-track-duration].row-hover-trail"
        )
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
    let doc = built(false);
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
