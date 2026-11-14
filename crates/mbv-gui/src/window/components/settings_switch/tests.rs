//! Headless tests of the Settings switch (D16, the contracts of
//! `SettingsSwitch.test.ts`): the control carries `role="switch"` and
//! `aria-checked` for assistive technology. A headless document cannot fire
//! a DOM click, so the press contract — a press calls the handler the
//! keyboard's Space calls, so both paths share one effect (design D11's
//! pointer contract) — is the component's one-line `onclick` wiring over the
//! `onpress` prop, which the parent's state method owns; it lands with the
//! sections that mount the switch (task 8.3).

use std::cell::Cell;
use std::rc::Rc;

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{SettingsSwitch, press};

/// The switch over a no-op press, mounted once per case.
fn painted(checked: bool) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            |(label, checked): (String, bool)| {
                rsx! {
                    SettingsSwitch {
                        label,
                        checked,
                        onpress: Callback::new(|()| {}),
                    }
                }
            },
            ("Use mpv config".to_owned(), checked),
        ),
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

/// Contract (`SettingsSwitch.test.ts`): a switch carries role="switch",
/// `aria-label`, and `aria-checked` from its `checked` prop.
#[test]
fn the_switch_carries_role_and_aria_checked() {
    let doc = painted(true);
    assert_eq!(
        attr_of(&doc, "[data-settings-switch]", LocalName::from("role")),
        Some("switch".to_owned()),
    );
    assert_eq!(
        attr_of(&doc, "[data-settings-switch]", LocalName::from("aria-label")),
        Some("Use mpv config".to_owned()),
    );
    assert_eq!(
        attr_of(&doc, "[data-settings-switch]", LocalName::from("aria-checked")),
        Some("true".to_owned()),
    );
}

/// Contract (`SettingsSwitch.test.ts`): a switch with checked=false carries
/// aria-checked=false — the case table's other outcome.
#[test]
fn an_off_switch_carries_aria_checked_false() {
    let doc = painted(false);
    assert_eq!(
        attr_of(&doc, "[data-settings-switch]", LocalName::from("aria-checked")),
        Some("false".to_owned()),
    );
}

/// A press counter over the callback the handler runs.
fn counted() -> (Rc<Cell<usize>>, Callback<()>) {
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let onpress = Callback::new(move |()| counter.set(counter.get() + 1));
    (calls, onpress)
}

/// Contract (design D11's pointer contract): an enabled switch's press runs
/// `onpress`, the store function the keyboard's Space runs.
#[test]
fn an_enabled_switch_press_runs_onpress() {
    let (calls, onpress) = counted();
    press(false, onpress);
    assert_eq!(calls.get(), 1);
}

/// Contract (design D11's pointer contract): a disabled switch's press takes
/// no activation — blitz-dom dispatches Click on disabled targets, so the
/// handler itself returns early.
#[test]
fn a_disabled_switch_press_takes_no_activation() {
    let (calls, onpress) = counted();
    press(true, onpress);
    assert_eq!(calls.get(), 0);
}
