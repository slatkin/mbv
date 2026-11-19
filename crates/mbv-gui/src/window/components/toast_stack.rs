//! The floating toast stack (design D7 of `gui-floating-toasts`): the live
//! toasts the Client core keeps, painted as cards at the window's bottom-right,
//! apart from every status line. The root mounts it once, last, so the cards
//! paint over the panels, the Settings screen, and the modal overlays; it
//! paints nothing while no toast is live.
//!
//! The container sits 52 px above the window's bottom edge (over the 40 px
//! footer band, with a 12 px gap), 16 px in from the right edge, and above the
//! modals' `z-30` stack at `z-index: 40`. Its cards stack upward in arrival
//! order — the section carries them oldest first, so the newest card is at the
//! bottom — and its width is `min(360px, 100% - 32px)` so a narrow pinned panel
//! still fits them.
//!
//! The container and every card take no pointer input: a press or a hover on a
//! card goes to what is under it, and the cards report no layout region. No key
//! acts on a card.
//!
//! A card paints the canvas popover's chrome — the track popover's fill,
//! border, shadow, and radius — with the toast's text beside a 16 px severity
//! icon, in the severity colour [`crate::window::status::toast_color`] names,
//! and a 3 px left border in the same colour.

use dioxus_native::prelude::*;

use crate::bridge::frame::chrome::{GuiToast, GuiToastSeverity};
use crate::window::icons::toast::{CircleCheck, CircleX, Info, TriangleAlert};
use crate::window::sections::Sections;
use crate::window::status::toast_color;

/// The stack's placement: absolute at the bottom-right, 52 px up, 16 px in,
/// over every panel and every modal overlay, in a column that stacks its cards
/// upward. A card never takes pointer input, and the stack is never wider than
/// 360 px or the window minus the two 16 px insets.
const STACK_STYLE: &str = concat!(
    "position: absolute; right: 16px; bottom: 52px; z-index: 40; ",
    "display: flex; flex-direction: column; gap: 8px; align-items: flex-end; ",
    "pointer-events: none; width: min(360px, calc(100% - 32px));",
);

/// The card's chrome and its one row: the popover fill and shadow, a 6 px
/// radius, 10 px by 12 px padding, and 13 px text beside the severity icon.
/// The severity border and colour add to this string per card.
const CARD_STYLE: &str = concat!(
    "display: flex; gap: 8px; pointer-events: none; ",
    "background: var(--canvas-popover); box-shadow: var(--canvas-popover-shadow); ",
    "border: 1px solid var(--canvas-popover-border); border-radius: 6px; ",
    "padding: 10px 12px; font-size: 13px; color: var(--canvas-text);",
);

/// The name a card carries in `data-toast-severity`, so a test reads the
/// severity it paints.
const fn severity_name(severity: GuiToastSeverity) -> &'static str {
    match severity {
        GuiToastSeverity::Neutral => "neutral",
        GuiToastSeverity::Success => "success",
        GuiToastSeverity::Warning => "warning",
        GuiToastSeverity::Error => "error",
    }
}

/// The severity's icon (design D7): `Info` for a Neutral toast, `CircleCheck`
/// for a Success one, `TriangleAlert` for a Warning one, and `CircleX` for an
/// Error one, at 16 px in the severity colour.
fn severity_icon(severity: GuiToastSeverity) -> Element {
    let color = toast_color(severity).to_owned();
    match severity {
        GuiToastSeverity::Neutral => rsx! {
            Info { size: 16, color: color }
        },
        GuiToastSeverity::Success => rsx! {
            CircleCheck { size: 16, color: color }
        },
        GuiToastSeverity::Warning => rsx! {
            TriangleAlert { size: 16, color: color }
        },
        GuiToastSeverity::Error => rsx! {
            CircleX { size: 16, color: color }
        },
    }
}

/// One card's style: the shared chrome and the 3 px severity border on the
/// left edge. The text keeps the canvas's ink; only the icon and the border
/// take the severity colour.
fn card_style(severity: GuiToastSeverity) -> String {
    format!(
        "{CARD_STYLE} border-left: 3px solid {};",
        toast_color(severity)
    )
}

/// The floating toast stack (design D7). It reads the `toasts` section itself
/// and paints one card per live toast, oldest first; with no live toast it
/// paints nothing.
#[component]
pub fn ToastStack() -> Element {
    let sections = consume_context::<Sections>();
    let toasts = sections.toasts.cloned();
    if toasts.0.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            style: "{STACK_STYLE}",
            "data-toast-stack": "",
            for toast in toasts.0.iter() {
                ToastCard { key: "{toast.id.0}", toast: toast.clone() }
            }
        }
    }
}

/// One card of the stack: the severity icon beside the toast's text, in the
/// severity colour, keyed on the toast's id and marked `data-toast-card` with
/// its severity in `data-toast-severity`.
#[component]
fn ToastCard(toast: GuiToast) -> Element {
    rsx! {
        div {
            style: "{card_style(toast.severity)}",
            "data-toast-card": "",
            "data-toast-severity": "{severity_name(toast.severity)}",
            {severity_icon(toast.severity)}
            span {
                style: "min-width: 0; overflow-wrap: break-word;",
                "{toast.text}"
            }
        }
    }
}

#[cfg(test)]
mod tests;
