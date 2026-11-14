//! One labelled text field of a Settings form (`replace-tauri-with-blitz`
//! task 8.2, the port of `SettingsField.svelte`; add-gui-settings-screen
//! design D13, D15): the canvas's small muted label above a mono input. The
//! Services setup forms and the Feeds form paint every field through it.

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
}

/// One labelled text field (the Svelte markup, character for character).
#[component]
pub fn SettingsField(props: SettingsFieldProps) -> Element {
    let oninput = props.oninput;
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
            }
        }
    }
}
