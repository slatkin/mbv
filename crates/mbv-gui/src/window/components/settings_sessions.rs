//! The Sessions section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsSessions.svelte`; add-gui-settings-screen design D9, D13): the
//! same playback targets the header's chooser lists, each marked while it is
//! the Owner's attached output, in a card of 36 px rows. Selecting one sends
//! `selectTarget` — the chooser's own path — and the screen stays open.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_entry_row::SettingsEntryRow;
use crate::window::components::settings_screen::settings_section_view;
use crate::window::sections::Sections;
use crate::window::settings::entries::entry_id;
use crate::window::settings::state::SettingsState;

/// The one Sessions pane (the Svelte markup, character for character).
#[component]
pub fn SettingsSessions() -> Element {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let Some(view) = settings_section_view(&sections, &settings) else {
        return rsx! {};
    };
    let GuiSettingsContent::Sessions { targets } = &view.frame.content else {
        return rsx! {};
    };

    // A press on a playback target: the entry's activation, the same one the
    // keyboard's Enter runs — `selectTarget` with the target's stable key.
    let press_settings = settings.clone();
    let press_entries = view.entries;
    let onpress = Callback::new(move |id: String| {
        press_settings.press_settings_entry(&press_entries, &id);
    });
    let cursor_id = view.cursor_id.clone();

    rsx! {
        div {
            class: "grid",
            style: "grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px; align-items: start;",
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
            }
        }
    }
}

#[cfg(test)]
mod tests;
