//! Tests of the launch copy (D5): the window attributes, and the resolve that
//! keeps hit testing off a removed node (gap `G-Hit`). No test opens a window,
//! sleeps, or reaches the core.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use blitz_dom::Document as _;
use blitz_traits::events::{
    BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, Point, PointerCoords,
    PointerDetails, UiEvent,
};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::winit::dpi::{LogicalSize, Size};
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::window::app::ROOT_ELEMENT_ID;
use crate::window::input::f32_from_f64;
use crate::window::pointer_target::{PointerList, PointerTarget, PointerTargetStore, PointerZone};

use super::{
    AnimationClock, carries_unresolved_change, focus_within, resolve_before_hit_testing,
    restore_root_focus, window_attributes,
};
/// Contract: the window keeps the earlier window's 400x200 logical floor, so a
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
    pointer(BlitzPointerKind::Down, x, y, MouseEventButtons::Primary)
}

/// The clock a resolve passes in a window whose surfaces exist: the anchor
/// `can_create_surfaces` takes after the shell created them.
fn resolved_clock() -> AnimationClock {
    let mut animation = AnimationClock::default();
    animation.shell_started();
    animation
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
    resolve_before_hit_testing(&doc.inner, &resolved_clock());
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
    resolve_before_hit_testing(&doc.inner, &resolved_clock());
    doc.handle_ui_event(pointer_down(10.0, 10.0));
}

/// The layout and transition the clock tests resolve with, in place of the app
/// stylesheet: a box that widens from `BAR_WIDTH` to `BAR_WIDE` over 120 ms, so
/// a resolve's animation time is readable as the box's width.
const TRANSITION_CSS: &str =
    "[data-bar] { height: 10px; width: 20px; transition: width 120ms linear; }";

/// The bar's width before the press, in CSS pixels.
const BAR_WIDTH: f64 = 20.0;

/// The bar's width after the press, in CSS pixels.
const BAR_WIDE: f64 = 200.0;

/// The animation time of the frame resolve that follows the pre-hit resolve,
/// in seconds: well past the 120 ms transition, so a transition that started
/// before it has surely progressed.
const FRAME_TIME: f64 = 0.2;

/// The root: the bar, and a press that widens it — a style change whose
/// transition a resolve has to start.
fn bar_app() -> Element {
    let mut wide = use_signal(|| false);
    rsx! {
        div {
            "data-bar": "",
            style: if wide() { "width: 200px" } else { "width: 20px" },
            onpointerdown: move |_| wide.set(true),
        }
    }
}

/// The bar's client rect, in CSS pixels.
fn bar_rect(doc: &DioxusDocument, bar: blitz_dom::NodeId) -> blitz_dom::BoundingRect {
    doc.inner()
        .get_client_bounding_rect(bar)
        .expect("the bar is laid out")
}

/// The bar's laid-out width, in CSS pixels: the transition's progress.
fn bar_width(doc: &DioxusDocument, bar: blitz_dom::NodeId) -> f64 {
    bar_rect(doc, bar).width
}

/// The transition fixture: the bar, resolved at t=0 as the shell's first frame
/// would leave it, with the bar's node id.
fn bar_document() -> (DioxusDocument, blitz_dom::NodeId) {
    let mut doc = DioxusDocument::new(
        VirtualDom::new(bar_app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(TRANSITION_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.0);
    let bar = doc
        .inner()
        .query_selector("[data-bar]")
        .expect("the selector is valid")
        .expect("the bar paints");
    (doc, bar)
}

/// Press the bar's centre, which widens it, and let the change land.
fn press_bar(doc: &mut DioxusDocument, bar: blitz_dom::NodeId) {
    let rect = bar_rect(doc, bar);
    let x = f32_from_f64(rect.x + rect.width / 2.0);
    let y = f32_from_f64(rect.y + rect.height / 2.0);
    doc.handle_ui_event(pointer_down(x, y));
    doc.poll(None);
}

/// Contract (gap `G-Hit`, the pre-hit resolve's animation clock): a transition
/// the pre-hit resolve starts follows its own time, so the frame that comes
/// after it is not frozen. Stylo dates a new transition from the resolve time
/// it is handed and clamps the progress to zero while a later resolve's time is
/// before that date, so the pre-hit clock must trail the shell's — which it
/// does, anchored after `View::resume` and not in `launch`.
#[test]
fn a_transition_the_pre_hit_resolve_starts_is_not_frozen() {
    let (mut doc, bar) = bar_document();
    press_bar(&mut doc, bar);

    resolve_before_hit_testing(&doc.inner, &resolved_clock());
    doc.inner.borrow_mut().resolve(FRAME_TIME);

    assert_eq!(
        bar_width(&doc, bar),
        BAR_WIDE,
        "the transition the pre-hit resolve started ran to its end by the frame"
    );
}

/// Contract (gap `G-Hit`): a pre-hit clock that runs *ahead* of the shell's
/// freezes the transition it starts until the frame's time passes that start,
/// which is the defect the anchor removes. The two times below are in the order
/// the launch copy would pass them if it anchored its clock before the
/// window's own, with the drift exaggerated so the frozen progress is exact.
#[test]
fn a_pre_hit_clock_ahead_of_the_frame_freezes_its_transition() {
    let (mut doc, bar) = bar_document();
    press_bar(&mut doc, bar);

    doc.inner.borrow_mut().resolve(FRAME_TIME + 1.0);
    doc.inner.borrow_mut().resolve(FRAME_TIME);

    assert_eq!(bar_width(&doc, bar), BAR_WIDTH);
}

/// The window's root, as `window::app` paints it: the element that carries the
/// window's one `keydown` handler, with a queue row as a `button` beside a text
/// field.
fn root_app() -> Element {
    rsx! {
        main {
            id: "{ROOT_ELEMENT_ID}",
            tabindex: 0,
            onkeydown: move |_| {},
            input { "data-field": "", r#type: "text" }
            button { "type": "button", "data-row": "", "row" }
        }
    }
}

/// The root fixture, built and resolved, with the root element's node id.
/// Nothing focused yet.
fn root_document() -> (DioxusDocument, blitz_dom::NodeId) {
    let mut doc = DioxusDocument::new(
        VirtualDom::new(root_app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![blitz_dom::UserAgentStylesheet::default_sheet()],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    doc.inner.borrow_mut().resolve(0.0);
    let root = doc
        .inner()
        .get_element_by_id(ROOT_ELEMENT_ID)
        .expect("the root element carries the id");
    (doc, root)
}

/// A node the selector names, in the root fixture.
fn node_of(doc: &DioxusDocument, selector: &str) -> blitz_dom::NodeId {
    doc.inner()
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("the fixture paints {selector}"))
}

/// The pointer event the shell builds for `x`/`y` in client coordinates, with
/// the button `buttons` says is held.
fn pointer(kind: BlitzPointerKind, x: f32, y: f32, buttons: MouseEventButtons) -> UiEvent {
    let coords = PointerCoords {
        page_x: x,
        page_y: y,
        screen_x: x,
        screen_y: y,
        client_x: x,
        client_y: y,
    };
    let event = BlitzPointerEvent {
        id: BlitzPointerId::Mouse,
        is_primary: true,
        coords,
        button: MouseEventButton::Main,
        buttons,
        mods: Modifiers::default(),
        details: PointerDetails::default(),
        element: Point::default(),
        active_pointers: Arc::default(),
    };
    match kind {
        BlitzPointerKind::Move => UiEvent::PointerMove(event),
        BlitzPointerKind::Down => UiEvent::PointerDown(event),
        BlitzPointerKind::Up => UiEvent::PointerUp(event),
    }
}

/// The pointer kind one step of a press-and-release carries.
#[derive(Clone, Copy)]
enum BlitzPointerKind {
    Move,
    Down,
    Up,
}

/// One press and release on `node`, as the shell delivers a click.
fn click(doc: &mut DioxusDocument, node: blitz_dom::NodeId) {
    let rect = doc
        .inner()
        .get_client_bounding_rect(node)
        .expect("the node is laid out");
    let x = f32_from_f64(rect.x + rect.width / 2.0);
    let y = f32_from_f64(rect.y + rect.height / 2.0);
    doc.handle_ui_event(pointer(
        BlitzPointerKind::Move,
        x,
        y,
        MouseEventButtons::None,
    ));
    doc.handle_ui_event(pointer(
        BlitzPointerKind::Down,
        x,
        y,
        MouseEventButtons::Primary,
    ));
    doc.handle_ui_event(pointer(BlitzPointerKind::Up, x, y, MouseEventButtons::None));
}

/// Contract (D8): a click on a queue row does not cost the window its
/// keyboard. Blitz clears the document's DOM focus on a click that matches no
/// text input, checkbox, radio, summary, label or link (`handle_click`'s
/// `if !matched { clear_focus() }`), and a row is a plain `button`; the shell
/// then delivers a keydown to the document's `<html>` element, which carries no
/// Dioxus id, so no key reaches the window's handler — every key in the window
/// went dead after the first click on a row (live run of PR 1003). The launch
/// copy gives the root the focus back, so the next keydown reaches it.
#[test]
fn a_click_that_blitz_clears_the_focus_of_leaves_the_window_root_focused() {
    let (mut doc, root) = root_document();
    doc.inner.borrow_mut().set_focus_to(root);
    let row = node_of(&doc, "[data-row]");

    click(&mut doc, row);

    assert!(
        !focus_within(&doc.inner(), root),
        "a row click leaves the DOM focus outside the window's root, where no keydown reaches it"
    );
    restore_root_focus(&mut doc.inner.borrow_mut());
    assert_eq!(
        doc.inner().get_focussed_node_id(),
        Some(root),
        "the root takes the keyboard back"
    );
}

/// Contract (D8): the restore fills an empty focus only. A text input that took
/// the focus keeps it, so an open field still types.
#[test]
fn the_restore_keeps_the_focus_a_text_input_took() {
    let (doc, _root) = root_document();
    let field = node_of(&doc, "[data-field]");
    doc.inner.borrow_mut().set_focus_to(field);

    restore_root_focus(&mut doc.inner.borrow_mut());

    assert_eq!(
        doc.inner().get_focussed_node_id(),
        Some(field),
        "the field keeps the keyboard"
    );
}

/// The store test's props: the handle the root provides its signal through,
/// and the render count that witnesses each signal write.
#[derive(Clone)]
struct StoreProps {
    store: PointerTargetStore,
    renders: Rc<Cell<usize>>,
}

/// The root: it provides its pointer-target signal through the store, as the
/// app root does, and counts its renders — a render follows each signal write,
/// an equal value included, so the count is the write witness.
fn store_app(props: StoreProps) -> Element {
    let store = props.store;
    let target = use_signal(PointerTarget::default);
    provide_context(target);
    use_hook(move || store.provide(target));
    props.renders.set(props.renders.get() + 1);
    let zone = target.cloned().zone.map_or("none", PointerZone::token);
    rsx! {
        div { "data-zone": "{zone}" }
    }
}

/// The document the store test writes into, built and idle.
fn store_document(store: PointerTargetStore, renders: Rc<Cell<usize>>) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(store_app, StoreProps { store, renders }),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    doc
}

/// Poll the document until its pending renders and effects have landed (a
/// fixed poll count with a tripwire: never an unbounded wait).
fn poll_until_idle(doc: &mut DioxusDocument) {
    for _ in 0..8 {
        if !doc.poll(None) {
            return;
        }
    }
    panic!("document did not become idle");
}

/// Contract (`gui-pointer-target-interaction` 3.2): the launch copy writes the
/// pointer target from the vdom's runtime when it changed, and writes nothing
/// when it did not — a signal write marks its subscribers, an equal value
/// included, so an unchanged hover must cost no render.
#[test]
fn a_pointer_target_writes_only_when_it_changes() {
    let store = PointerTargetStore::new();
    let renders = Rc::new(Cell::new(0));
    let mut doc = store_document(store.clone(), Rc::clone(&renders));
    let settled = renders.get();

    let target = PointerTarget {
        zone: Some(PointerZone::Library),
        list: Some(PointerList::Music),
        row: Some("artist-1".to_owned()),
    };
    store.set(&doc.vdom, target.clone());
    poll_until_idle(&mut doc);
    assert_eq!(renders.get(), settled + 1, "a changed target writes once");

    store.set(&doc.vdom, target);
    poll_until_idle(&mut doc);
    assert_eq!(
        renders.get(),
        settled + 1,
        "an unchanged target writes nothing"
    );
}
