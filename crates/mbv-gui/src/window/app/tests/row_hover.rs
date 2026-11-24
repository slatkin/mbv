//! The Library rows' hover controls under the window's real stylesheet
//! (`gui-row-hover-actions` design D2): the stylesheet is the part that shows
//! the controls, covers the row's trailing end, and keeps the row reading as
//! one hovered row, so each contract here loads the committed sheet through the
//! root's own style element, lays the row out, and puts the pointer over the
//! row, as the window does. No test opens a window, sleeps, or reaches the core.

use blitz_dom::Document as _;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::window::components::list_row::{FlatRow, LibraryTrailPart};
use crate::window::input::f32_from_f64;

/// The row's own hook: the face the pointer rests on to hover the row.
const FACE: &str = "[data-library-row]";

/// The cover the row's hover controls paint behind them.
const COVER: &str = "[data-row-hover-actions]";

/// The Play control, the first of the controls the cover holds.
const PLAY: &str = "[data-row-action=\"play\"]";

/// One top-level tree parent with every control, selected, as the Music list
/// paints an open artist: it paints a divider only while it shows no child
/// rows, so `tree_guide` picks the divider the cover must stop above.
fn parent_row(tree_guide: bool) -> Element {
    let noop = Callback::new(|()| {});
    rsx! {
        {super::super::root_style_element()}
        div { style: "width: 600px;",
            FlatRow {
                attr: "",
                kind: "artist".to_owned(),
                id: "a1".to_owned(),
                title: "Beastie Boys".to_owned(),
                subtitle: None,
                trail: None,
                trail_part: LibraryTrailPart::Date,
                selected: true,
                tree_open: Some(false),
                tree_guide,
                on_press: noop,
                on_play: noop,
                on_toggle: Some(noop),
                on_hover_play: Some(noop),
                on_hover_queue: Some(noop),
                on_hover_shuffle: Some(noop),
            }
        }
    }
}

/// The row over the real stylesheet, laid out once.
fn built(tree_guide: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(parent_row, tree_guide),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![blitz_dom::UserAgentStylesheet::default_sheet()],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.0);
    doc
}

/// Put the pointer over the middle of `selector`'s box and restyle, as a
/// pointer move does.
fn hover(doc: &DioxusDocument, selector: &str) {
    let mut dom = doc.inner.borrow_mut();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    let rect = dom
        .get_client_bounding_rect(id)
        .unwrap_or_else(|| panic!("{selector} is laid out"));
    dom.set_hover_to(
        f32_from_f64(rect.x + rect.width / 2.0),
        f32_from_f64(rect.y + rect.height / 2.0),
    );
    dom.resolve(0.0);
}

/// The resolved value of `property` on `selector`'s element.
fn resolved(doc: &DioxusDocument, selector: &str, property: &str) -> String {
    let dom = doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    dom.resolved_style_value(id, property)
}

/// Contract (live run of PR 1062): the pointer over the row shows its controls,
/// on a top-level tree parent as on every other row, and the row's veil stays
/// on the row and on the controls' cover while the pointer moves from the row
/// onto a control — the row is still the hovered row, and its selection fill
/// still shows under the veil.
#[test]
fn a_hovered_parent_row_keeps_its_veil_when_the_pointer_moves_onto_a_control() {
    let doc = built(false);
    assert_eq!(
        resolved(&doc, COVER, "display"),
        "none",
        "the controls rest hidden"
    );

    hover(&doc, FACE);
    assert_eq!(
        resolved(&doc, COVER, "display"),
        "flex",
        "the pointer on the row shows the controls"
    );
    assert!(
        resolved(&doc, FACE, "background-image").starts_with("linear-gradient("),
        "the pointer on the row paints the veil on the row"
    );

    hover(&doc, PLAY);
    assert!(
        resolved(&doc, FACE, "background-image").starts_with("linear-gradient("),
        "the pointer on a control keeps the veil on the row"
    );
    assert!(
        resolved(&doc, COVER, "background-image").starts_with("linear-gradient("),
        "the pointer on a control keeps the veil on the cover"
    );
    assert_eq!(
        resolved(&doc, FACE, "background-color"),
        resolved(&doc, COVER, "background-color"),
        "the cover paints the row's own selection fill under the veil"
    );
}

/// Contract (live run of PR 1062): the cover stops above the divider the row
/// paints under itself, so its fill does not blank the line, and fills the whole
/// row where the row paints none.
#[rstest::rstest]
#[case::a_row_with_a_divider(false, "1px")]
#[case::a_row_that_shows_children_paints_none(true, "0px")]
fn the_cover_stops_above_the_rows_divider(#[case] tree_guide: bool, #[case] divider: &str) {
    let doc = built(tree_guide);
    hover(&doc, FACE);

    assert_eq!(resolved(&doc, COVER, "bottom"), divider);
}
