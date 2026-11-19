//! The Queue panel's theme-token contract: what the recessed box paints once
//! the window's `:root` rule has been applied. The reference front end
//! left the six two-word chrome fills undefined, so the box, the footer band,
//! and the compact strip painted no fill of their own and the queue column's
//! backdrop showed through. The window's token
//! source reproduces that set (`window/theme.rs`), and this test reads the
//! resolved result rather than the rule's text.

use blitz_dom::Document as _;

use super::harness::*;

/// The recessed box the panel insets inside the queue column.
const BOX: &str = "[data-queue-box]";

/// What a `background-color` resolves to when its `var()` names no property:
/// the declaration is invalid at computed-value time, so the longhand takes
/// its initial value.
const TRANSPARENT: &str = "rgba(0, 0, 0, 0)";

/// The `background-color` the document resolves for the first node
/// `selector` names, after custom-property substitution.
fn background_of(fixture: &Fixture, selector: &str) -> String {
    let dom = fixture.doc.inner();
    let id = dom
        .query_selector(selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{selector} paints"));
    dom.resolved_style_value(id, "background-color")
}

/// Contract (look parity with the Tauri build, `replace-tauri-with-blitz`):
/// the recessed box paints no fill of its own, because the theme defines no
/// `--mbv-queuePanel` or `--mbv-queuePanelFocused`. A token writer that
/// defines them repaints the `#374145` box the reference build never showed,
/// hiding the queue column's backdrop.
#[test]
fn the_recessed_box_paints_no_fill_of_its_own() {
    let mut fixture = built_sized(themed_queue_panel_app);
    fixture.show(queue(vec![item(1)]), summary(), None);
    assert_eq!(background_of(&fixture, BOX), TRANSPARENT);
}
