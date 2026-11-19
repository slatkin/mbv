//! The shared confirmation modal (`replace-tauri-with-blitz` task 4.2, the
//! port of `ConfirmOverlay`): the message and the button row the
//! frame's `confirm` overlay carries, painted window-modal over every panel.
//! The core holds the action; Enter and Esc answer through the core's
//! overlay key rules, and a press on a button forwards like every other
//! pointer event — the core resolves it against the hit map and executes the
//! button's key hint.
//!
//! The buttons report their `confirm` regions under this component's own
//! report key, so the core's hit map names them while the modal paints; the
//! contribution retracts on unmount.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::{GuiConfirmButton, GuiConfirmButtonTone};
use crate::bridge::layout::{ConfirmTargetId, RegionTarget};
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};

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

/// The modal and dialog button fill: the canvas.s recessed dark fill for every tone.
pub(super) const BUTTON_FILL: &str = "var(--canvas-well)";

/// The modal's button text colour: the tone's token — the affirmative pill
/// reads as accent text on the dim surface, never as the picked row's fill.
pub(super) fn button_color(tone: GuiConfirmButtonTone) -> &'static str {
    match tone {
        GuiConfirmButtonTone::Affirmative => "var(--canvas-accent)",
        GuiConfirmButtonTone::Cancel => "var(--canvas-destructive)",
    }
}

/// The modal (task 4.2). The core raised it; the component paints and
/// reports, and the core resolves a press or chord.
#[component]
pub fn ConfirmOverlay(props: ConfirmOverlayProps) -> Element {
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);
    let mut mounted_buttons: Signal<std::collections::HashMap<usize, Event<MountedData>>> =
        use_signal(std::collections::HashMap::new);

    // No region outlives the modal: the contribution retracts on unmount.
    let mut drop_reporter = reporter.clone();
    use_drop(move || drop_reporter.retract_layout(key));

    // Each paint that moved a region reports (design D4): the frame's fields,
    // the buttons' mounts, and every layout fact (the epoch) re-measure. A
    // resize moves the modal's buttons, so the epoch subscription keeps their
    // regions matched to the layout that painted them.
    let epoch = try_consume_context::<LayoutEpoch>();
    use_effect(use_reactive(
        (&props.message.clone(), &props.buttons.clone()),
        move |(_message, buttons)| {
            let mut mounted_buttons = mounted_buttons;
            let mut effect_reporter = reporter.clone();
            let _epoch = epoch.as_ref().map(|epoch| epoch.0.cloned());
            spawn(async move {
                // A button the current frame does not paint owns no region.
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
                                RegionTarget::Confirm(ConfirmTargetId::Button(index)),
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
            "data-confirm-overlay": "true",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "Confirm",
            div {
                class: "flex flex-col",
                style: format!(
                    "{MENU_CARD_STYLE} padding: 21px; gap: 16px; max-width: 442px;"
                ),
                p {
                    style: "font-size: 17px; color: var(--canvas-heading);",
                    "{props.message}"
                }
                div {
                    class: "flex justify-end",
                    style: "gap: 10px;",
                    for (index, button) in props.buttons.iter().enumerate() {
                        button {
                            "type": "button",
                            "data-confirm-button": "{index}",
                            class: "flex items-center",
                            style: "gap: 8px; padding: 5px 13px; border-radius: 5px; font-size: 16px; background: {BUTTON_FILL}; color: {button_color(button.tone)};",
                            onmounted: move |event| {
                                mounted_buttons.write().insert(index, event);
                            },
                            span { "{button.label}" }
                            span {
                                class: "font-mono tabular-nums",
                                style: "font-size: 14px;",
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
