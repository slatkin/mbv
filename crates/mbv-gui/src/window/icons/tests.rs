//! Headless tests of the icon shell's colours (the live run's icons that
//! painted black or with no stroke, `replace-tauri-with-blitz` §11). Blitz
//! paints an inline `svg` by serialising the subtree and re-parsing it with
//! usvg, and usvg cannot parse a CSS `var()` in `fill` or `stroke`: the stroke
//! is dropped and the fill falls back to black. The contracts here read the
//! tree the painter paints, so a `var()` that reaches an attribute fails.
//! Nothing opens a window or sleeps.

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{Check, Play};

/// The theme rule the fixture resolves with, in place of the app stylesheet
/// the window loads: two roles at rgb values the assertions name.
const THEME_CSS: &str = ":root { --mbv-textMuted: rgb(1, 2, 3); --mbv-accent: rgb(4, 5, 6); }";

/// An icon's colour, as rgb bytes.
type Rgb = (u8, u8, u8);

/// One painted path's fill and stroke, in that order.
type PathPaint = (Option<Rgb>, Option<Rgb>);

/// The root: a stroked icon in the muted role, and a filled one in the accent.
fn icon_app() -> Element {
    rsx! {
        Check { size: 16, color: "var(--mbv-textMuted)".to_owned() }
        Play {
            size: 16,
            color: "var(--mbv-accent)".to_owned(),
            fill: "var(--mbv-accent)".to_owned(),
        }
    }
}

/// The fixture document over the theme rule, resolved once so the svg
/// subtrees are constructed and parsed as the first frame would parse them.
fn painted() -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new(icon_app),
        DocumentConfig {
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(THEME_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.0);
    doc
}

/// A paint's colour.
fn rgb(paint: &usvg::Paint) -> Rgb {
    match paint {
        usvg::Paint::Color(color) => (color.red, color.green, color.blue),
        _ => panic!("the icon paints in one flat colour"),
    }
}

/// The fill and the stroke of every path in the svg the selector names, in
/// document order: the `usvg` tree Blitz parsed from the serialised subtree,
/// which is what the painter paints.
fn path_paints(doc: &DioxusDocument, selector: &str) -> Vec<PathPaint> {
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
    let mut paints = Vec::new();
    collect_paints(tree.root(), &mut paints);
    paints
}

/// The fill and stroke of every path of `group` and its nested groups, in
/// document order: usvg groups the view box transform's children.
fn collect_paints(group: &usvg::Group, paints: &mut Vec<PathPaint>) {
    for child in group.children() {
        match child {
            usvg::Node::Path(path) => paints.push((
                path.fill().map(|fill| rgb(fill.paint())),
                path.stroke().map(|stroke| rgb(stroke.paint())),
            )),
            usvg::Node::Group(nested) => collect_paints(nested, paints),
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

/// Contract: an icon strokes in the role colour its caller passes, and keeps
/// Lucide's unfilled body. A `var()` in the `stroke` attribute leaves the
/// icon with no stroke at all.
#[test]
fn an_icon_strokes_in_the_role_its_caller_names() {
    let doc = painted();

    assert_eq!(
        path_paints(&doc, "svg.lucide-check"),
        vec![(None, Some((1, 2, 3)))]
    );
}

/// Contract: an icon whose caller names a `fill` paints its body in the same
/// role as its stroke. A `var()` in the `fill` attribute paints the body
/// black.
#[test]
fn an_icon_fills_its_body_in_that_roles_colour() {
    let doc = painted();

    assert_eq!(
        path_paints(&doc, "svg.lucide-play"),
        vec![(Some((4, 5, 6)), Some((4, 5, 6)))]
    );
}
