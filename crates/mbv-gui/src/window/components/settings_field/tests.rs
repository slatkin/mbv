//! Headless tests of the Settings field's mount passthrough (D16): the
//! optional `onmounted` prop hands the input's mount event to the caller —
//! the port's stand-in for the Svelte `focusOnMount` action's first-input
//! descendant query (task 8.4 attaches `focus_on_mount` through it).

use std::cell::Cell;
use std::rc::Rc;

use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::SettingsField;

/// The field with the caller's mount callback, as a form's first field
/// paints it.
fn field_app(onmounted: Option<Callback<Event<MountedData>>>) -> Element {
    rsx! {
        SettingsField {
            label: "URL".to_owned(),
            value: String::new(),
            field: "url".to_owned(),
            aria_label: "Server URL".to_owned(),
            oninput: Callback::new(|_: String| {}),
            onmounted,
        }
    }
}

/// Build the field and let its first render and effects land.
fn built(onmounted: Option<Callback<Event<MountedData>>>) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(field_app, onmounted),
        DocumentConfig::default(),
    );
    doc.initial_build();
    for _ in 0..8 {
        if !doc.poll(None) {
            break;
        }
    }
    doc
}

/// Contract (task 8.4's mount hook): the input hands its mount event to the
/// caller's callback.
#[test]
fn the_input_hands_its_mount_event_to_the_caller() {
    let fired = Rc::new(Cell::new(false));
    let flag = Rc::clone(&fired);
    let doc = built(Some(Callback::new(move |_event: Event<MountedData>| {
        flag.set(true);
    })));
    let dom = doc.inner();
    assert!(
        dom.query_selector("[data-settings-field=\"url\"]")
            .expect("the selector is valid")
            .is_some(),
        "the field paints"
    );
    assert!(fired.get(), "the mount callback ran");
}

/// Contract (the prop's default): a field without a mount callback paints
/// the same input — the passthrough stays out of the way.
#[test]
fn a_field_without_a_mount_callback_paints_the_input() {
    let doc = built(None);
    let dom = doc.inner();
    assert!(
        dom.query_selector("[data-settings-field=\"url\"]")
            .expect("the selector is valid")
            .is_some(),
        "the field paints"
    );
}
