//! The Settings screen's sidebar (`replace-tauri-with-blitz` task 8.3, the
//! port of `SettingsSidebar`; add-gui-settings-screen design D11,
//! D13's canvas `nav`): the fixed section list at the left window edge.
//! It paints the frame's `sections` in order — a section whose every row the
//! GUI hides never arrives — each with its Lucide icon and its name, the
//! cursor the window content owns, and the open section's row in the accent
//! fill with ink text. A press selects the section, as Enter on the cursor
//! row does; in Mini the sidebar is the pane it replaces. The Back control
//! above the title closes the screen (design D11 *Mouse-centric*), so a user
//! can leave without a key, as Esc from the sidebar does.
//!
//! the former client sidebar's `scrollIntoView` effect ports as the cursor row's own
//! reveal ([`cursor_reveal`]): Blitz exposes no descendant query, so the row
//! that paints `data-settings-cursor` keeps itself in view (D15).

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsSection;
use crate::bridge::settings::GuiSettingsCommand;
use crate::window::components::cursor_reveal::cursor_reveal;
use crate::window::components::settings_screen::{
    section_label, section_wire, settings_section_view,
};
use crate::window::icons::{
    ArrowLeft, Cast, Clock, IconProps, Keyboard, Library, ListOrdered, Monitor, Play, Rss, Server,
    SquarePlay, Zap,
};
use crate::window::sections::Sections;
use crate::window::settings::state::{SettingsPane, SettingsState};

/// One section's sidebar icon: the
/// closest icon to the artboard's custom path, because the GUI's rule stays
/// Lucide only.
type SectionIcon = fn(IconProps) -> Element;

/// The props the sidebar's icons paint with.
fn sidebar_icon_defaults() -> IconProps {
    IconProps {
        class: String::new(),
        size: 16,
        color: "currentColor".to_owned(),
        fill: "none".to_owned(),
        stroke_width: 2.0,
        aria_hidden: false,
    }
}

/// Each section's icon.
fn section_icon(section: GuiSettingsSection) -> SectionIcon {
    match section {
        GuiSettingsSection::Services => Server,
        GuiSettingsSection::Playback => Play,
        GuiSettingsSection::Display => Monitor,
        GuiSettingsSection::Session => Clock,
        GuiSettingsSection::Library => Library,
        GuiSettingsSection::Queue => ListOrdered,
        GuiSettingsSection::Mpv => SquarePlay,
        GuiSettingsSection::Feeds => Rss,
        GuiSettingsSection::Actions => Zap,
        GuiSettingsSection::Keys => Keyboard,
        GuiSettingsSection::Sessions => Cast,
    }
}

/// The one sidebar.
#[component]
pub fn SettingsSidebar() -> Element {
    let sections = consume_context::<Sections>();
    let settings = consume_context::<SettingsState>();
    let Some(view) = settings_section_view(&sections, &settings) else {
        return rsx! {};
    };
    let ui = settings.ui.cloned();
    let press_settings = settings.clone();
    let back_settings = settings;
    rsx! {
        nav {
            class: "flex h-full min-h-0 flex-col",
            style: "background: var(--canvas-queue-column); border-right: 1px solid var(--canvas-border); padding: 20px 12px; gap: 2px;",
            "aria-label": "Settings sections",
            "data-settings-sidebar": "",
            button {
                "type": "button",
                class: "flex flex-none items-center text-left",
                style: "height: 32px; padding: 0 12px; gap: 8px; border: 0; border-radius: 8px; background: none; color: var(--canvas-muted); font-size: 14px;",
                "data-settings-back": "",
                onclick: move |_| back(&back_settings),
                span {
                    class: "flex flex-none items-center",
                    "aria-hidden": "true",
                    ArrowLeft { size: 16 }
                }
                "Back"
            }
            div {
                class: "flex flex-none items-baseline",
                style: "padding: 0 12px 14px; font-size: 20px; font-weight: 600; color: var(--canvas-heading);",
                "data-settings-sidebar-title": "",
                "Settings"
            }
            div {
                class: "min-h-0 flex-1 overflow-y-auto",
                for (index, section) in view.frame.sections.iter().enumerate() {
                    {
                        let current = *section == view.frame.section;
                        let cursor = index == ui.sidebar_cursor
                            && ui.pane == SettingsPane::Sidebar
                            && ui.keyboard;
                        let row_settings = press_settings.clone();
                        let pressed_section = *section;
                        rsx! {
                            SidebarRow {
                                key: "{section_wire(pressed_section)}",
                                section: pressed_section,
                                current,
                                cursor,
                                onpress: Callback::new(move |()| press_section(&row_settings, index, pressed_section)),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One sidebar row.
/// The listing renders one component per row, so the cursor reveal's hooks
/// stay per row: a hook in the loop itself would change the listing
/// component's hook count with the section count.
#[component]
fn SidebarRow(
    section: GuiSettingsSection,
    current: bool,
    cursor: bool,
    onpress: Callback<()>,
) -> Element {
    let label = section_label(section);
    let icon = section_icon(section);
    // The row that paints the cursor reveals itself: the former sidebar's
    // `scrollIntoView` on its `[data-settings-cursor]` row (D15).
    let mut reveal = cursor_reveal(cursor);
    rsx! {
        button {
            "type": "button",
            class: "flex w-full items-center text-left",
            style: "height: 38px; padding: 0 12px; gap: 12px; border: 0; border-radius: 8px; font-size: 14px; background: {background(current, cursor)}; color: {ink_or_text(current)}; font-weight: {weight(current)};",
            "aria-current": current.then_some("page"),
            "data-settings-section": section_wire(section),
            "data-settings-cursor": cursor.then_some(""),
            onmounted: move |event: Event<MountedData>| reveal.set(Some(event)),
            onclick: move |_| onpress.call(()),
            span {
                class: "flex flex-none items-center",
                style: "color: {ink_or_muted(current)};",
                "aria-hidden": "true",
                { icon(IconProps {
                    size: 16,
                    ..sidebar_icon_defaults()
                }) }
            }
            "{label}"
        }
    }
}

/// The row's fill: the open section's accent, the cursor's selected tone, or
/// none.
fn background(current: bool, cursor: bool) -> &'static str {
    if current {
        "var(--canvas-accent)"
    } else if cursor {
        "var(--canvas-selected)"
    } else {
        "none"
    }
}

/// The row's text colour: ink on the open section, the canvas text otherwise.
fn ink_or_text(current: bool) -> &'static str {
    if current {
        "var(--canvas-ink)"
    } else {
        "var(--canvas-text)"
    }
}

/// The row's icon colour: ink on the open section, the muted tone otherwise.
fn ink_or_muted(current: bool) -> &'static str {
    if current {
        "var(--canvas-ink)"
    } else {
        "var(--canvas-muted)"
    }
}

/// The row's weight: 600 on the open section, 400 otherwise.
fn weight(current: bool) -> &'static str {
    if current { "600" } else { "400" }
}

/// A press on a section row: the cursor
/// takes the row and the keyboard takes the sidebar, then the open command
/// the Enter key sends moves the keyboard to the content — in Mini the
/// content is what then shows (the spec's *Narrow window* requirement).
fn press_section(settings: &SettingsState, index: usize, section: GuiSettingsSection) {
    let mut ui = settings.ui;
    ui.with_mut(|ui| {
        ui.pane = SettingsPane::Sidebar;
        ui.sidebar_cursor = index;
    });
    settings.select_settings_section(section);
}

/// Back closes the screen, as Esc from the sidebar does (the spec's *A Back
/// control in the sidebar closes the screen*).
fn back(settings: &SettingsState) {
    settings.link.settings(GuiSettingsCommand::Close);
}

#[cfg(test)]
mod tests;
