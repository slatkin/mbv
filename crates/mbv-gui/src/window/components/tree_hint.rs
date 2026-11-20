//! The tree rows' hover tooltip (`gui-tree-double-click` design D3; user
//! request 2026-11-20): the card a parent row floats under its bottom edge,
//! asking for the double-click that opens or closes the node. The text names
//! the gesture the node's own open state answers — `Double-click to expand`
//! while it is closed and `Double-click to collapse` while it is open. The card
//! overlays the rows below it and never the row's own title or trailing text,
//! so the hint hides no information.
//!
//! The span holds no hover state and no open state: the row owns the open
//! state and passes it in, and `assets/app.css` shows the tooltip only while
//! the pointer rests on the row, so Blitz sets and clears the hover. The row
//! is the tooltip's positioning box and does not clip it.

use dioxus_native::prelude::*;

use crate::window::components::menu_card::MENU_CARD_STYLE;

/// The props of the hint.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TreeHintProps {
    /// Whether the row's node is open: it picks the hint's text.
    pub open: bool,
}

/// The hint a node's open state names.
fn hint_text(open: bool) -> &'static str {
    if open {
        "Double-click to collapse"
    } else {
        "Double-click to expand"
    }
}

/// The hint of a parent row.
#[component]
pub fn TreeHint(props: TreeHintProps) -> Element {
    rsx! {
        span {
            class: "tree-hint pointer-events-none absolute z-30 whitespace-nowrap",
            style: "top: 100%; right: 8px; margin-top: 2px; padding: 4px 8px; font-size: 12px; color: var(--canvas-heading); {MENU_CARD_STYLE}",
            "data-tree-hint": "",
            "{hint_text(props.open)}"
        }
    }
}

#[cfg(test)]
mod tests;
