//! The Sessions section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsSessions`; add-gui-settings-screen design D9, D13): the
//! same playback targets the header's chooser lists, each marked while it is
//! the Owner's attached output, in a card of 36 px rows. Selecting one sends
//! `selectTarget` — the chooser's own path — and the screen stays open.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_entry_row::SettingsEntryRow;
use crate::window::components::settings_screen::{
    SETTINGS_PANES_GRID_STYLE, SettingsPaneScaffold, settings_pane,
};
use crate::window::settings::entries::entry_id;

/// The one Sessions pane.
#[component]
pub fn SettingsSessions() -> Element {
    let Some(SettingsPaneScaffold {
        frame,
        onpress,
        cursor_id,
        ..
    }) = settings_pane()
    else {
        return rsx! {};
    };
    let GuiSettingsContent::Sessions {
        targets,
        refreshing,
    } = &frame.content
    else {
        return rsx! {};
    };
    let refreshing = *refreshing;

    rsx! {
        div {
            class: "grid",
            style: SETTINGS_PANES_GRID_STYLE,
            "data-settings-sessions": "",
            SettingsCard {
                title: Some("Playback targets".to_owned()),
                rows: true,
                for (index, target) in targets.iter().enumerate() {
                    {
                        let id = entry_id::target(index);
                        let row_press = onpress;
                        rsx! {
                            SettingsEntryRow {
                                key: "{id}",
                                id: id.clone(),
                                cursor: cursor_id.as_deref() == Some(id.as_str()),
                                onpress: row_press,
                                label: target.label.clone(),
                                trailing: target.is_active.then(|| "● Active".to_owned()),
                            }
                        }
                    }
                }
                {
                    // The Refresh control: the Sessions section's own button,
                    // below the targets. It sends the header chooser's Refresh
                    // command, so a press starts one discovery; while one runs
                    // the label says so and the button dims.
                    let id = entry_id::targets_refresh();
                    let row_press = onpress;
                    rsx! {
                        SettingsEntryRow {
                            key: "{id}",
                            id: id.clone(),
                            cursor: cursor_id.as_deref() == Some(id.as_str()),
                            onpress: row_press,
                            label: if refreshing { "Refreshing…" } else { "Refresh" },
                            trailing: None,
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
