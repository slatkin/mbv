//! The daemon-lost recovery dialog the Client core raised
//! (`replace-tauri-with-blitz` task 4.2, the port of
//! `DaemonLostOverlay`; `App::raise_daemon_lost_modal`,
//! `daemon-disconnect-handling`): the diagnostics and the recovery choices
//! the frame's `daemonLost` overlay carries, painted window-modal over every
//! panel in the [dialog frame](super::dialog_frame)'s chrome.
//!
//! The dialog blocks every other input until one of its choices runs: the
//! core's overlay key rules bind its three chords (and nothing else, Esc
//! included), and a press on a button forwards like every other pointer
//! event — the core resolves it against the hit map and answers with the
//! choice the index names.
//!
//! The buttons report their `daemonLost` regions under the frame's own report
//! key, so the core's hit map names them while the dialog paints; the
//! contribution retracts on unmount.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiConfirmButton;
use crate::bridge::layout::{DaemonLostTargetId, RegionTarget};
use crate::window::components::dialog_frame::{DialogFrame, DialogSizes};

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

/// The dialog (task 4.2): its diagnostics and its sizes in the shared frame.
/// The core raised it; the frame paints and reports, and the core resolves a
/// press or chord.
#[component]
pub fn DaemonLostOverlay(props: DaemonLostOverlayProps) -> Element {
    let targets: Vec<RegionTarget> = (0..props.buttons.len())
        .map(|index| RegionTarget::DaemonLost(DaemonLostTargetId::Button(index)))
        .collect();

    rsx! {
        DialogFrame {
            sizes: DialogSizes {
                padding: 16,
                gap: 12,
                max_width: 420,
                button_text: 12,
            },
            buttons: props.buttons.clone(),
            targets,
            marker: "data-daemon-lost-overlay",
            button_marker: "data-daemon-lost-button",
            label: "The local daemon connection was lost",
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
        }
    }
}

#[cfg(test)]
mod tests;
