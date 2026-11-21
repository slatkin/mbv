//! The one dialog frame the window's two modals share
//! (`gui-tech-debt-simplification` design D3): the window-modal scrim, the card
//! on [`MENU_CARD_STYLE`], the body the dialog supplies, and the button row
//! whose choices report the dialog's own regions.
//!
//! The confirmation modal and the daemon-lost recovery dialog painted the same
//! scrim, card, button row, button fill, mounted-handle map, and report, and
//! differed only in their body and in four sizes. The frame owns the chrome; a
//! dialog passes its sizes, its choices, the region target of each choice, the
//! DOM hooks and the label that name it, and its body as children.
//!
//! The choices report under the frame's own region set (design D1): the set's
//! one effect re-runs on the choices and their targets and on the layout epoch,
//! as the dialogs' former `use_reactive` effects did, one mark per painted
//! choice in the core's order, and the set retracts the key on unmount. A press
//! on a choice forwards `pointer` like every other pointer event — the core
//! resolves it against the hit map.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::{GuiConfirmButton, GuiConfirmButtonTone};
use crate::bridge::layout::RegionTarget;
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::layout::region_set::{MarkHandler, use_region_set_for};

/// The frame's props: the four sizes that make the dialog's look, the choices
/// the core projected, each choice's region target, the identity the dialog
/// paints, and the body.
#[derive(Props, Clone, PartialEq)]
pub struct DialogFrameProps {
    /// The dialog's sizes: padding, gap, and maximum width of the card, and
    /// the choices' text size.
    pub sizes: DialogSizes,
    /// The choices, in the core's order.
    pub buttons: Vec<GuiConfirmButton>,
    /// The region target of the choice at each index, in the core's order.
    #[props(default)]
    pub targets: Vec<RegionTarget>,
    /// The scrim's `data-*` hook, as in `data-confirm-overlay`.
    pub marker: &'static str,
    /// Each choice's `data-*` hook, as in `data-confirm-button`.
    pub button_marker: &'static str,
    /// The dialog's accessible name.
    pub label: &'static str,
    /// The dialog's body, painted above the choices.
    pub children: Element,
}

impl std::fmt::Debug for DialogFrameProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogFrameProps")
            .field("sizes", &self.sizes)
            .field("buttons", &self.buttons)
            .field("targets", &self.targets)
            .field("marker", &self.marker)
            .field("button_marker", &self.button_marker)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// One dialog's sizes, in CSS pixels: the card's padding, gap, and maximum
/// width, and the choices' text size. The confirmation modal paints
/// 19/14/398/14 and the daemon-lost dialog 16/12/420/12.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogSizes {
    /// The card's padding.
    pub padding: u32,
    /// The gap between the card's body and its button row.
    pub gap: u32,
    /// The card's maximum width.
    pub max_width: u32,
    /// The choices' text size.
    pub button_text: u32,
}

/// The button row's chrome at one dialog's choice text size. Each value scales
/// with the text, so the smaller dialog's choices shrink with their labels: at
/// 14 px the row paints a 9 px gap, 7 px choice gap, 12 px side padding, 5 px
/// radius, and 12 px key text; at 12 px it paints 8, 6, 10, 4, and 11.
#[derive(Clone, Copy)]
struct ButtonChrome {
    /// The gap between the choices.
    row_gap: u32,
    /// The gap between a choice's label and its keys.
    gap: u32,
    /// A choice's side padding (its vertical padding is 4 px in both dialogs).
    padding: u32,
    /// A choice's corner radius.
    radius: u32,
    /// A choice's key text size.
    keys_text: u32,
}

impl DialogSizes {
    /// The button row's chrome at this dialog's choice text size.
    fn button_chrome(self) -> ButtonChrome {
        let text = self.button_text;
        ButtonChrome {
            row_gap: text / 2 + 2,
            gap: text / 2,
            padding: text.saturating_sub(2),
            radius: (text / 2).saturating_sub(2),
            keys_text: text / 2 + 5,
        }
    }
}

/// The dialog button fill: the canvas's recessed dark fill for every tone.
pub(super) const BUTTON_FILL: &str = "var(--canvas-well)";

/// A choice's text colour: the tone's token — the affirmative pill reads as
/// accent text on the dim surface, never as the picked row's fill.
pub(super) fn button_color(tone: GuiConfirmButtonTone) -> &'static str {
    match tone {
        GuiConfirmButtonTone::Affirmative => "var(--canvas-accent)",
        GuiConfirmButtonTone::Cancel => "var(--canvas-destructive)",
    }
}

/// The dialog frame. The core raised the dialog; the frame paints and reports,
/// and the core resolves a press or chord.
#[component]
pub fn DialogFrame(props: DialogFrameProps) -> Element {
    // Each paint that moved a region reports (design D1): the choices and their
    // targets re-run the set's one effect, and the layout epoch through the
    // set's own read re-reports when a body change moves them.
    let region_set = use_region_set_for(0, (&props.buttons.clone(), &props.targets.clone()));
    // The marks, in the report's order (design D1): one per painted choice,
    // under the target the dialog named for its index. A choice the frame
    // paints without a target owns no region.
    let marks: Vec<Option<MarkHandler>> = (0..props.buttons.len())
        .map(|index| {
            props
                .targets
                .get(index)
                .map(|target| region_set.mark(*target))
        })
        .collect();
    let chrome = props.sizes.button_chrome();
    let scrim_attrs = vec![Attribute::new(props.marker, "true", None, false)];
    let card_style = format!(
        "{MENU_CARD_STYLE} padding: {}px; gap: {}px; max-width: {}px;",
        props.sizes.padding, props.sizes.gap, props.sizes.max_width
    );
    let row_style = format!("gap: {}px;", chrome.row_gap);
    let button_style = |tone: GuiConfirmButtonTone| {
        format!(
            "gap: {}px; padding: 4px {}px; border-radius: {}px; font-size: {}px; background: {BUTTON_FILL}; color: {};",
            chrome.gap,
            chrome.padding,
            chrome.radius,
            props.sizes.button_text,
            button_color(tone)
        )
    };
    let keys_style = format!("font-size: {}px;", chrome.keys_text);

    rsx! {
        div {
            class: "absolute inset-0 z-30 flex items-center justify-center",
            style: "background: var(--canvas-modal-scrim);",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "{props.label}",
            ..scrim_attrs,
            div {
                class: "flex flex-col",
                style: card_style,
                {props.children}
                div {
                    class: "flex justify-end",
                    style: row_style,
                    for (index, button) in props.buttons.iter().enumerate() {
                        {
                            let attrs = vec![Attribute::new(
                                props.button_marker,
                                index.to_string(),
                                None,
                                false,
                            )];
                            let mark = marks.get(index).copied().flatten();
                            rsx! {
                                button {
                                    "type": "button",
                                    class: "flex items-center",
                                    style: button_style(button.tone),
                                    onmounted: move |event: Event<MountedData>| {
                                        if let Some(mark) = mark {
                                            mark.call(event);
                                        }
                                    },
                                    ..attrs,
                                    span { "{button.label}" }
                                    span {
                                        class: "font-mono tabular-nums",
                                        style: keys_style.clone(),
                                        "{button.keys}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
