//! Headless test of the Settings status line (D16): the contract of
//! `SettingsStatusBar.test.ts` — the line shows the armed prefix (the spec's
//! *The Settings screen shows the toasts* puts the toasts in floating cards
//! over the screen, so the line shows none).

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::SettingsStatusBar;
use crate::bridge::frame::chrome::GuiStatusBar;

/// The status line over the carried facts, mounted once per case.
fn painted(status: GuiStatusBar) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            |(status, section): (GuiStatusBar, String)| {
                rsx! {
                    SettingsStatusBar { status, section }
                }
            },
            (status, "Playback".to_owned()),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// The text of the first node the selector names.
fn text_of(doc: &DioxusDocument, selector: &str) -> String {
    let dom = doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    dom.get_node(id)
        .expect("the node is in the document")
        .text_content()
        .trim()
        .to_owned()
}

/// Contract (`SettingsStatusBar.test.ts`): with prefix mode armed the line
/// shows the armed prefix, and carries no toast text.
#[test]
fn the_status_line_shows_the_prefix_and_no_toast() {
    let doc = painted(GuiStatusBar { prefix_armed: true });

    assert_eq!(text_of(&doc, "[data-settings-status-prefix]"), "PREFIX");
    assert!(!paints_a_toast(&doc), "the line paints no toast node");
}

/// Whether the line paints a toast node: it paints none, since the floating
/// stack owns the toasts.
fn paints_a_toast(doc: &DioxusDocument) -> bool {
    matches!(
        doc.inner().query_selector("[data-settings-status-toast]"),
        Ok(Some(_))
    )
}

/// Contract: with no prefix armed, the line paints its fixed key hints, and
/// the shown section sits at the right.
#[test]
fn the_quiet_line_paints_the_key_hints_and_the_section() {
    let doc = painted(GuiStatusBar {
        prefix_armed: false,
    });

    assert_eq!(
        doc.inner()
            .query_selector_all("[data-settings-hint]")
            .expect("the selector is valid")
            .len(),
        HINT_COUNT,
        "the five fixed hints paint"
    );
    assert_eq!(
        text_of(&doc, "[data-settings-status-section]"),
        "Playback",
        "the shown section paints at the right"
    );
}

/// The screen's fixed hint count.
const HINT_COUNT: usize = 5;
