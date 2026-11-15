//! Headless tests of the Settings row (D16): the label box paints the 14 px
//! face the label's measure cuts for — a box that paints a larger face
//! clips mid-glyph instead of ending in an ellipsis (design D12).

use blitz_dom::{Document as _, local_name};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::SettingsRow;

/// The row over a static control, mounted once.
fn painted(cursor: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            |(label, cursor): (String, bool)| {
                rsx! {
                    SettingsRow {
                        label,
                        cursor,
                        span { "control" }
                    }
                }
            },
            ("Auto play".to_owned(), cursor),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// Contract (design D12): the row's label box paints the 14 px face its
/// `TextStyle` measures with, so a cut ends in an ellipsis, not a clipped
/// glyph.
#[test]
fn the_label_box_paints_the_14px_face_its_measure_cuts_for() {
    let doc = painted(false);
    let dom = doc.inner();
    let id = dom
        .query_selector("[data-settings-setting-row] .truncate")
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("the label box paints"));
    let style = dom
        .get_node(id)
        .expect("the node is in the document")
        .attr(local_name!("style"))
        .unwrap_or_default();
    assert!(style.contains("font-size: 14px"), "{style:?}");
    assert!(style.contains("color: var(--canvas-heading)"), "{style:?}");
}
