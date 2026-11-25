//! The list filter field's clear-button contract (user request 2026-11-25):
//! the field paints the clear button only while it holds text. What the
//! press does — empty the destination's filter text — is the `on_input`
//! callback's path, which the destination's tests own. No test opens a
//! window, sleeps, or reaches the core.

use std::sync::{Arc, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::LibraryFilterField;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::library::state::LibraryState;
use crate::window::sections::Sections;

/// The harness's props: the mock core sender and the field's text.
#[derive(Clone)]
struct FixtureProps {
    sender: mpsc::Sender<CoreInput>,
    value: String,
}

/// The root: provides the Library store over the mock core link and paints
/// the field, as a destination's list header composes it.
fn field_app(props: FixtureProps) -> Element {
    let FixtureProps { sender, value } = props;
    let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
    let state = use_hook(move || {
        let playhead_position = Memo::new(|| 0.0);
        LibraryState::new(
            Sections::new(),
            CoreLink::new(sender),
            playhead_position,
            clock,
        )
    });
    provide_context(state);
    rsx! {
        LibraryFilterField {
            value,
            placeholder: "Filter".to_owned(),
            aria_label: "Filter the list".to_owned(),
            on_input: |_: String| {},
            on_accept: |()| {},
        }
    }
}

/// The field painted over `value`. The receiver rides along so the link's
/// sends land.
fn painted(value: &str) -> (DioxusDocument, mpsc::Receiver<CoreInput>) {
    let (sender, receiver) = mpsc::channel::<CoreInput>();
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            field_app,
            FixtureProps {
                sender,
                value: value.to_owned(),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    (doc, receiver)
}

/// Contract: an empty field paints no clear button.
#[test]
fn an_empty_field_paints_no_clear_button() {
    let (doc, _receiver) = painted("");

    assert!(matches!(
        doc.inner().query_selector("[data-field-clear]"),
        Ok(None)
    ));
}

/// Contract: a field that holds text paints the clear button.
#[test]
fn a_field_that_holds_text_paints_the_clear_button() {
    let (doc, _receiver) = painted("nubian");

    assert!(matches!(
        doc.inner().query_selector("[data-field-clear]"),
        Ok(Some(_))
    ));
}
