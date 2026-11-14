//! Tests of the launch copy (D5): the window attributes, and the resolve that
//! keeps hit testing off a removed node (gap `G-Hit`). No test opens a window,
//! sleeps, or reaches the core.

use std::sync::Arc;
use std::time::Instant;

use blitz_dom::Document as _;
use blitz_traits::events::{
    BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, Point, PointerCoords,
    PointerDetails, UiEvent,
};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::winit::dpi::{LogicalSize, Size};
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{carries_unresolved_change, resolve_before_hit_testing, window_attributes};

/// Contract: the window keeps the Tauri window's 400x200 logical floor, so a
/// floating window cannot be dragged below the size at which the compact
/// strip lays out.
#[test]
fn the_window_carries_a_400_by_200_logical_minimum() {
    let attributes = window_attributes();

    assert_eq!(
        attributes.min_surface_size,
        Some(Size::Logical(LogicalSize::new(400.0, 200.0)))
    );
}

/// The layout rules the fixture resolves with, in place of the app stylesheet:
/// a fixed-height row inside a fixed-height container, so the press point below
/// lands on the first row.
const FIXTURE_CSS: &str = concat!(
    "[data-rows] { height: 100px; }",
    "[data-row] { height: 20px; }",
);

/// The root: three rows, and a press that rebuilds them away — the queue row's
/// double-click, whose play rebuilds the rows (gap `G-Hit`).
fn rows_app() -> Element {
    let mut rows = use_signal(|| 3_usize);
    rsx! {
        div {
            "data-rows": "",
            for index in 0..rows() {
                div {
                    key: "{index}",
                    "data-row": "",
                    onpointerdown: move |_| rows.set(0),
                    "row {index}"
                }
            }
        }
    }
}

/// A pointer press at `x`/`y` in the document's client coordinates, as the
/// shell builds one from a winit pointer button.
fn pointer_down(x: f32, y: f32) -> UiEvent {
    let coords = PointerCoords {
        page_x: x,
        page_y: y,
        screen_x: x,
        screen_y: y,
        client_x: x,
        client_y: y,
    };
    UiEvent::PointerDown(BlitzPointerEvent {
        id: BlitzPointerId::Mouse,
        is_primary: true,
        coords,
        button: MouseEventButton::Main,
        buttons: MouseEventButtons::Primary,
        mods: Modifiers::default(),
        details: PointerDetails::default(),
        element: Point::default(),
        active_pointers: Arc::default(),
    })
}

/// Whether the selector names a node in the document.
fn presents(doc: &DioxusDocument, selector: &str) -> bool {
    matches!(doc.inner().query_selector(selector), Ok(Some(_)))
}

/// The fixture document: the rows, built and resolved as the first frame
/// would leave them.
fn rows_document() -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new(rows_app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    resolve_before_hit_testing(&doc.inner, Instant::now());
    doc
}

/// Contract: the resolve leaves the document with no unresolved change, so the
/// gate in front of the next pointer event stays closed and an unchanged tree
/// costs one flag read, not a resolve (gap `G-Hit`).
#[test]
fn a_resolved_document_carries_no_unresolved_change() {
    let doc = rows_document();

    assert!(!carries_unresolved_change(&doc.inner.borrow()));
}

/// Contract: a pointer event after a DOM change does not panic (gap `G-Hit`;
/// the live run's double-click on a queue row, whose play rebuilt the rows).
/// `Node::paint_children` refreshes only at resolve, so the event that follows
/// the change must find a resolved document.
#[test]
fn a_pointer_event_after_a_dom_change_does_not_panic() {
    let mut doc = rows_document();
    assert!(
        presents(&doc, "[data-row]"),
        "the rows paint before the press"
    );

    doc.handle_ui_event(pointer_down(10.0, 10.0));
    doc.poll(None);

    assert!(
        !presents(&doc, "[data-row]"),
        "the press rebuilt the rows away"
    );
    resolve_before_hit_testing(&doc.inner, Instant::now());
    doc.handle_ui_event(pointer_down(10.0, 10.0));
}
