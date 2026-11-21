//! Headless tests of the confirmation modal (D16): the contracts of
//! `ConfirmOverlay.test.ts` that the window's port keeps — the modal paints
//! its message and its choices, and the affirmative choice paints the accent
//! text token on the recessed fill (the dark-on-green contrast bug this
//! guards). The core channel is a mock pair; no test opens a window or
//! sleeps.

use std::sync::mpsc::{Receiver, Sender, channel};

use blitz_dom::{Document as _, local_name};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::ConfirmOverlay;
use crate::bridge::frame::overlay::{GuiConfirmButton, GuiConfirmButtonTone};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::layout::LayoutEpoch;
use crate::window::layout::region_set::last_reported_targets;
use crate::window::layout::use_layout_reporter;

#[derive(Clone)]
struct FixtureProps {
    sender: Sender<CoreInput>,
    message: String,
    buttons: Vec<GuiConfirmButton>,
}

/// The root: provides the reporter's contexts and paints the modal.
fn confirm_app(props: FixtureProps) -> Element {
    provide_context(CoreLink::new(props.sender));
    provide_context(use_hook(LayoutEpoch::new));
    let reporter = use_layout_reporter();
    provide_context(reporter);
    rsx! {
        ConfirmOverlay { message: props.message, buttons: props.buttons }
    }
}

struct Fixture {
    doc: DioxusDocument,
    receiver: Receiver<CoreInput>,
}

/// The modal the core projects: a question and the two choices the TUI's
/// confirm modal offers.
fn confirm_buttons() -> Vec<GuiConfirmButton> {
    vec![
        GuiConfirmButton {
            label: "Clear".to_owned(),
            keys: "Enter".to_owned(),
            tone: GuiConfirmButtonTone::Affirmative,
        },
        GuiConfirmButton {
            label: "Cancel".to_owned(),
            keys: "Esc".to_owned(),
            tone: GuiConfirmButtonTone::Cancel,
        },
    ]
}

impl Fixture {
    /// The whole text the document paints.
    fn text(&self) -> String {
        use blitz_dom::traversal::TreeTraverser;
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

    /// The `style` attribute of the node the selector names.
    fn style_of(&self, selector: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")?;
        let node = dom.get_node(id)?;
        node.attr(local_name!("style")).map(str::to_owned)
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
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            confirm_app,
            FixtureProps {
                sender,
                message: "Clear the queue?".to_owned(),
                buttons: confirm_buttons(),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let mut fixture = Fixture { doc, receiver };
    fixture.poll_until_idle();
    fixture
}

/// Contract (task 4.2): the modal paints its message and its choices.
#[test]
fn the_modal_paints_its_message_and_its_choices() {
    let fixture = built();
    let text = fixture.text();
    assert!(text.contains("Clear the queue?"));
    assert!(text.contains("Clear"));
    assert!(text.contains("Cancel"));
}

/// Contract (task 4.2): the affirmative choice paints the accent text token
/// on the recessed fill — never the picked row's fill token (the
/// dark-on-green contrast bug this guards).
#[test]
fn the_affirmative_choice_paints_the_accent_text_token_on_the_recessed_fill() {
    let fixture = built();
    let style = fixture
        .style_of("[data-confirm-button=\"0\"]")
        .expect("the affirmative button paints");
    assert!(style.contains("color: var(--canvas-accent)"), "{style}");
    assert!(style.contains("background: var(--canvas-well)"), "{style}");
}

/// Contract (task 4.2): the buttons report their `confirm` regions, in the
/// core's order (the merged report the core's hit map reads).
#[test]
fn the_buttons_report_their_confirm_regions() {
    use crate::bridge::layout::{ConfirmTargetId, RegionTarget};
    let mut fixture = built();
    fixture.poll_until_idle();
    let targets = last_reported_targets(&fixture.receiver);
    assert_eq!(
        targets,
        vec![
            RegionTarget::Confirm(ConfirmTargetId::Button(0)),
            RegionTarget::Confirm(ConfirmTargetId::Button(1)),
        ]
    );
}
