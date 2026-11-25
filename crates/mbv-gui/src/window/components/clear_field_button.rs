//! The clear button of a text field: the small X a search or filter field
//! paints while it holds text, and whose press empties the field. The Library's
//! three fields — the Search field, the list's `/` filter, and the collection
//! dropdown's filter — paint it through this one part, so each has one glyph,
//! one size, and one press rule.
//!
//! The caller paints the button only while its field holds text and passes the
//! field's own empty-the-text path as `on_clear`. The press must not take the
//! keyboard from the field: Blitz clears the DOM focus on a click that matches
//! no text input, and a blurred filter closes. The press handler therefore
//! stops the click's default action, so the field keeps the caret it had.

use dioxus_native::prelude::*;

use crate::window::icons::X;

/// The glyph's size in CSS pixels.
const CLEAR_ICON_SIZE: u16 = 12;

/// The props of the clear button.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct ClearFieldButtonProps {
    /// The button's accessible name and tooltip.
    pub label: &'static str,
    /// Extra inline CSS that places the button in its field: the Search field
    /// anchors it absolutely, the flex fields leave it in the row.
    #[props(default)]
    pub placement: &'static str,
    /// A press on the button: the field empties its text.
    pub on_clear: Callback<()>,
}

/// The clear button.
#[component]
pub fn ClearFieldButton(props: ClearFieldButtonProps) -> Element {
    let on_clear = props.on_clear;
    rsx! {
        button {
            "type": "button",
            class: "flex flex-none items-center justify-center",
            style: "width: 20px; height: 20px; padding: 0; background: none; border: 0; border-radius: 4px; color: var(--canvas-muted); cursor: pointer; {props.placement}",
            "aria-label": props.label,
            title: props.label,
            tabindex: -1,
            "data-field-clear": "",
            onclick: move |event: MouseEvent| {
                event.prevent_default();
                on_clear.call(());
            },
            X { size: CLEAR_ICON_SIZE, aria_hidden: true }
        }
    }
}

#[cfg(test)]
mod tests;
