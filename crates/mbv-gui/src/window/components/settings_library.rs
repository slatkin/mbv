//! The Library section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsLibrary`; add-gui-settings-screen design D9, D13): the
//! section's choice rows (the hidden and the feed-view libraries) as
//! checkbox-grid cards, then the routes list in a card of its own — each
//! routable library with its assigned device, and, once a `routeDevices`
//! load lands, the selected library's candidates with the automatic entry
//! that clears the route. A device with no direct-connect endpoint paints
//! disabled: it is not currently routable, so a press on it assigns nothing.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_entry_row::SettingsEntryRow;
use crate::window::components::settings_rows::SettingsRows;
use crate::window::components::settings_screen::{SettingsPaneScaffold, settings_pane};
use crate::window::settings::entries::entry_id;

/// The one Library pane.
#[component]
pub fn SettingsLibrary() -> Element {
    let Some(SettingsPaneScaffold {
        frame,
        onpress,
        cursor_id,
        ..
    }) = settings_pane()
    else {
        return rsx! {};
    };
    let GuiSettingsContent::Library {
        routes,
        selected,
        devices,
        ..
    } = &frame.content
    else {
        return rsx! {};
    };

    rsx! {
        div {
            class: "flex flex-col",
            style: "gap: 20px;",
            "data-settings-library": "",
            SettingsRows {}
            if let Some(routes) = routes.as_ref() {
                SettingsCard {
                    title: Some("Library routes".to_owned()),
                    rows: true,
                    for route in routes {
                        {
                            let id = entry_id::route(&route.library);
                            let row_press = onpress;
                            rsx! {
                                SettingsEntryRow {
                                    key: "{id}",
                                    id: id.clone(),
                                    cursor: cursor_id.as_deref() == Some(id.as_str()),
                                    onpress: row_press,
                                    label: route.name.clone(),
                                    trailing: Some(route.endpoint.clone().unwrap_or_else(|| "Automatic".to_owned())),
                                }
                            }
                        }
                    }
                    if let Some(library) = selected.as_ref() {
                        {
                            let clear_id = entry_id::route_clear(library);
                            let clear_press = onpress;
                            rsx! {
                                SettingsEntryRow {
                                    key: "{clear_id}",
                                    id: clear_id.clone(),
                                    cursor: cursor_id.as_deref() == Some(clear_id.as_str()),
                                    onpress: clear_press,
                                    label: "Automatic".to_owned(),
                                    indent: true,
                                }
                            }
                        }
                        for device in devices.iter().flatten() {
                            {
                                let device_id = entry_id::route_device(library, &device.name);
                                let device_press = onpress;
                                rsx! {
                                    SettingsEntryRow {
                                        key: "{device_id}",
                                        id: device_id.clone(),
                                        cursor: cursor_id.as_deref() == Some(device_id.as_str()),
                                        onpress: device_press,
                                        label: device.name.clone(),
                                        trailing: (device.endpoint.is_none()).then(|| "Not routable".to_owned()),
                                        disabled: device.endpoint.is_none(),
                                        indent: true,
                                    }
                                }
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
