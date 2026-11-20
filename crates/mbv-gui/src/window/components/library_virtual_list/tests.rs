//! Headless tests of the scroll shell (D16): the two seams the former client
//! component owned and no `.test.ts` covered directly — the spacers that
//! hold the scroll range of the rows outside the window, and the scroller
//! that mirrors the scroll store's offset (design D9, D15). The windowed
//! paint itself is the Library list's contract
//! (`library_list/tests.rs`). The core channel is a mock pair; no test opens
//! a window, sleeps, or reaches the core.

use std::cell::RefCell;
use std::rc::Rc;

use blitz_dom::{Document as _, LocalName, local_name};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::library_virtual_list;
use crate::window::layout::LayoutEpoch;
use crate::window::library::list::GeometryRow;
use crate::window::library::scroll::ListScroll;
use crate::window::pointer_target::PointerList;

/// The context handles the root hands the test: the layout epoch, the scroll
/// offset the root paints with, the rows the shell painted last, and the row
/// art sets it reported.
type FixtureHandles = (
    LayoutEpoch,
    Signal<f32>,
    Rc<RefCell<Vec<String>>>,
    Rc<RefCell<Vec<Vec<String>>>>,
);

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the shell's utilities, the fixed
/// 100 px scroller box that makes the 300 px of rows scrollable, and the
/// fixed 30 px row box. Without the row height a bare text row measures by
/// font metrics: the CI container has no fonts, so the rows lay out 0 px
/// tall, the content shrinks to the spacers, and the scroller clamps the
/// store's offset.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".min-h-0 { min-height: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    "[data-library-list] { height: 100px; }",
    "[data-row] { height: 30px; }",
);

/// One fixture row: `height` tall at `top`, named `id`.
#[derive(Debug, Clone, PartialEq)]
struct TestRow {
    id: String,
    top: f32,
    height: f32,
}

impl GeometryRow for TestRow {
    fn id(&self) -> &str {
        &self.id
    }

    fn top(&self) -> f32 {
        self.top
    }

    fn height(&self) -> f32 {
        self.height
    }
}

/// The harness's props: ten 30 px rows and the handle the test reads
/// through.
#[derive(Clone)]
struct FixtureProps {
    rows: std::sync::Arc<[TestRow]>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: provides the layout epoch and paints the shell over ten rows,
/// recording every row the shell painted and every row art set it reported.
fn list_app(props: FixtureProps) -> Element {
    let FixtureProps { rows, handle } = props;
    let epoch = use_hook(LayoutEpoch::new);
    provide_context(epoch);
    let scroll_top = use_signal(|| 0.0);
    let painted = use_hook(|| Rc::new(RefCell::new(Vec::new())));
    let reported = use_hook(|| Rc::new(RefCell::new(Vec::new())));
    handle
        .borrow_mut()
        .replace((epoch, scroll_top, Rc::clone(&painted), Rc::clone(&reported)));
    let reported_ids = Rc::clone(&reported);
    let list = library_virtual_list(
        rows.as_ref(),
        ListScroll {
            scroll_top: scroll_top(),
            viewport_height: 100.0,
        },
        "",
        PointerList::Music,
        "8px 8px 8px 8px",
        Callback::new(move |_| {}),
        Callback::new(move |_| {}),
        Some(Callback::new(move |ids: Vec<String>| {
            reported_ids.borrow_mut().push(ids);
        })),
        move |row: &TestRow| {
            painted.borrow_mut().push(row.id.clone());
            rsx! {
                div { "data-row": "{row.id}", "{row.id}" }
            }
        },
    );
    rsx! {
        {list}
    }
}

struct Fixture {
    doc: DioxusDocument,
    epoch: LayoutEpoch,
    scroll_top: Signal<f32>,
    painted: Rc<RefCell<Vec<String>>>,
    reported: Rc<RefCell<Vec<Vec<String>>>>,
}

impl Fixture {
    /// Poll the document until the effects have run and their re-renders
    /// landed (a fixed poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }

    /// Run `f` inside the virtual dom's runtime, so the signal writes mark
    /// their subscribers, and let the re-renders land.
    fn in_runtime(&mut self, f: impl FnOnce() + 'static) {
        self.doc.vdom.in_runtime(f);
        self.poll_until_idle();
    }

    /// Resolve the document's style and layout, as the renderer's frame
    /// would, and let the re-measures the epoch bump drives land.
    fn lay_out(&mut self) {
        self.doc.inner.borrow_mut().resolve(0.0);
        let mut epoch = self.epoch;
        self.doc.vdom.in_runtime(move || epoch.bump());
        self.poll_until_idle();
    }

    /// The `style` attribute of every node the selector names, in paint
    /// order.
    fn styles_of(&self, selector: &str) -> Vec<String> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .filter_map(|id| {
                dom.get_node(id)
                    .expect("the node is in the document")
                    .attr(local_name!("style"))
                    .map(str::to_owned)
            })
            .collect()
    }
}

/// Build the document over ten 30 px rows and let its first render, effects,
/// and layout land.
fn built() -> Fixture {
    let rows: Vec<TestRow> = (0..10u16)
        .map(|index| TestRow {
            id: format!("row-{index}"),
            top: f32::from(index) * 30.0,
            height: 30.0,
        })
        .collect();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            list_app,
            FixtureProps {
                rows: rows.into(),
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let (epoch, scroll_top, painted, reported) = handle
        .borrow_mut()
        .take()
        .expect("the root handed the handles over");
    let mut fixture = Fixture {
        doc,
        epoch,
        scroll_top,
        painted,
        reported,
    };
    fixture.poll_until_idle();
    fixture.lay_out();
    fixture
}

/// The shell's own contract (design D7): the spacers hold the scroll range
/// of the rows outside the window, so the native scroll still spans the
/// whole list while only the rows in view paint.
#[test]
fn the_spacers_hold_the_scroll_range_of_the_rows_outside_the_window() {
    let mut fixture = built();
    // The first render's painted rows are the scroll-zero window's; the
    // assertion reads the window the scrolled offset paints.
    fixture.painted.borrow_mut().clear();
    let mut scroll_top = fixture.scroll_top;
    fixture.in_runtime(move || scroll_top.set(45.0));

    assert_eq!(
        *fixture.painted.borrow(),
        ["row-1", "row-2", "row-3", "row-4"],
        "only the rows in view paint"
    );
    let spacers = fixture.styles_of("li[aria-hidden=\"true\"]");
    assert_eq!(spacers.len(), 2, "the two spacers paint");
    assert!(
        spacers[0].contains("height: 30px"),
        "the above spacer holds the first painted row's top edge: {:?}",
        spacers[0]
    );
    assert!(
        spacers[1].contains("height: 150px"),
        "the below spacer holds the rows past the window's bottom edge: {:?}",
        spacers[1]
    );
}

/// The shell's own contract (design D9, D15): the scroller mirrors the
/// scroll store's offset — a store offset the element does not hold yet
/// scrolls the element.
#[test]
fn the_scroller_mirrors_the_scroll_stores_offset() {
    let mut fixture = built();
    let mut scroll_top = fixture.scroll_top;
    fixture.in_runtime(move || scroll_top.set(100.0));

    let dom = fixture.doc.inner();
    let id = dom
        .query_selector("[data-library-list]")
        .expect("the selector is valid")
        .expect("the list paints");
    let offset = *dom
        .get_node(id)
        .expect("the node is in the document")
        .scroll_offset();
    assert_eq!(offset.y, 100.0);
}

/// Contract (`gui-pointer-target-interaction` 3.3): the scroller names the
/// list kind it rolls, and every painted row names its own id — the pair a
/// key's pointer target reads.
#[test]
fn the_scroller_and_its_rows_carry_the_pointer_markers() {
    let fixture = built();
    let dom = fixture.doc.inner();
    let list = dom
        .query_selector("[data-pointer-list]")
        .expect("the selector is valid")
        .expect("the scroller carries the list marker");

    assert_eq!(
        dom.get_node(list)
            .expect("the node is in the document")
            .attr(LocalName::from("data-pointer-list")),
        Some("music")
    );
    let rows: Vec<String> = dom
        .query_selector_all("[data-pointer-row]")
        .expect("the selector is valid")
        .into_iter()
        .filter_map(|id| {
            dom.get_node(id)
                .expect("the node is in the document")
                .attr(LocalName::from("data-pointer-row"))
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(rows, ["row-0", "row-1", "row-2", "row-3"]);
}

/// Contract (gui-delineated-rows design D7): the shell reports the ids of the
/// rows it paints plus four rows of overscan either side, and a scroll reports
/// the new set.
#[test]
fn the_shell_reports_the_painted_rows_with_their_overscan() {
    let mut fixture = built();
    assert_eq!(
        fixture.reported.borrow()[0],
        [
            "row-0", "row-1", "row-2", "row-3", "row-4", "row-5", "row-6", "row-7"
        ],
        "the first window reports its rows plus the four below"
    );

    let mut scroll_top = fixture.scroll_top;
    fixture.in_runtime(move || scroll_top.set(60.0));

    assert_eq!(
        fixture.reported.borrow().last(),
        Some(&(0..10).map(|index| format!("row-{index}")).collect()),
        "the scrolled window reports the whole list's rows"
    );
}

/// Contract (gui-delineated-rows design D7): an unchanged row set reports
/// nothing, so a paint that moves no row art sends no command.
#[test]
fn an_unchanged_row_set_reports_nothing() {
    let mut fixture = built();
    fixture.reported.borrow_mut().clear();

    let mut scroll_top = fixture.scroll_top;
    fixture.in_runtime(move || scroll_top.set(45.0));
    let mut scrolled_top = fixture.scroll_top;
    fixture.in_runtime(move || scrolled_top.set(50.0));

    assert_eq!(
        fixture.reported.borrow().len(),
        1,
        "only the scroll that changed the row set reports it"
    );
}
