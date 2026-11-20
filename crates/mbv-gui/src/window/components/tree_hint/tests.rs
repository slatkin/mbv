//! Headless tests of the tree rows' hover hint (`gui-tree-double-click`
//! design D3): the span's text names the gesture the node's open state answers,
//! and it carries the `data-tree-hint` mark the row tests read. The hover
//! itself is CSS, so no test checks it here. No test opens a window, sleeps,
//! or reaches the core.

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::TreeHint;

/// The hint over one open state, mounted once per case.
fn painted(open: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            |open: bool| {
                rsx! {
                    TreeHint { open }
                }
            },
            open,
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// The text the hinted span paints.
fn hint_of(doc: &DioxusDocument) -> String {
    let dom = doc.inner();
    let id = dom
        .query_selector("[data-tree-hint]")
        .expect("the selector is valid")
        .expect("the hint paints");
    dom.get_node(id)
        .expect("the node is in the document")
        .text_content()
}

/// Whether the hinted span carries the mark's attribute.
fn marked(doc: &DioxusDocument) -> bool {
    let dom = doc.inner();
    let id = dom
        .query_selector("[data-tree-hint]")
        .expect("the selector is valid")
        .expect("the hint paints");
    dom.get_node(id)
        .expect("the node is in the document")
        .attr(LocalName::from("data-tree-hint"))
        .is_some()
}

/// Contract (design D3): a closed node's hint asks for the expand gesture and
/// an open node's asks for the collapse gesture, and both carry the
/// `data-tree-hint` mark.
#[rstest]
#[case(false, "Double-click to expand")]
#[case(true, "Double-click to collapse")]
fn the_hint_names_the_gesture_its_open_state_answers(#[case] open: bool, #[case] text: &str) {
    let doc = painted(open);
    assert!(marked(&doc), "the hint carries its mark");
    assert_eq!(hint_of(&doc), text);
}
