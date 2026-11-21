//! The shared confirmation modal (`replace-tauri-with-blitz` task 4.2, the
//! port of `ConfirmOverlay`): the message and the button row the
//! frame's `confirm` overlay carries, painted window-modal over every panel in
//! the [dialog frame](super::dialog_frame)'s chrome.
//! The core holds the action; Enter and Esc answer through the core's
//! overlay key rules, and a press on a button forwards like every other
//! pointer event — the core resolves it against the hit map and executes the
//! button's key hint.
//!
//! The buttons report their `confirm` regions under the frame's own report
//! key, so the core's hit map names them while the modal paints; the
//! contribution retracts on unmount.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiConfirmButton;
use crate::bridge::layout::{ConfirmTargetId, RegionTarget};
use crate::window::components::dialog_frame::{DialogFrame, DialogSizes};

/// The modal's props: the message and the button row the `confirm` overlay
/// carries.
#[derive(Props, Clone, PartialEq)]
pub struct ConfirmOverlayProps {
    /// The modal's message.
    pub message: String,
    /// The button row, in the core's order.
    pub buttons: Vec<GuiConfirmButton>,
}

impl std::fmt::Debug for ConfirmOverlayProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmOverlayProps")
            .field("message", &self.message)
            .field("buttons", &self.buttons)
            .finish()
    }
}

/// The modal (task 4.2): its message and its sizes in the shared frame. The
/// core raised it; the frame paints and reports, and the core resolves a press
/// or chord.
#[component]
pub fn ConfirmOverlay(props: ConfirmOverlayProps) -> Element {
    let targets: Vec<RegionTarget> = (0..props.buttons.len())
        .map(|index| RegionTarget::Confirm(ConfirmTargetId::Button(index)))
        .collect();

    rsx! {
        DialogFrame {
            sizes: DialogSizes {
                padding: 19,
                gap: 14,
                max_width: 398,
                button_text: 14,
            },
            buttons: props.buttons.clone(),
            targets,
            marker: "data-confirm-overlay",
            button_marker: "data-confirm-button",
            label: "Confirm",
            p {
                style: "font-size: 14px; color: var(--canvas-heading);",
                "{props.message}"
            }
        }
    }
}

#[cfg(test)]
mod tests;
