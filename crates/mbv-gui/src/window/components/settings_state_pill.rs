//! A Service's state (`replace-tauri-with-blitz` task 8.2, the port of
//! `SettingsStatePill`; add-gui-settings-screen design D13, D15, the
//! canvas state pill): a dot and the state's label in a 24 px pill. Ready
//! paints the success fill and the accent tone; Not configured, Connecting,
//! Needs authentication, and Unavailable paint the muted, source, partial,
//! and destructive tones as text, so one label grammar serves every state.

use dioxus_native::prelude::*;

/// Each state's tone: the dot and the
/// label share it. Not configured and any state the map does not name paint
/// the muted tone.
fn tone(state: &str) -> &'static str {
    match state {
        "ready" => "var(--canvas-accent)",
        "connecting" => "var(--canvas-source)",
        "needs authentication" => "var(--canvas-partial)",
        "unavailable" => "var(--canvas-destructive)",
        _ => "var(--canvas-muted)",
    }
}

/// The props of the state pill (the `SettingsStatePill` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsStatePillProps {
    /// The Service's state label, as the frame carries it.
    pub state: String,
}

/// The one state pill.
#[component]
pub fn SettingsStatePill(props: SettingsStatePillProps) -> Element {
    let lower = props.state.to_lowercase();
    let ready = lower == "ready";
    let tone = tone(&lower);
    let style = format!(
        "height: 24px; padding: 0 10px; gap: 6px; border-radius: 12px; background: {}; color: {}; font-size: 12px; font-weight: 500;",
        if ready {
            "var(--canvas-success-fill)"
        } else {
            "none"
        },
        tone,
    );
    rsx! {
        span {
            class: "flex flex-none items-center",
            style: "{style}",
            "data-settings-state-pill": "{props.state}",
            span {
                "aria-hidden": "true",
                style: "width: 7px; height: 7px; border-radius: 4px; background: {tone};",
            }
            "{props.state}"
        }
    }
}
