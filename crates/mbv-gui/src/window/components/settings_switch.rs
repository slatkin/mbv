//! One Settings toggle (`replace-tauri-with-blitz` task 8.2, the port of
//! `SettingsSwitch.svelte`; add-gui-settings-screen design D13, D15, the
//! canvas switch): a 44 by 24 px track with an 18 px knob, `role="switch"`
//! and `aria-checked` for assistive technology, and the accent fill while it
//! is on. A press runs the same store function the keyboard's Space runs, so
//! the two paths share one effect (design D11's pointer contract).

use dioxus_native::prelude::*;

/// The props of the switch (the `SettingsSwitch.svelte` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsSwitchProps {
    /// The toggle's accessible name.
    pub label: String,
    /// Whether the toggle is on.
    pub checked: bool,
    /// Whether the toggle cannot be operated.
    #[props(default)]
    pub disabled: bool,
    /// A press on the track.
    pub onpress: Callback<()>,
}

/// The one Settings switch (the Svelte markup, character for character).
#[component]
pub fn SettingsSwitch(props: SettingsSwitchProps) -> Element {
    let checked = props.checked;
    let onpress = props.onpress;
    let track = format!(
        "width: 44px; height: 24px; padding: 0; border: 0; border-radius: 12px; background: {}; transition: background 120ms;",
        if checked {
            "var(--canvas-accent)"
        } else {
            "var(--canvas-segment-active)"
        },
    );
    let knob = format!(
        "top: 3px; left: {}; width: 18px; height: 18px; border-radius: 9px; background: {}; transition: left 120ms;",
        if checked { "23px" } else { "3px" },
        if checked {
            "var(--canvas-ink)"
        } else {
            "var(--canvas-text)"
        },
    );
    rsx! {
        button {
            "type": "button",
            role: "switch",
            "aria-checked": "{checked}",
            "aria-label": "{props.label}",
            class: "relative flex-none",
            style: "{track}",
            disabled: props.disabled,
            "data-settings-switch": "",
            onclick: move |_| onpress.call(()),
            span {
                "aria-hidden": "true",
                class: "absolute",
                style: "{knob}",
            }
        }
    }
}
