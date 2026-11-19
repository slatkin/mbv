//! The Settings screen's status line (`replace-tauri-with-blitz` task 8.3,
//! the port of `SettingsStatusBar`; add-gui-settings-screen design D9,
//! the spec's *The Settings screen shows the toasts*; the canvas's status
//! bar, design D13): the 36 px band that closes the screen, painting the
//! shared Status bar projection — the armed prefix and the live toast —
//! because neither the footer line nor the Library Status bar shows while
//! the screen is open. The left side is the screen's fixed key hints in the
//! canvas's source tone while no toast or prefix is live; the right side is
//! the shown section.

use dioxus_native::prelude::*;

use crate::bridge::frame::chrome::{GuiStatusBar, GuiToast};
use crate::window::status::toast_color;

/// The screen's fixed keys (design D11), which `[keys]` does not configure,
/// so the text is fixed.
const HINTS: &[(&str, &str)] = &[
    ("↑↓", "move"),
    ("⏎", "open"),
    ("space", "toggle"),
    ("tab", "pane"),
    ("esc", "back"),
];

/// The props of the status line (the `SettingsStatusBar` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct SettingsStatusBarProps {
    /// The frame's Status bar facts: the armed prefix.
    pub status: GuiStatusBar,
    /// The newest live toast, `None` while none is live. It rides the `toasts`
    /// section now (task row 2.1); the inline paint stays until its own row
    /// removes it.
    #[props(default)]
    pub toast: Option<GuiToast>,
    /// The shown section's sidebar label.
    pub section: String,
}

/// The one status line.
#[component]
pub fn SettingsStatusBar(props: SettingsStatusBarProps) -> Element {
    // The live toast's severity colour, resolved before the markup (the
    // the former client `toastColor` call in the attribute).
    let toast_style = props.toast.as_ref().map_or(String::new(), |toast| {
        format!("color: {};", toast_color(toast.severity))
    });
    rsx! {
        div {
            class: "flex flex-none items-center justify-between overflow-hidden font-mono whitespace-nowrap",
            style: "height: 36px; padding-inline: 16px; font-size: 12px; border-top: 1px solid var(--canvas-border); background: var(--canvas-library-panel);",
            "data-settings-status": "",
            span {
                class: "flex items-center overflow-hidden",
                style: "gap: 12px;",
                "data-settings-status-hints": "",
                if props.status.prefix_armed {
                    span {
                        class: "font-bold",
                        style: "color: var(--mbv-accent);",
                        "data-settings-status-prefix": "",
                        "PREFIX"
                    }
                }
                if let Some(toast) = props.toast.as_ref() {
                    span {
                        style: "{toast_style}",
                        "data-settings-status-toast": "",
                        "{toast.text}"
                    }
                }
                if props.toast.is_none() && !props.status.prefix_armed {
                    span {
                        class: "flex items-center",
                        style: "gap: 12px; color: var(--canvas-muted);",
                        for hint in HINTS {
                            {
                                let (key, label) = *hint;
                                rsx! {
                                    span {
                                        key: "{key}",
                                        "data-settings-hint": "",
                                        span { style: "color: var(--canvas-source);", "{key}" }
                                        " {label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            span {
                class: "overflow-hidden",
                style: "color: var(--canvas-muted);",
                "data-settings-status-section": "",
                "{props.section}"
            }
        }
    }
}

#[cfg(test)]
mod tests;
