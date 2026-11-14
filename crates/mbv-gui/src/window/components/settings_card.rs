//! One card of a Settings section (`replace-tauri-with-blitz` task 8.2, the
//! port of `SettingsCard.svelte`; add-gui-settings-screen design D13, D15):
//! the raised fill, a border, a 10 px radius, and 20 px of padding. A card
//! that holds setting rows paints the canvas's 8 px by 20 px padding instead,
//! so each row's divider spans the card's inner width. The heading takes
//! trailing content — a state pill or a count — through the `header` element.

use dioxus_native::prelude::*;

/// The props of the card (the `SettingsCard.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsCardProps {
    /// The card's heading. A card with no heading names itself through
    /// `label` instead.
    #[props(default)]
    pub title: Option<String>,
    /// The card's accessible name while it paints no heading.
    #[props(default)]
    pub label: Option<String>,
    /// The heading's trailing content: a state pill or a count.
    #[props(default)]
    pub header: Option<Element>,
    /// Paint the canvas's row-list padding (`8px 20px`) and no gap, for a
    /// card whose body is `SettingsRow`s with their own dividers.
    #[props(default)]
    pub rows: bool,
    /// The card's body.
    pub children: Element,
}

/// The one Settings card (the Svelte markup, character for character).
#[component]
pub fn SettingsCard(props: SettingsCardProps) -> Element {
    // Whether the card paints a heading band.
    let headed = props.title.is_some() || props.header.is_some();
    // The card's padding: a headed card whose body is setting rows keeps the
    // canvas's 20 px side padding, so the heading and the rows share one left
    // edge, and closes with the canvas's 8 px foot.
    let padding = if props.rows {
        if headed { "20px 20px 8px" } else { "8px 20px" }
    } else {
        "20px"
    };
    let gap = if props.rows { "0" } else { "16px" };
    rsx! {
        section {
            class: "flex flex-col",
            style: "background: var(--canvas-library-panel); border: 1px solid var(--canvas-border); border-radius: 10px; padding: {padding}; gap: {gap};",
            "aria-label": props.label,
            "data-settings-card": "",
            if headed {
                div {
                    class: "flex items-center justify-between",
                    style: if props.rows {
                        "gap: 12px; padding-bottom: 12px;"
                    } else {
                        "gap: 12px;"
                    },
                    if let Some(title) = props.title.as_ref() {
                        h2 {
                            style: "margin: 0; font-size: 16px; font-weight: 600; color: var(--canvas-heading);",
                            "{title}"
                        }
                    }
                    if let Some(header) = props.header.as_ref() {
                        {header.clone()}
                    }
                }
            }
            {props.children}
        }
    }
}
