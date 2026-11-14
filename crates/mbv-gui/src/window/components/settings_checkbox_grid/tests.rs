//! Headless tests of the Settings checkbox grid (D16, the contracts of
//! `SettingsCheckboxGrid.test.ts`): each box carries `role="checkbox"` and
//! `aria-checked` from the window's marks, and the entry a press reports is
//! the id the box paints in `data-settings-entry` — the grid holds no marks
//! of its own, so the keyboard and the pointer share one store (the spec's
//! *Choice-list settings are edited in place*). A headless document cannot
//! fire a DOM click, so the press contract rides the painted entry ids: the
//! component's handler calls `onpress` with the same id the attribute
//! carries, one line each.

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{CheckboxOption, SettingsCheckboxGrid};

/// The boxes of one grid: two languages, one marked by the window.
fn options() -> Vec<CheckboxOption> {
    vec![
        CheckboxOption {
            id: "english".to_owned(),
            label: "English".to_owned(),
        },
        CheckboxOption {
            id: "french".to_owned(),
            label: "French".to_owned(),
        },
    ]
}

/// The grid over a no-op press, as the paint tests mount it.
fn grid_app() -> Element {
    rsx! {
        SettingsCheckboxGrid {
            title: "My languages".to_owned(),
            row_key: "myLanguages".to_owned(),
            options: options(),
            selected: vec!["english".to_owned()],
            onpress: Callback::new(|_: String| {}),
        }
    }
}

/// Build the painted document once for the paint contracts.
fn painted() -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new(grid_app),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// The attribute of the first node the selector names.
fn attr_of(doc: &DioxusDocument, selector: &str, attribute: LocalName) -> Option<String> {
    let dom = doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    dom.get_node(id)
        .expect("the node is in the document")
        .attr(attribute)
        .map(str::to_owned)
}

/// Contract (`SettingsCheckboxGrid.test.ts`): the boxes carry role="checkbox"
/// and the window's aria-checked.
#[test]
fn the_boxes_carry_role_checkbox_and_the_windows_aria_checked() {
    let doc = painted();
    assert_eq!(
        attr_of(&doc, "[data-settings-checkbox=\"english\"]", LocalName::from("role")),
        Some("checkbox".to_owned()),
    );
    assert_eq!(
        attr_of(&doc, "[data-settings-checkbox=\"english\"]", LocalName::from("aria-checked")),
        Some("true".to_owned()),
    );
    assert_eq!(
        attr_of(&doc, "[data-settings-checkbox=\"french\"]", LocalName::from("aria-checked")),
        Some("false".to_owned()),
    );
}

/// Contract (`SettingsCheckboxGrid.test.ts`): a box paints its choice entry
/// id — the id a press on it reports, since the component's `onclick` calls
/// `onpress` with the same id (a headless document cannot fire a DOM click).
#[test]
fn a_box_paints_its_choice_entry_id() {
    let doc = painted();
    assert_eq!(
        attr_of(&doc, "[data-settings-checkbox=\"french\"]", LocalName::from("data-settings-entry")),
        Some("choice:myLanguages:french".to_owned()),
    );
}

/// Contract (`SettingsCheckboxGrid.test.ts`): the Apply button paints the
/// apply entry id — the id a press on it reports, same one-line wiring.
#[test]
fn the_apply_button_paints_the_apply_entry_id() {
    let doc = painted();
    assert_eq!(
        attr_of(&doc, "[data-settings-button=\"primary\"]", LocalName::from("data-settings-entry")),
        Some("apply:myLanguages".to_owned()),
    );
}

/// The count header names the held marks' count (`countLabel`'s default).
#[test]
fn the_header_names_the_mark_count() {
    let doc = painted();
    let dom = doc.inner();
    let text = dom
        .query_selector("[data-settings-card]")
        .expect("the selector is valid")
        .map(|id| dom.get_node(id).expect("the card paints").text_content())
        .unwrap_or_default();
    assert!(text.contains("1 selected"), "the header names the count: {text}");
}
