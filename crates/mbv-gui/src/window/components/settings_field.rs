//! One labelled text field of a Settings form (`replace-tauri-with-blitz`
//! task 8.2, the port of `SettingsField.svelte`; add-gui-settings-screen
//! design D13, D15): the canvas's small muted label above a mono input. The
//! Services setup forms and the Feeds form paint every field through it.
//!
//! The optional `onmounted` passthrough hands the input's mount event to the
//! caller, so a form can attach `window/settings/focus.rs`'s
//! `focus_on_mount` to its first field's input (task 8.4) — the Svelte
//! `focusOnMount` action focused the first `input` descendant, and the
//! passthrough is the port's stand-in for that descendant query.

use dioxus_native::prelude::*;

/// The props of the field (the `SettingsField.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsFieldProps {
    /// The label above the input.
    pub label: String,
    /// The input's current draft value.
    pub value: String,
    /// The `data-settings-field` name the tests and the key router read.
    pub field: String,
    /// The input's accessible name.
    pub aria_label: String,
    /// The input's placeholder; the label is the default.
    #[props(default)]
    pub placeholder: Option<String>,
    /// `password` for a secret the draft clears on submit.
    #[props(default)]
    pub secret: bool,
    /// The input's own event: the new value.
    pub oninput: Callback<String>,
    /// The input's mount event, while the caller attaches one — the first
    /// field of a form passes `focus_on_mount` (task 8.4).
    #[props(default)]
    pub onmounted: Option<Callback<Event<MountedData>>>,
}

/// One labelled text field (the Svelte markup, character for character).
#[component]
pub fn SettingsField(props: SettingsFieldProps) -> Element {
    let oninput = props.oninput;
    let onmounted = props.onmounted;
    rsx! {
        label {
            class: "flex flex-col",
            style: "gap: 6px; font-size: 12px; color: var(--canvas-muted);",
            "{props.label}"
            input {
                "type": if props.secret { "password" } else { "text" },
                placeholder: props.placeholder.unwrap_or_else(|| props.label.clone()),
                "aria-label": "{props.aria_label}",
                value: "{props.value}",
                "data-settings-field": "{props.field}",
                oninput: move |event: Event<FormData>| oninput.call(event.value()),
                onmounted: move |event: Event<MountedData>| {
                    if let Some(onmounted) = onmounted {
                        onmounted.call(event);
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests;
