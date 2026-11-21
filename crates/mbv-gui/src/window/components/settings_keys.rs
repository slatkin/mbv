//! The Keys section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsKeys`; add-gui-settings-screen design D9, D13): the
//! configured prefix chord and one card for each action group, every card
//! holding 36 px rows of buttons. Each action row shows the effective router
//! chords, its prefix-namespace chord where one is assigned, and the
//! changed-from-default mark. Each chord is its own capture target:
//! activating it arms the capture, and the next press is the candidate
//! chord. The frame's last refusal paints under the edited row.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::keys::{GuiSettingsKeyRefusal, GuiSettingsKeyRow};
use crate::window::components::settings_button::{SettingsButton, SettingsButtonVariant};
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_screen::{
    SETTINGS_PANES_GRID_STYLE, SettingsPaneScaffold, settings_pane,
};
use crate::window::settings::entries::entry_id;
use crate::window::settings::state::Capture;
use crate::window::truncate::{TextStyle, Truncate};

/// The one Keys pane.
#[component]
pub fn SettingsKeys() -> Element {
    let Some(SettingsPaneScaffold {
        settings,
        frame,
        onpress,
        cursor_id,
        ..
    }) = settings_pane()
    else {
        return rsx! {};
    };
    let GuiSettingsContent::Keys {
        prefix,
        rows,
        refusal,
    } = &frame.content
    else {
        return rsx! {};
    };

    let capture = settings.ui.cloned().capture;

    // The rows grouped into one card per section, in the frame's order (the
    // the former client `groups` derived).
    let mut groups: Vec<(&str, Vec<&GuiSettingsKeyRow>)> = Vec::new();
    for row in rows {
        match groups.last_mut() {
            Some((section, group)) if *section == row.section.as_str() => group.push(row),
            _ => groups.push((row.section.as_str(), vec![row])),
        }
    }

    rsx! {
        div {
            class: "grid",
            style: SETTINGS_PANES_GRID_STYLE,
            "data-settings-keys": "",
            SettingsCard {
                title: Some("Prefix chord".to_owned()),
                rows: true,
                div {
                    class: "flex items-center",
                    style: "height: 36px; gap: 8px;",
                    "data-settings-key-row": "prefix",
                    Truncate {
                        text: "Assigned".to_owned(),
                        class: "min-w-0 flex-1".to_owned(),
                        style: TextStyle::sans(14.0, 400.0),
                        css: "font-size: 14px; color: var(--canvas-heading);".to_owned(),
                    }
                    {
                        let chord_press = onpress;
                        let target_id = entry_id::key_target("prefix");
                        let chord_text = target_text(
                            capture.as_ref(),
                            "prefix",
                            prefix.as_deref().unwrap_or("none"),
                        );
                        rsx! {
                            SettingsButton {
                                entry: Some(target_id.clone()),
                                cursor: cursor_id.as_deref() == Some(target_id.as_str()),
                                onpress: Some(Callback::new(move |()| chord_press.call(target_id.clone()))),
                                span {
                                    class: "font-mono",
                                    "{chord_text}"
                                }
                            }
                        }
                    }
                    if prefix.is_some() {
                        {
                            let clear_press = onpress;
                            let clear_id = entry_id::key_clear("prefix");
                            rsx! {
                                SettingsButton {
                                    variant: SettingsButtonVariant::Destructive,
                                    entry: Some(clear_id.clone()),
                                    label: Some("Clear the prefix chord".to_owned()),
                                    cursor: cursor_id.as_deref() == Some(clear_id.as_str()),
                                    onpress: Some(Callback::new(move |()| clear_press.call(clear_id.clone()))),
                                    "✕"
                                }
                            }
                        }
                    }
                }
                { refusal_line(refusal.as_ref(), "prefix") }
            }
            for (group_section, group_rows) in &groups {
                SettingsCard {
                    key: "{group_section}",
                    title: Some((*group_section).to_owned()),
                    rows: true,
                    for row in group_rows {
                        {
                            let target_id = entry_id::key_target(&row.id);
                            let prefix_target_id = entry_id::key_target(&format!("prefix:{}", row.id));
                            let reset_id = entry_id::key_reset(&row.id);
                            let clear_id = entry_id::key_clear(&format!("prefix:{}", row.id));
                            let row_press = onpress;
                            let prefix_press = onpress;
                            let reset_press = onpress;
                            let clear_press = onpress;
                            let label = if row.changed {
                                format!("{} *", row.label)
                            } else {
                                row.label.clone()
                            };
                            let label_color = if row.changed {
                                "var(--canvas-accent)"
                            } else {
                                "var(--canvas-heading)"
                            };
                            let chords = target_text(capture.as_ref(), &row.id, &row.chords.join(" "));
                            let prefix_chord = target_text(
                                capture.as_ref(),
                                &format!("prefix:{}", row.id),
                                row.prefix_chord.as_deref().unwrap_or("—"),
                            );
                            rsx! {
                                div {
                                    key: "{row.id}",
                                    class: "flex items-center",
                                    style: "height: 36px; gap: 8px;",
                                    "data-settings-key-row": "{row.id}",
                                    Truncate {
                                        text: label,
                                        class: "min-w-0 flex-1".to_owned(),
                                        style: TextStyle::sans(14.0, 400.0),
                                        css: format!("font-size: 14px; color: {label_color};"),
                                    }
                                    SettingsButton {
                                        entry: Some(target_id.clone()),
                                        cursor: cursor_id.as_deref() == Some(target_id.as_str()),
                                        onpress: Some(Callback::new(move |()| row_press.call(target_id.clone()))),
                                        span {
                                            class: "font-mono",
                                            "{chords}"
                                        }
                                    }
                                    if row.prefix_addressable {
                                        SettingsButton {
                                            entry: Some(prefix_target_id.clone()),
                                            cursor: cursor_id.as_deref() == Some(prefix_target_id.as_str()),
                                            onpress: Some(Callback::new(move |()| prefix_press.call(prefix_target_id.clone()))),
                                            span {
                                                class: "font-mono",
                                                "{prefix_chord}"
                                            }
                                        }
                                    }
                                    if row.changed {
                                        SettingsButton {
                                            variant: SettingsButtonVariant::Ghost,
                                            entry: Some(reset_id.clone()),
                                            label: Some(format!("Reset {} to its default", row.label)),
                                            cursor: cursor_id.as_deref() == Some(reset_id.as_str()),
                                            onpress: Some(Callback::new(move |()| reset_press.call(reset_id.clone()))),
                                            "↺"
                                        }
                                    }
                                    if row.prefix_chord.is_some() {
                                        SettingsButton {
                                            variant: SettingsButtonVariant::Destructive,
                                            entry: Some(clear_id.clone()),
                                            label: Some(format!("Clear {}'s prefix chord", row.label)),
                                            cursor: cursor_id.as_deref() == Some(clear_id.as_str()),
                                            onpress: Some(Callback::new(move |()| clear_press.call(clear_id.clone()))),
                                            "✕"
                                        }
                                    }
                                }
                                { refusal_line(refusal.as_ref(), &row.id) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The capture target's painted text (`SettingsKeys`'s
/// `targetText`): "press a key" while its capture waits, the assigned chords
/// otherwise.
fn target_text(capture: Option<&Capture>, target: &str, chords: &str) -> String {
    if capture.is_some_and(|capture| capture.target == target) {
        "press a key…".to_owned()
    } else {
        chords.to_owned()
    }
}

/// The frame's last refusal under the edited row: the refusal text paints only on the row it names.
fn refusal_line(refusal: Option<&GuiSettingsKeyRefusal>, action: &str) -> Element {
    match refusal.filter(|refusal| refusal.action == action) {
        Some(refusal) => rsx! {
            div {
                style: "padding: 0 0 6px; font-size: 13px; color: var(--canvas-destructive);",
                "data-settings-key-refusal": "",
                "{refusal.text}"
            }
        },
        None => rsx! {},
    }
}

#[cfg(test)]
mod tests;
