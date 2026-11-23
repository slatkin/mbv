//! One activatable row of a Settings section (`replace-tauri-with-blitz`
//! task 8.3, the port of `SettingsEntryRow`; add-gui-settings-screen
//! design D11): the label, an optional leading mark, and an optional
//! trailing value, in the one row look every section paints — 36 px inside
//! the card that holds it, with no side padding of its own. The cursor fill
//! paints only while the content pane holds the keyboard (`cursor` names the
//! row the cursor rests on). A press reports to the section's press handler,
//! which moves the cursor and runs the entry's activation.
//!
//! The row paints `disabled` for a control that activates nothing (a device
//! with no endpoint) but does not gate its press on it: the entries the
//! press resolves already no-op that arm (`SettingsState`'s
//! `activate_settings_entry`), so the paint and the effect stay one fact.

use dioxus_native::prelude::*;

use crate::window::components::cursor_reveal::cursor_reveal;
use crate::window::icons::Check;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the row (the `SettingsEntryRow` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsEntryRowProps {
    /// The entry's id (`entryId` in the settings entries), which the press
    /// handler and the cursor fact read.
    pub id: String,
    /// Whether the content cursor rests on this row while the content pane
    /// holds the keyboard.
    pub cursor: bool,
    /// A press on the row: the entry's id.
    pub onpress: Callback<String>,
    /// The row's label.
    pub label: String,
    /// The right-side value text, if any.
    #[props(default)]
    pub trailing: Option<String>,
    /// A choice mark: checked, unchecked, or absent.
    #[props(default)]
    pub marked: Option<bool>,
    /// Whether the row activates nothing (a device with no endpoint).
    #[props(default)]
    pub disabled: bool,
    /// Indent the row under its group header.
    #[props(default)]
    pub indent: bool,
    /// A glyph painted before the label (the Sessions Refresh icon), if any.
    #[props(default)]
    pub leading: Option<Element>,
}

/// The one entry row.
#[component]
pub fn SettingsEntryRow(props: SettingsEntryRowProps) -> Element {
    let indent = props.indent;
    let disabled = props.disabled;
    let cursor = props.cursor;
    let marked = props.marked;
    let onpress = props.onpress;
    let press_id = props.id.clone();
    let label = props.label;
    let trailing = props.trailing;
    let leading = props.leading;
    // The row that paints the cursor reveals itself: the former screen's
    // `scrollIntoView` on the pane's `[data-settings-cursor]` row (D15).
    let mut reveal = cursor_reveal(cursor);
    rsx! {
        button {
            "type": "button",
            class: "flex w-full items-center text-left",
            style: "height: 36px; padding: 0; padding-left: {indent.then_some(\"16px\").unwrap_or(\"0\")}; gap: 8px; font-size: 14px; background: {row_background(cursor)}; border: 0; color: {text_color(disabled)};{opacity(disabled)}",
            disabled: props.disabled,
            "data-settings-entry": "{props.id}",
            "data-settings-cursor": cursor.then_some(""),
            onmounted: move |event: Event<MountedData>| reveal.set(Some(event)),
            onclick: move |_| onpress.call(press_id.clone()),
            if let Some(marked) = marked {
                span {
                    class: "flex flex-none items-center",
                    style: "width: 18px; color: {mark_color(marked)};",
                    "aria-hidden": "true",
                    if marked {
                        Check { size: 14 }
                    }
                }
            }
            if let Some(leading) = leading {
                span {
                    class: "flex flex-none items-center",
                    "aria-hidden": "true",
                    {leading}
                }
            }
            Truncate {
                text: label,
                class: "min-w-0 flex-1".to_owned(),
                style: TextStyle::sans(14.0, 400.0),
                css: "font-size: 14px;".to_owned(),
            }
            if let Some(trailing) = trailing {
                Truncate {
                    text: trailing,
                    class: "flex-none".to_owned(),
                    style: TextStyle::sans(14.0, 400.0),
                    css: "font-size: 14px; color: var(--canvas-muted);".to_owned(),
                }
            }
        }
    }
}

/// The row's fill: the cursor's selected tone, or none.
fn row_background(cursor: bool) -> &'static str {
    if cursor {
        "var(--canvas-selected)"
    } else {
        "none"
    }
}

/// The row's text colour: the ordinal tone while disabled, the canvas text
/// otherwise.
fn text_color(disabled: bool) -> &'static str {
    if disabled {
        "var(--canvas-ordinal)"
    } else {
        "var(--canvas-text)"
    }
}

/// The disabled row's dimming.
fn opacity(disabled: bool) -> &'static str {
    if disabled { " opacity: 0.7;" } else { "" }
}

/// The mark's colour: the accent while checked, the muted tone otherwise.
fn mark_color(marked: bool) -> &'static str {
    if marked {
        "var(--canvas-accent)"
    } else {
        "var(--canvas-muted)"
    }
}
