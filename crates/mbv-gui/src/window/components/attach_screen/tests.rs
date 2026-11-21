//! Headless tests of the attach screen (D16): the contracts of
//! `AttachScreen.test.ts` that the window's port keeps — every
//! `GuiAttachState` paints its text, the mismatch paints the Restart and
//! Quit buttons, and the mismatch buttons appear in the reported regions.
//! The core channel is a mock pair; no test opens a window, sleeps, or
//! reaches an Owner.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::Document as _;
use blitz_dom::traversal::TreeTraverser;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::AttachScreen;
use crate::bridge::frame::GuiAttachState;
use crate::bridge::layout::{AttachTargetId, RegionTarget};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::layout::region_set::last_reported_targets;
use crate::window::layout::{LayoutEpoch, use_layout_reporter};
use crate::window::sections::Sections;

/// The wait text the two wait states paint.
const WAITING_TEXT: &str = "Attaching to the Owner process\u{2026}";

/// What the root hands the test: the section stores.
type FixtureHandles = Sections;

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    handle: Rc<RefCell<Option<FixtureHandles>>>,
}

/// The root: creates the stores once, provides the reporter's contexts, and
/// paints the attach screen.
fn attach_app(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    provide_context(CoreLink::new(props.sender));
    let reporter = use_layout_reporter();
    provide_context(reporter);
    provide_context(LayoutEpoch::new());
    props.handle.borrow_mut().replace(sections);
    rsx! {
        AttachScreen {}
    }
}

struct Fixture {
    doc: DioxusDocument,
    sections: Sections,
    receiver: Receiver<CoreInput>,
}

impl Fixture {
    /// Write the attach section's value, as the drain task would.
    fn set_attach(&mut self, value: GuiAttachState) {
        let mut attach = self.sections.attach;
        self.doc.vdom.in_runtime(move || attach.set(Some(value)));
        self.poll_until_idle();
    }

    /// The whole text the document paints.
    fn text(&self) -> String {
        let dom = self.doc.inner();
        let mut out = String::new();
        for id in TreeTraverser::new(&dom) {
            if let Some(node) = dom.get_node(id) {
                out.push_str(&node.text_content());
                out.push('\n');
            }
        }
        out
    }

    /// Poll the document until the effects have run and their re-renders
    /// landed.
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }
}

fn built() -> Fixture {
    let (sender, receiver) = channel();
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            attach_app,
            FixtureProps {
                sender,
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let sections = handle
        .borrow_mut()
        .take()
        .expect("the root handed the stores over");
    let mut fixture = Fixture {
        doc,
        sections,
        receiver,
    };
    fixture.poll_until_idle();
    fixture
}

/// Contract (task 4.2): the wait before the first batch paints the waiting
/// state.
#[test]
fn no_frame_yet_renders_the_waiting_state() {
    let fixture = built();
    assert!(fixture.text().contains(WAITING_TEXT));
}

/// Contract (task 4.2): a shutting-down Owner paints the same waiting state.
#[test]
fn a_shutting_down_owner_renders_the_waiting_state() {
    let mut fixture = built();
    fixture.set_attach(GuiAttachState::Waiting);
    assert!(fixture.text().contains(WAITING_TEXT));
}

/// Contract (task 4.2): a refusing Owner paints its refusal text.
#[test]
fn a_refusing_owner_renders_its_refusal_text() {
    let mut fixture = built();
    fixture.set_attach(GuiAttachState::Refusal("the refusal text".to_owned()));
    assert!(fixture.text().contains("the refusal text"));
}

/// Contract (task 4.2): an exclusive Owner paints its refusal text.
#[test]
fn an_exclusive_owner_renders_its_refusal_text() {
    let mut fixture = built();
    fixture.set_attach(GuiAttachState::ExclusiveRefusal(
        "the exclusive text".to_owned(),
    ));
    assert!(fixture.text().contains("the exclusive text"));
}

/// Contract (task 4.2): a build mismatch paints its text and the Restart and
/// Quit buttons.
#[test]
fn a_build_mismatch_renders_its_text_and_the_restart_and_quit_buttons() {
    let mut fixture = built();
    fixture.set_attach(GuiAttachState::Mismatch(
        "the mismatch text\nRestarting stops playback and closes any other mbv terminals."
            .to_owned(),
    ));
    let text = fixture.text();
    assert!(text.contains("the mismatch text"));
    assert!(text.contains("Restart"));
    assert!(text.contains("Quit"));
}

/// Contract (task 4.2): an attach failure paints its text.
#[test]
fn an_attach_failure_renders_its_text() {
    let mut fixture = built();
    fixture.set_attach(GuiAttachState::Failed(
        "failed to attach to Owner process: nope".to_owned(),
    ));
    assert!(
        fixture
            .text()
            .contains("failed to attach to Owner process: nope")
    );
}

/// Contract (task 4.2): the mismatch buttons appear in the reported regions,
/// in the core's order.
#[test]
fn the_mismatch_buttons_appear_in_the_reported_regions() {
    let mut fixture = built();
    fixture.set_attach(GuiAttachState::Mismatch("the mismatch text".to_owned()));
    assert_eq!(
        last_reported_targets(&fixture.receiver),
        vec![
            RegionTarget::Attach(AttachTargetId::Restart),
            RegionTarget::Attach(AttachTargetId::Quit),
        ]
    );
}
