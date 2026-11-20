//! The tree rows' hover hint (`gui-tree-double-click` design D3): the span a
//! parent row paints at its trailing end, asking for the double-click that
//! opens or closes the node. The text names the gesture the node's own open
//! state answers — `Double-click to expand` while it is closed and
//! `Double-click to collapse` while it is open — in the canvas's muted tone at
//! 12 px.
//!
//! The span holds no hover state and no open state: the row owns the open
//! state and passes it in, and `assets/app.css` shows the hint only while the
//! pointer rests on the row, so Blitz sets and clears the hover.

use dioxus_native::prelude::*;

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
            class: "tree-hint flex-none whitespace-nowrap",
            style: "font-size: 12px; color: var(--canvas-muted);",
            "data-tree-hint": "",
            "{hint_text(props.open)}"
        }
    }
}

#[cfg(test)]
mod tests;
