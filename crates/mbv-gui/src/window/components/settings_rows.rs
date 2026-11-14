//! The F2-row sections (`replace-tauri-with-blitz` task 8.3, the port of
//! `SettingsRows.svelte`; add-gui-settings-screen design D9, D13): Playback,
//! Display, Session, Queue, Mpv, and Actions — each row's label, its value,
//! and the way it is edited, painted with the shared Settings parts.
//!
//! A toggle row is a switch, a pick row is a pick button with its value
//! menu, a cycle row is a button carrying the value it steps, an action row
//! is a button, and an info row is its value text with no control. A choice
//! list is a checkbox-grid card whose Apply runs the row's own Apply entry,
//! so the grid and the keyboard share one effect. The cards sit in the
//! canvas's two-column grid, which leaves a single-card section (mpv) inside
//! the canvas's 560 px.
//!
//! The Library section reuses this component for its choice rows: the
//! content's rows paint whichever kind the frame carried, and the section's
//! full entry list keeps the cursor index the screen's own.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::rows::{GuiSettingRow, GuiSettingRowKind};
use crate::window::components::settings_button::SettingsButton;
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_checkbox_grid::{CheckboxOption, SettingsCheckboxGrid};
use crate::window::components::settings_pick_button::SettingsPickButton;
use crate::window::components::settings_row::SettingsRow;
use crate::window::components::settings_screen::{section_label, settings_section_view};
use crate::window::components::settings_switch::SettingsSwitch;
use crate::window::sections::Sections;
use crate::window::settings::entries::entry_id;
use crate::window::settings::state::SettingsState;

/// The one rows pane (the Svelte markup, character for character).
#[component]
pub fn SettingsRows() -> Element {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let Some(view) = settings_section_view(&sections, &settings) else {
        return rsx! {};
    };
    let rows = content_rows(&view.frame.content);
    let label = section_label(view.frame.section);

    // The rows one card holds as setting rows: a toggle, a cycle, a pick, or
    // an info row; the action rows, which the canvas paints as buttons; and
    // the choice-list rows, one checkbox-grid card each.
    let setting_rows: Vec<&GuiSettingRow> = rows
        .iter()
        .filter(|row| {
            matches!(
                row.kind,
                GuiSettingRowKind::Toggle
                    | GuiSettingRowKind::Cycle
                    | GuiSettingRowKind::Pick
                    | GuiSettingRowKind::Info
            )
        })
        .collect();
    let action_rows: Vec<&GuiSettingRow> = rows
        .iter()
        .filter(|row| row.kind == GuiSettingRowKind::Action)
        .collect();
    let choice_rows: Vec<&GuiSettingRow> = rows
        .iter()
        .filter(|row| row.kind == GuiSettingRowKind::ChoiceList)
        .collect();

    // A press on any control of this pane: the entry's activation through
    // the section's full entry list, the same one the keyboard's Enter runs.
    let press_settings = settings.clone();
    let press_entries = view.entries;
    let onpress = Callback::new(move |id: String| {
        press_settings.press_settings_entry(&press_entries, &id);
    });
    let cursor_id = view.cursor_id;

    rsx! {
        div {
            class: "grid",
            style: "grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px; align-items: start;",
            "data-settings-rows": "",
            if !setting_rows.is_empty() {
                SettingsCard {
                    label: Some(label.to_owned()),
                    rows: true,
                    for (index, row) in setting_rows.iter().enumerate() {
                        {
                            let id = entry_id::setting(&row.key);
                            let row_cursor = cursor_id.as_deref() == Some(id.as_str());
                            let switch_press = onpress.clone();
                            let pick_press = onpress.clone();
                            let cycle_press = onpress.clone();
                            let switch_id = id.clone();
                            let pick_id = id.clone();
                            let cycle_id = id.clone();
                            rsx! {
                                SettingsRow {
                                    label: row.label.clone(),
                                    divider: index < setting_rows.len() - 1,
                                    cursor: row_cursor,
                                    if row.kind == GuiSettingRowKind::Toggle {
                                        SettingsSwitch {
                                            label: row.label.clone(),
                                            checked: row.flag.unwrap_or(false),
                                            onpress: Callback::new(move |()| switch_press.call(switch_id.clone())),
                                        }
                                    } else if row.kind == GuiSettingRowKind::Info {
                                        span {
                                            "data-settings-info": "{row.key}",
                                            style: "color: var(--canvas-dim);",
                                            "{row.value}"
                                        }
                                    } else if row.kind == GuiSettingRowKind::Pick {
                                        SettingsPickButton {
                                            setting: row.key.clone(),
                                            label: row.label.clone(),
                                            value: row.value.clone(),
                                            onpress: Callback::new(move |()| pick_press.call(pick_id.clone())),
                                        }
                                    } else {
                                        SettingsButton {
                                            onpress: Some(Callback::new(move |()| cycle_press.call(cycle_id.clone()))),
                                            "{row.value}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !action_rows.is_empty() {
                SettingsCard {
                    label: Some(label.to_owned()),
                    div {
                        class: "flex flex-wrap",
                        style: "gap: 8px;",
                        for row in &action_rows {
                            {
                                let action_press = onpress.clone();
                                let action_id = entry_id::setting(&row.key);
                                rsx! {
                                    SettingsButton {
                                        onpress: Some(Callback::new(move |()| action_press.call(action_id.clone()))),
                                        "{row.label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for row in &choice_rows {
                {
                    let options = row
                        .choices
                        .iter()
                        .flatten()
                        .map(|choice| CheckboxOption {
                            id: choice.id.clone(),
                            label: choice.label.clone(),
                        })
                        .collect();
                    let selected = settings.marked_choices(&row.key, row.choices.as_deref());
                    rsx! {
                        SettingsCheckboxGrid {
                            key: "{row.key}",
                            title: row.label.clone(),
                            row_key: row.key.clone(),
                            options,
                            selected,
                            cursor_id: cursor_id.clone(),
                            onpress: onpress.clone(),
                        }
                    }
                }
            }
        }
    }
}

/// The F2 rows the content carries (the Svelte screen's `rows` prop source):
/// a rows content's own rows, or the Library section's choice rows.
fn content_rows(content: &GuiSettingsContent) -> &[GuiSettingRow] {
    match content {
        GuiSettingsContent::Rows { rows } => rows,
        GuiSettingsContent::Library { rows, .. } => rows,
        _ => &[],
    }
}

#[cfg(test)]
mod tests;
