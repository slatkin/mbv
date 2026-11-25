//! The clear button's paint contract: a named button carrying the Lucide X.
//! What a press does is each field's own empty-the-text path, owned by that
//! field's tests; no test opens a window.

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::ClearFieldButton;

/// The button over a no-op press, as a field paints it.
fn painted() -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new(|| {
            rsx! {
                ClearFieldButton {
                    label: "Clear filter",
                    on_clear: |()| {},
                }
            }
        }),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// Contract: the button is a named, non-submitting button that keeps out of
/// the tab order and paints the X glyph.
#[test]
fn the_button_paints_its_name_and_the_x_glyph() {
    let doc = painted();
    let dom = doc.inner();
    let id = dom
        .query_selector("button[data-field-clear]")
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("the button paints"));
    let node = dom.get_node(id).expect("the node is in the document");

    assert_eq!(
        node.attr(LocalName::from("aria-label")),
        Some("Clear filter")
    );
    assert_eq!(node.attr(LocalName::from("type")), Some("button"));
    assert_eq!(node.attr(LocalName::from("tabindex")), Some("-1"));
    assert!(
        matches!(dom.query_selector("button svg.lucide-x"), Ok(Some(_))),
        "the X glyph paints"
    );
}
