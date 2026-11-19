//! Headless test of the Settings status line (D16): the contract of
//! `SettingsStatusBar.test.ts` — the line shows the armed prefix and the live
//! toast together (the spec's *The Settings screen shows the toasts*).

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::SettingsStatusBar;
use crate::bridge::frame::chrome::{GuiStatusBar, GuiToast, GuiToastSeverity};
use mbv_ui_model::toast::ToastId;

/// The status line over the carried facts and toast, mounted once per case.
fn painted(status: GuiStatusBar, toast: Option<GuiToast>) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            |(status, toast, section): (GuiStatusBar, Option<GuiToast>, String)| {
                rsx! {
                    SettingsStatusBar { status, toast, section }
                }
            },
            (status, toast, "Playback".to_owned()),
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

/// Contract (`SettingsStatusBar.test.ts`): the status line shows the armed
/// prefix and the live toast together.
#[test]
fn the_status_line_shows_the_prefix_and_the_toast_together() {
    let doc = painted(
        GuiStatusBar { prefix_armed: true },
        Some(GuiToast {
            id: ToastId(1),
            text: "Setting refused".to_owned(),
            severity: GuiToastSeverity::Warning,
        }),
    );

    assert_eq!(text_of(&doc, "[data-settings-status-prefix]"), "PREFIX");
    assert_eq!(
        text_of(&doc, "[data-settings-status-toast]"),
        "Setting refused"
    );
}

/// Contract: with no prefix and no toast, the line paints
/// its fixed key hints, and the shown section sits at the right.
#[test]
fn the_quiet_line_paints_the_key_hints_and_the_section() {
    let doc = painted(
        GuiStatusBar {
            prefix_armed: false,
        },
        None,
    );

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
