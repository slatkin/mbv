//! Headless tests of the Workspace table's hover swap (D16): the port
//! review's regression test — off-hover the row's play control hides and the
//! number paints, and the hovered row swaps them, exactly as the Svelte
//! ternaries did. A headless document cannot fire a pointerenter, so the
//! hovered half asserts the one style decision both cells route through. No
//! test opens a window, sleeps, or reaches the core.

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{LibraryWorkspaceTable, WorkspaceRow, hover_styles};

/// The harness's props: the rows the table paints.
#[derive(Clone)]
struct FixtureProps {
    rows: Vec<WorkspaceRow>,
}

/// The root: paints the table, as the Heroes mount it.
fn table_app(props: FixtureProps) -> Element {
    rsx! {
        LibraryWorkspaceTable {
            root: "data-library-tracklist",
            label: "Tracklist",
            hook: "track",
            noun: "Track",
            rows: props.rows,
            on_press: Callback::new(|_: String| {}),
            on_play: Callback::new(|_: String| {}),
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
}

impl Fixture {
    /// Poll the document until the first render has landed (a fixed poll
    /// count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// The style attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(LocalName::from("style"))
            .unwrap_or_default()
            .to_owned()
    }
}

/// Build the document over the rows and let the first render land.
fn built(rows: Vec<WorkspaceRow>) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(table_app, FixtureProps { rows }),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

/// Regression test (the port review's hidden-control finding): off-hover the
/// row's play control is `display: none` and the number paints with no
/// display style, so the control swaps for the number instead of painting
/// beside it; the hovered row swaps them.
#[test]
fn the_hover_swaps_the_number_for_the_play_control() {
    let fixture = built(vec![WorkspaceRow {
        id: "t1".to_owned(),
        number: "1".to_owned(),
        name: "Rhymin & Stealin".to_owned(),
        duration: "3:27".to_owned(),
        playing: false,
        state: None,
        progress: None,
    }]);

    let control_style = fixture.style_of("[data-library-track-action=\"play\"]");
    assert!(
        control_style.contains("display: none"),
        "the play control hides off-hover, painted: {control_style}"
    );
    let number_style = fixture.style_of("[data-library-track-number]");
    assert!(
        !number_style.contains("display"),
        "the number paints off-hover with no display style, painted: {number_style}"
    );
    assert_eq!(
        hover_styles(true),
        ("display: none;", "display: flex;"),
        "the hovered row hides the number and shows the control"
    );
}
