//! The daemon-lost recovery dialog the Client core raised
//! (`replace-tauri-with-blitz` task 4.2, the port of
//! `DaemonLostOverlay`; `App::raise_daemon_lost_modal`,
//! `daemon-disconnect-handling`): the diagnostics and the recovery choices
//! the frame's `daemonLost` overlay carries, painted window-modal over every
//! panel in the confirmation modal's own look.
//!
//! The dialog blocks every other input until one of its choices runs: the
//! core's overlay key rules bind its three chords (and nothing else, Esc
//! included), and a press on a button forwards like every other pointer
//! event — the core resolves it against the hit map and answers with the
//! choice the index names.
//!
//! The buttons report their `daemonLost` regions under this component's own
//! report key, so the core's hit map names them while the dialog paints; the
//! contribution retracts on unmount.

use std::collections::HashMap;

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiConfirmButton;
use crate::bridge::layout::{DaemonLostTargetId, RegionTarget};
use crate::window::components::dialog_frame::{BUTTON_FILL, button_color};
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};

/// The dialog's props: the diagnostics and the recovery choices the
/// `daemonLost` overlay carries.
#[derive(Props, Clone, PartialEq)]
pub struct DaemonLostOverlayProps {
    /// The title of whatever was playing right before the Owner vanished;
    /// `None` when nothing was.
    #[props(default)]
    pub last_playing_title: Option<String>,
    /// The local daemon's log path, painted as the dialog's log line.
    pub daemon_log_path: String,
    /// The last restart attempt's failure, painted as the dialog's
    /// `Restart failed:` line.
    #[props(default)]
    pub restart_error: Option<String>,
    /// The recovery choices, in the core's order.
    pub buttons: Vec<GuiConfirmButton>,
}

impl std::fmt::Debug for DaemonLostOverlayProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaemonLostOverlayProps")
            .field("last_playing_title", &self.last_playing_title)
            .field("daemon_log_path", &self.daemon_log_path)
            .field("restart_error", &self.restart_error)
            .field("buttons", &self.buttons)
            .finish()
    }
}

/// The dialog (task 4.2). The core raised it; the component paints and
/// reports, and the core resolves a press or chord.
#[component]
pub fn DaemonLostOverlay(props: DaemonLostOverlayProps) -> Element {
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);
    let mut mounted_buttons: Signal<HashMap<usize, Event<MountedData>>> = use_signal(HashMap::new);

    // No region outlives the dialog: the contribution retracts on unmount.
    let mut drop_reporter = reporter.clone();
    use_drop(move || drop_reporter.retract_layout(key));

    // Each paint that moved a region reports (design D4): the frame's fields,
    // the buttons' mounts, and every layout fact (the epoch) re-measure. A
    // resize moves the dialog's buttons, so the epoch subscription keeps their
    // regions matched to the layout that painted them.
    let epoch = try_consume_context::<LayoutEpoch>();
    use_effect(use_reactive(
        (
            &props.last_playing_title.clone(),
            &props.restart_error.clone(),
            &props.buttons.clone(),
        ),
        move |(_last_playing_title, _restart_error, buttons)| {
            let mut mounted_buttons = mounted_buttons;
            let mut effect_reporter = reporter.clone();
            let _epoch = epoch.as_ref().map(|epoch| epoch.0.cloned());
            spawn(async move {
                // A choice the current frame does not paint owns no region.
                let count = buttons.len();
                mounted_buttons.write().retain(|index, _| *index < count);
                let mut indexes: Vec<usize> = mounted_buttons.peek().keys().copied().collect();
                indexes.sort_unstable();
                let mut regions = Vec::new();
                for index in indexes {
                    let event = mounted_buttons.peek().get(&index).cloned();
                    if let Some(event) = event {
                        regions.extend(
                            element_region(
                                &event,
                                RegionTarget::DaemonLost(DaemonLostTargetId::Button(index)),
                            )
                            .await,
                        );
                    }
                }
                effect_reporter.report_layout(
                    key,
                    LayoutContribution {
                        lists: Vec::new(),
                        regions,
                    },
                );
            });
        },
    ));

    rsx! {
        div {
            class: "absolute inset-0 z-30 flex items-center justify-center",
            style: "background: var(--canvas-modal-scrim);",
            "data-daemon-lost-overlay": "true",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "The local daemon connection was lost",
            div {
                class: "flex flex-col",
                style: format!(
                    "{MENU_CARD_STYLE} padding: 16px; gap: 12px; max-width: 420px;"
                ),
                p {
                    style: "font-size: 13px; color: var(--canvas-heading);",
                    "The local daemon connection was lost unexpectedly."
                }
                if let Some(title) = props.last_playing_title.as_ref() {
                    p {
                        style: "font-size: 12px; color: var(--canvas-text-secondary);",
                        "Was playing: {title}"
                    }
                }
                p {
                    style: "font-size: 12px; color: var(--canvas-text-secondary);",
                    "Daemon log: {props.daemon_log_path}"
                }
                if let Some(error) = props.restart_error.as_ref() {
                    p {
                        style: "font-size: 12px; color: var(--canvas-destructive);",
                        "Restart failed: {error}"
                    }
                }
                div {
                    class: "flex justify-end",
                    style: "gap: 8px;",
                    for (index, button) in props.buttons.iter().enumerate() {
                        button {
                            "type": "button",
                            "data-daemon-lost-button": "{index}",
                            class: "flex items-center",
                            style: "gap: 6px; padding: 4px 10px; border-radius: 4px; font-size: 12px; background: {BUTTON_FILL}; color: {button_color(button.tone)};",
                            onmounted: move |event| {
                                mounted_buttons.write().insert(index, event);
                            },
                            span { "{button.label}" }
                            span {
                                class: "font-mono tabular-nums",
                                style: "font-size: 11px;",
                                "{button.keys}"
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
