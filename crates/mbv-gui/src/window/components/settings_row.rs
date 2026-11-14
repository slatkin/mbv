//! One setting row of a Settings card (`replace-tauri-with-blitz` task 8.2,
//! the port of `SettingsRow.svelte`; add-gui-settings-screen design D13,
//! D15, the canvas row): 60 px high, the label on the left at the canvas's
//! 14 px heading colour, the control on the right, and the canvas's divider
//! under every row but its card's last one. The cursor row takes the picked
//! row's fill while the content pane holds the keyboard: a 6 px rounded fill
//! that reaches 12 px past the row's text edges (the canvas's picked-row
//! radius) and hides the divider beneath it.

use dioxus_native::prelude::*;

use crate::window::components::cursor_reveal::cursor_reveal;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the row (the `SettingsRow.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsRowProps {
    /// The row's label.
    pub label: String,
    /// Whether the row paints its divider; the last row of a card passes
    /// `false`.
    #[props(default = true)]
    pub divider: bool,
    /// Whether the content cursor rests on this row while the content pane
    /// holds the keyboard.
    #[props(default)]
    pub cursor: bool,
    /// The row's control, flush right: a switch, a pick button, or text.
    pub children: Element,
}

/// One setting row (the Svelte markup, character for character).
#[component]
pub fn SettingsRow(props: SettingsRowProps) -> Element {
    let background = if props.cursor {
        "var(--canvas-selected)"
    } else {
        "none"
    };
    let border_bottom = if props.divider {
        format!(
            "border-bottom: 1px solid {};",
            if props.cursor {
                "transparent"
            } else {
                "var(--canvas-border)"
            }
        )
    } else {
        String::new()
    };
    // The row that paints the cursor reveals itself: the Svelte screen's
    // `scrollIntoView` on the pane's `[data-settings-cursor]` row (D15).
    let mut reveal = cursor_reveal(props.cursor);
    rsx! {
        div {
            class: "flex items-center justify-between",
            style: "height: 60px; gap: 16px; margin: 0 -12px; padding: 0 12px; border-radius: 6px; background: {background}; {border_bottom}",
            "data-settings-setting-row": "",
            "data-settings-cursor": props.cursor.then_some(""),
            onmounted: move |event: Event<MountedData>| reveal.set(Some(event)),
            Truncate {
                text: props.label,
                class: "min-w-0 flex-1".to_owned(),
                style: TextStyle::sans(14.0, 400.0),
                // The Svelte row's `style:font-size="14px"`: the measure cuts
                // for 14 px, so the box must paint 14 px or the cut clips
                // mid-glyph instead of ending in an ellipsis.
                css: "font-size: 14px; color: var(--canvas-heading);".to_owned(),
            }
            span {
                class: "flex flex-none items-center",
                {props.children}
            }
        }
    }
}

#[cfg(test)]
mod tests;
