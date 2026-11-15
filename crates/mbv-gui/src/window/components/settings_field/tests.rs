//! Headless test of the Settings field's paint (D16): the field paints its
//! labelled mono input, and the optional `onmounted` prop stays out of the
//! way when no caller attaches one. The mount passthrough itself — the port's
//! stand-in for the Svelte `focusOnMount` action's first-input descendant
//! query (task 8.4 attaches `focus_on_mount` through it) — observes a
//! callback, which no stateless headless test can pin; the forms that mount
//! it carry the contract (task 8.4).

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::SettingsField;

/// The field without a mount callback, as a form's later fields paint it.
fn painted() -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            |(label, field): (String, String)| {
                rsx! {
                    SettingsField {
                        label,
                        value: String::new(),
                        field,
                        aria_label: "Server URL".to_owned(),
                        oninput: Callback::new(|_: String| {}),
                    }
                }
            },
            ("URL".to_owned(), "url".to_owned()),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// Contract (the Svelte markup): the field paints its labelled input with
/// the `data-settings-field` name the tests and the key router read.
#[test]
fn the_field_paints_its_named_input() {
    let doc = painted();
    let dom = doc.inner();
    let id = dom
        .query_selector("[data-settings-field=\"url\"]")
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("the field's input paints"));
    let input_type = dom
        .get_node(id)
        .expect("the node is in the document")
        .attr(LocalName::from("type"))
        .unwrap_or_default();
    assert_eq!(input_type, "text");
}
