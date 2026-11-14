//! A Settings checkbox grid (`replace-tauri-with-blitz` task 8.2, the port
//! of `SettingsCheckboxGrid.svelte`; add-gui-settings-screen design D13,
//! D15, the canvas My languages card): three columns of 36 px
//! `role="checkbox"` buttons over a count header, closed by the Apply
//! primary button. A checked box paints the success fill and border with an
//! accent check box. The marks are the window's, not this card's:
//! `selected` is the store's held marks (or the frame's selection), and a
//! press on a box or on Apply runs its entry's activation through `onpress`,
//! the same one the keyboard runs. The content cursor paints on the box or
//! the Apply it rests on.

use dioxus_native::prelude::*;

use crate::window::icons::Check;
use crate::window::settings::entries::entry_id;
use crate::window::truncate::{TextStyle, Truncate};

use super::settings_button::{SettingsButton, SettingsButtonVariant};
use super::settings_card::SettingsCard;

/// One box of the grid (the Svelte `options` row shape): the value the apply
/// sends back and the label it paints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckboxOption {
    pub id: String,
    pub label: String,
}

/// The props of the grid (the `SettingsCheckboxGrid.svelte` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsCheckboxGridProps {
    /// The card's heading: the section's name for the list.
    pub title: String,
    /// The row's setting name, which names each box's and Apply's entry.
    pub row_key: String,
    /// The boxes, in paint order: `id` is the value the apply sends back.
    pub options: Vec<CheckboxOption>,
    /// The ids the window marks.
    pub selected: Vec<String>,
    /// The entry id the content cursor rests on, or `None` while the sidebar
    /// holds the keyboard.
    #[props(default)]
    pub cursor_id: Option<String>,
    /// A press on a box or on Apply: the entry's id.
    pub onpress: Callback<String>,
    /// The header's count text; the default names the count alone.
    #[props(default)]
    pub count_label: Option<Callback<usize, String>>,
}

/// The one checkbox grid (the Svelte markup, character for character).
#[component]
pub fn SettingsCheckboxGrid(props: SettingsCheckboxGridProps) -> Element {
    let apply_id = entry_id::choices_apply(&props.row_key);
    let count = props.selected.len();
    let count_label = match props.count_label {
        Some(count_label) => count_label.call(count),
        None => format!("{count} selected"),
    };
    let onpress = props.onpress;
    let row_key = props.row_key.clone();
    let cursor_id = props.cursor_id.clone();
    let selected = props.selected.clone();
    let header = rsx! {
        span {
            style: "font-size: 12px; color: var(--canvas-type-count);",
            "{count_label}"
        }
    };
    rsx! {
        SettingsCard {
            title: props.title.clone(),
            header: Some(header),
            div {
                class: "grid",
                style: "grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 6px;",
                "data-settings-checkbox-grid": "",
                for option in &props.options {
                    {
                        let id = entry_id::choice(&row_key, &option.id);
                        let on = selected.contains(&option.id);
                        let cursor = cursor_id.as_deref() == Some(id.as_str());
                        let box_style = format!(
                            "height: 36px; padding: 0 10px; gap: 10px; border-radius: 6px; border: 1px solid {}; background: {}; color: {}; font-size: 13px;{}",
                            if on { "var(--canvas-success-border)" } else { "transparent" },
                            if on { "var(--canvas-success-fill)" } else { "none" },
                            if on { "var(--canvas-heading)" } else { "var(--canvas-text)" },
                            if cursor {
                                " outline: 2px solid var(--canvas-accent); outline-offset: -2px;"
                            } else {
                                ""
                            },
                        );
                        let mark_style = format!(
                            "width: 16px; height: 16px; box-sizing: border-box; border-radius: 4px; border: 1.5px solid {}; background: {};",
                            if on { "var(--canvas-accent)" } else { "var(--canvas-ordinal)" },
                            if on { "var(--canvas-accent)" } else { "transparent" },
                        );
                        let press_id = id.clone();
                        let label = option.label.clone();
                        rsx! {
                            button {
                                key: "{option.id}",
                                "type": "button",
                                role: "checkbox",
                                "aria-checked": "{on}",
                                class: "flex items-center justify-start text-left",
                                style: "{box_style}",
                                "data-settings-checkbox": "{option.id}",
                                "data-settings-entry": "{id}",
                                "data-settings-cursor": cursor.then_some(""),
                                onclick: move |_| onpress.call(press_id.clone()),
                                span {
                                    class: "flex flex-none items-center justify-center",
                                    style: "{mark_style}",
                                    "aria-hidden": "true",
                                    if on {
                                        Check { size: 11, stroke_width: 3.5, color: "var(--canvas-ink)" }
                                    }
                                }
                                Truncate {
                                    text: label,
                                    class: "min-w-0".to_owned(),
                                    style: TextStyle::sans(13.0, 400.0),
                                }
                            }
                        }
                    }
                }
            }
            div {
                class: "flex justify-end",
                SettingsButton {
                    variant: SettingsButtonVariant::Primary,
                    entry: Some(apply_id.clone()),
                    cursor: cursor_id.as_deref() == Some(apply_id.as_str()),
                    onpress: Some(Callback::new(move |()| onpress.call(apply_id.clone()))),
                    "Apply"
                }
            }
        }
    }
}
