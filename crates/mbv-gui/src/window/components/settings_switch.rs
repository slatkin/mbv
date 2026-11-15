//! One Settings toggle (`replace-tauri-with-blitz` task 8.2, the port of
//! `SettingsSwitch`; add-gui-settings-screen design D13, D15, the
//! canvas switch): a 44 by 24 px track with an 18 px knob, `role="switch"`
//! and `aria-checked` for assistive technology, and the accent fill while it
//! is on. A press runs the same store function the keyboard's Space runs, so
//! the two paths share one effect (design D11's pointer contract).

use dioxus_native::prelude::*;

/// The props of the switch (the `SettingsSwitch` props, in order).
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

/// The one Settings switch.
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
    let disabled = props.disabled;
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
            onclick: move |_| {
                press(disabled, onpress);
            },
            span {
                "aria-hidden": "true",
                class: "absolute",
                style: "{knob}",
            }
        }
    }
}

/// The switch's press (`onclick`): a disabled toggle takes no press —
/// blitz-dom dispatches Click on disabled targets, so the handler returns
/// before the activation — and an enabled one runs `onpress`, the same store
/// function the keyboard's Space runs (design D11's pointer contract).
fn press(disabled: bool, onpress: Callback<()>) {
    if disabled {
        return;
    }
    onpress.call(());
}

#[cfg(test)]
mod tests;
