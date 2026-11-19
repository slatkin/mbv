//! Headless tests of the floating toast stack (design D7 of
//! `gui-floating-toasts`): the cards the `toasts` section carries paint in
//! arrival order with their severities, and a section with no live toast paints
//! no card. The CSS placement, the pointer behaviour, and the paint over the
//! panels are the window's manual checks; these tests read the painted tree.
//! No test opens a window or sleeps.

use std::cell::RefCell;
use std::rc::Rc;

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::ToastStack;
use crate::bridge::frame::chrome::{GuiToast, GuiToastSeverity, GuiToasts};
use crate::window::sections::Sections;
use mbv_ui_model::toast::ToastId;

#[derive(Clone)]
struct FixtureProps {
    /// The slot the root hands the section stores back out through, so a test
    /// writes the `toasts` section the way the core's batch would.
    handle: Rc<RefCell<Option<Sections>>>,
}

/// The root: provides the section stores and paints the stack, as the window
/// root does.
fn stack_app(props: FixtureProps) -> Element {
    let FixtureProps { handle } = props;
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    handle.borrow_mut().replace(sections);
    rsx! {
        ToastStack {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
}

impl Fixture {
    /// The nodes the selector names, in tree order.
    fn all(&self, selector: &str) -> Vec<blitz_dom::NodeId> {
        let dom = self.doc.inner();
        dom.query_selector_all(selector)
            .expect("the selector is valid")
            .into_iter()
            .collect()
    }

    /// The text of the node the selector names, at the given position in tree
    /// order.
    fn text_of(&self, selector: &str, position: usize) -> String {
        let nodes = self.all(selector);
        let id = nodes.get(position).unwrap_or_else(|| {
            panic!("{selector} paints at least {} node(s)", position + 1);
        });
        let dom = self.doc.inner();
        dom.get_node(*id)
            .expect("the node is in the document")
            .text_content()
    }

    /// Write the section's live toasts, the way the core's batch does, and let
    /// the re-render land.
    fn show(&mut self, toasts: GuiToasts) {
        let mut sections = self.sections.clone();
        self.doc
            .vdom
            .in_runtime(move || sections.toasts.set(toasts));
        self.poll_until_idle();
    }

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
}

/// A stack over an empty `toasts` section.
fn built() -> Fixture {
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            stack_app,
            FixtureProps {
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let sections = handle
        .borrow()
        .clone()
        .expect("the root provides the sections");
    let mut fixture = Fixture { doc, sections };
    fixture.poll_until_idle();
    fixture
}

/// A live toast: its id, text, and severity.
fn toast(id: u64, text: &str, severity: GuiToastSeverity) -> GuiToast {
    GuiToast {
        id: ToastId(id),
        text: text.to_owned(),
        severity,
    }
}

/// Contract (spec `gui-toasts`): two live toasts paint two cards in arrival
/// order — the oldest first, so the newest card is the bottom one — each with
/// its own severity.
#[test]
fn two_live_toasts_paint_two_cards_in_arrival_order() {
    let mut fixture = built();
    fixture.show(GuiToasts(vec![
        toast(7, "Saved the queue", GuiToastSeverity::Success),
        toast(8, "Setting refused", GuiToastSeverity::Warning),
    ]));

    assert_eq!(fixture.all("[data-toast-card]").len(), 2);
    assert_eq!(fixture.text_of("[data-toast-card]", 0), "Saved the queue");
    assert_eq!(fixture.text_of("[data-toast-card]", 1), "Setting refused");
    assert_eq!(
        fixture.text_of("[data-toast-card][data-toast-severity=\"success\"]", 0),
        "Saved the queue"
    );
    assert_eq!(
        fixture.text_of("[data-toast-card][data-toast-severity=\"warning\"]", 0),
        "Setting refused"
    );
}

/// Contract (spec `gui-toasts`): with no live toast the stack paints nothing.
#[test]
fn no_live_toast_paints_no_card() {
    let mut fixture = built();
    fixture.show(GuiToasts::default());

    assert_eq!(fixture.all("[data-toast-card]").len(), 0);
    assert_eq!(fixture.all("[data-toast-stack]").len(), 0);
}
