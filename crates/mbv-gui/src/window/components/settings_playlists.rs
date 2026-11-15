//! The Playlists section (`replace-tauri-with-blitz` task 8.4, the port of
//! `SettingsPlaylists`; add-gui-settings-screen design D9, D13): the
//! saved playlists in a card, and the open playlist's items in one. Each
//! playlist offers the TUI's actions — open, play, shuffle, add, rename,
//! delete, refresh — and each item play, shuffle, and add, all as the
//! canvas's buttons on 36 px rows. The rename field is the window content's
//! draft; a delete raises the core's confirm.
//!
//! The rename host owns the keyboard hold's focus facts (task 8.4's wiring
//! requirement): its `focusin` re-arms the hold and its `focusout` drops it,
//! and the field mounts with `focus_on_mount`.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::settings::GuiPlaylistAction;
use crate::window::components::settings_button::{SettingsButton, SettingsButtonVariant};
use crate::window::components::settings_card::SettingsCard;
use crate::window::components::settings_entry_row::SettingsEntryRow;
use crate::window::components::settings_screen::settings_section_view;
use crate::window::sections::Sections;
use crate::window::settings::entries::entry_id;
use crate::window::settings::focus::focus_on_mount;
use crate::window::settings::state::{SettingsForm, SettingsState};
use crate::window::truncate::{TextStyle, Truncate};

/// The three whole-playlist and whole-item actions, in paint order (the
/// the former client `ACTIONS` list).
const ACTIONS: &[(GuiPlaylistAction, &str)] = &[
    (GuiPlaylistAction::Play, "Play"),
    (GuiPlaylistAction::Shuffle, "Shuffle"),
    (GuiPlaylistAction::Enqueue, "Add"),
];

/// The one Playlists pane.
#[component]
pub fn SettingsPlaylists() -> Element {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let Some(view) = settings_section_view(&sections, &settings) else {
        return rsx! {};
    };
    let GuiSettingsContent::Playlists {
        playlists,
        open,
        loading,
        open_loading,
    } = &view.frame.content
    else {
        return rsx! {};
    };

    // A press on a playlist row, an action, a rename, a delete, a refresh,
    // or Back: the entry's activation, the same one the keyboard's Enter
    // runs.
    let press_settings = settings.clone();
    let press_entries = view.entries;
    let onpress = Callback::new(move |id: String| {
        press_settings.press_settings_entry(&press_entries, &id);
    });
    let cursor_id = view.cursor_id.clone();
    let rename = settings.ui.cloned().rename;

    rsx! {
        div {
            class: "grid",
            style: "grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px; align-items: start;",
            "data-settings-playlists": "",
            if let Some(open) = open.as_ref() {
                SettingsCard {
                    title: Some(open.name.clone()),
                    header: Some(rsx! {
                        div {
                            class: "flex",
                            style: "gap: 8px;",
                            {
                                let back_press = onpress;
                                let back_id = entry_id::playlist_back();
                                rsx! {
                                    SettingsButton {
                                        variant: SettingsButtonVariant::Ghost,
                                        entry: Some(back_id.clone()),
                                        cursor: cursor_id.as_deref() == Some(back_id.as_str()),
                                        onpress: Some(Callback::new(move |()| back_press.call(back_id.clone()))),
                                        "‹ Back"
                                    }
                                }
                            }
                            {
                                let refresh_press = onpress;
                                let refresh_id = entry_id::playlist_refresh(Some(&open.id));
                                rsx! {
                                    SettingsButton {
                                        entry: Some(refresh_id.clone()),
                                        cursor: cursor_id.as_deref() == Some(refresh_id.as_str()),
                                        onpress: Some(Callback::new(move |()| refresh_press.call(refresh_id.clone()))),
                                        { refresh_label(*open_loading) }
                                    }
                                }
                            }
                        }
                    }),
                    { playlist_actions(onpress, cursor_id.as_deref(), &open.id) }
                    if rename.as_ref().is_some_and(|draft| draft.id == open.id) {
                        { rename_field(&settings, rename.as_ref().map(|draft| draft.name.clone())) }
                    }
                    div {
                        class: "flex flex-col",
                        for item in &open.items {
                            {
                                let item_name = item.name.clone();
                                rsx! {
                                    div {
                                        key: "{item.id}",
                                        class: "flex items-center",
                                        style: "height: 36px; gap: 8px;",
                                        Truncate {
                                            text: item_name,
                                            class: "min-w-0 flex-1".to_owned(),
                                            style: TextStyle::sans(14.0, 400.0),
                                            css: "font-size: 14px; color: var(--canvas-text);".to_owned(),
                                        }
                                        for (action, label) in ACTIONS {
                                            {
                                                let action_id = entry_id::playlist_item_action(&open.id, &item.id, *action);
                                                let action_press = onpress;
                                                rsx! {
                                                    SettingsButton {
                                                        key: "{action_id}",
                                                        entry: Some(action_id.clone()),
                                                        cursor: cursor_id.as_deref() == Some(action_id.as_str()),
                                                        onpress: Some(Callback::new(move |()| action_press.call(action_id.clone()))),
                                                        "{label}"
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
            } else {
                SettingsCard {
                    title: Some("Playlists".to_owned()),
                    header: Some(rsx! {
                        {
                            let refresh_press = onpress;
                            let refresh_id = entry_id::playlist_refresh(None);
                            rsx! {
                                SettingsButton {
                                    entry: Some(refresh_id.clone()),
                                    cursor: cursor_id.as_deref() == Some(refresh_id.as_str()),
                                    onpress: Some(Callback::new(move |()| refresh_press.call(refresh_id.clone()))),
                                    { refresh_label(*loading) }
                                }
                            }
                        }
                    }),
                    if playlists.is_empty() {
                        div {
                            style: "font-size: 13px; color: var(--canvas-muted);",
                            "No saved playlists"
                        }
                    }
                    div {
                        class: "flex flex-col",
                        for playlist in playlists {
                            {
                                let row_id = entry_id::playlist(&playlist.id);
                                let row_press = onpress;
                                rsx! {
                                    SettingsEntryRow {
                                        key: "{row_id}",
                                        id: row_id.clone(),
                                        cursor: cursor_id.as_deref() == Some(row_id.as_str()),
                                        onpress: row_press,
                                        label: playlist.name.clone(),
                                        trailing: Some(format!("{} items", playlist.item_count)),
                                    }
                                    { playlist_actions(onpress, cursor_id.as_deref(), &playlist.id) }
                                    if rename.as_ref().is_some_and(|draft| draft.id == playlist.id) {
                                        { rename_field(&settings, rename.as_ref().map(|draft| draft.name.clone())) }
                                    }
                                    div {
                                        style: "height: 8px;",
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

/// A playlist's action row: the three
/// whole-playlist actions, Rename, and Delete.
fn playlist_actions(onpress: Callback<String>, cursor_id: Option<&str>, id: &str) -> Element {
    rsx! {
        div {
            class: "flex flex-wrap",
            style: "gap: 6px;",
            for (action, label) in ACTIONS {
                {
                    let action_id = entry_id::playlist_action(id, *action);
                    let action_press = onpress;
                    rsx! {
                        SettingsButton {
                            key: "{action_id}",
                            entry: Some(action_id.clone()),
                            cursor: cursor_id == Some(action_id.as_str()),
                            onpress: Some(Callback::new(move |()| action_press.call(action_id.clone()))),
                            "{label}"
                        }
                    }
                }
            }
            {
                let rename_id = entry_id::playlist_rename(id);
                let rename_press = onpress;
                rsx! {
                    SettingsButton {
                        entry: Some(rename_id.clone()),
                        cursor: cursor_id == Some(rename_id.as_str()),
                        onpress: Some(Callback::new(move |()| rename_press.call(rename_id.clone()))),
                        "Rename"
                    }
                }
            }
            {
                let delete_id = entry_id::playlist_delete(id);
                let delete_press = onpress;
                rsx! {
                    SettingsButton {
                        variant: SettingsButtonVariant::Destructive,
                        entry: Some(delete_id.clone()),
                        cursor: cursor_id == Some(delete_id.as_str()),
                        onpress: Some(Callback::new(move |()| delete_press.call(delete_id.clone()))),
                        "Delete"
                    }
                }
            }
        }
    }
}

/// The rename field: the draft's input
/// under its focus host, mounted with `focus_on_mount`, which arms the
/// rename hold and focuses the input.
fn rename_field(settings: &SettingsState, name: Option<String>) -> Element {
    let focus_settings = settings.clone();
    let blur_settings = settings.clone();
    let input_settings = settings.clone();
    let arm_settings = settings.clone();
    rsx! {
        div {
            class: "flex",
            style: "padding: 4px 0;",
            "data-settings-rename": "",
            onfocusin: move |_| focus_settings.focus_settings_field(SettingsForm::Rename),
            onfocusout: move |_| blur_settings.blur_settings_field(SettingsForm::Rename),
            input {
                "type": "text",
                "aria-label": "New playlist name",
                style: "flex: 1;",
                value: name.unwrap_or_default(),
                "data-settings-field": "rename",
                onmounted: move |event: Event<MountedData>| {
                    focus_on_mount(event, &arm_settings, SettingsForm::Rename);
                },
                oninput: move |event: Event<FormData>| {
                    let value = event.value();
                    input_settings.set_rename_draft(&value);
                },
            }
        }
    }
}

/// A refresh control's painted text (`SettingsPlaylists`'s
/// `refreshLabel`): the load flag while it runs.
fn refresh_label(loading: bool) -> &'static str {
    if loading { "Refreshing…" } else { "Refresh" }
}

#[cfg(test)]
mod tests;
