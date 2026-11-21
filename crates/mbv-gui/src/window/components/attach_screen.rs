//! The Owner attach screen (`replace-tauri-with-blitz` task 4.2, the port of
//! `AttachScreen`): the state the window shows while the core thread's
//! attach campaign has no slice to paint — one variant per `GuiAttachState`,
//! with the text the campaign carries. The mismatch state paints the Restart
//! and Quit buttons and reports their regions through the shared layout
//! reporter — before attach the core thread resolves them itself against the
//! hit map. Every other state answers nothing and paints no regions.
//!
//! The component interprets no key and no pointer event: it paints and
//! reports, and the core resolves a press or chord.

use dioxus_native::prelude::*;

use crate::bridge::frame::GuiAttachState;
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{AttachTargetId, RegionTarget};
use crate::window::icons::{LoaderCircle, RotateCcw, X};
use crate::window::layout::region_set::use_region_set;
use crate::window::sections::Sections;
use crate::window::theme::role_var;

/// The chrome fill named `secondary` (the canvas's pill control fill; the
/// theme's chrome writer in `window/theme.rs` puts it at `--mbv-secondary`).
const CHROME_SECONDARY: &str = "var(--mbv-secondary)";

/// The chrome fill named `border` (`--mbv-border`).
const CHROME_BORDER: &str = "var(--mbv-border)";

/// The attach section's value the screen paints: the state the campaign
/// carries, `None` while the core has sent no batch or once the attach
/// succeeds.
fn waiting(attach: Option<&GuiAttachState>) -> bool {
    matches!(attach, None | Some(GuiAttachState::Waiting))
}

/// The detail text the carried state paints, `None` for the wait states.
fn detail_of(attach: Option<&GuiAttachState>) -> Option<String> {
    match attach {
        Some(
            GuiAttachState::Refusal(detail)
            | GuiAttachState::ExclusiveRefusal(detail)
            | GuiAttachState::Mismatch(detail)
            | GuiAttachState::Failed(detail),
        ) => Some(detail.clone()),
        _ => None,
    }
}

/// The detail text's colour role: the error role for the two refusals, the
/// primary role for the rest — the former client `style:color` conditional.
fn detail_color(attach: Option<&GuiAttachState>) -> String {
    match attach {
        Some(GuiAttachState::Refusal(_) | GuiAttachState::ExclusiveRefusal(_)) => {
            role_var(GuiRole::StatusError)
        }
        _ => role_var(GuiRole::TextPrimary),
    }
}

/// The attach screen (task 4.2). The screen reads the `attach` section
/// itself; each paint that moved a region reports, and the mount report asks
/// the core to emit its frame — the campaign answers a report by re-showing
/// the state with its theme.
#[component]
pub fn AttachScreen() -> Element {
    let sections = consume_context::<Sections>();

    let attach = sections.attach.cloned();

    // Each paint that moved a region reports (design D1): the region set's one
    // effect watches the attach change and the layout epoch — the
    // ResizeObserver the former effect watched — re-measures. The marks run in
    // this render, and only the mismatch state paints the two controls, so
    // every other state reports its empty contribution.
    let watch_attach = sections.attach;
    let region_set = use_region_set(0, move || {
        let _watch_attach = watch_attach.cloned();
    });

    let waiting = waiting(attach.as_ref());
    let detail = detail_of(attach.as_ref()).unwrap_or_default();
    let detail_color = detail_color(attach.as_ref());
    let mismatch = matches!(attach.as_ref(), Some(GuiAttachState::Mismatch(_)));

    rsx! {
        div {
            class: "flex h-full flex-col items-center justify-center",
            style: "gap: var(--mbv-cell-col); padding-inline: calc(var(--mbv-cell-col) * 2);",
            if waiting {
                // The wait before the first frame and the wait for a
                // shutting-down Owner read the same: the core is still
                // working toward the attach.
                LoaderCircle { size: 20, class: "animate-spin", color: role_var(GuiRole::TextMuted) }
                span {
                    class: "text-sm font-medium",
                    style: "color: {role_var(GuiRole::TextSecondary)};",
                    "Attaching to the Owner process…"
                }
            } else {
                span {
                    class: "max-w-prose text-center text-sm leading-relaxed whitespace-pre-line",
                    style: "color: {detail_color};",
                    "{detail}"
                }
                if mismatch {
                    div {
                        class: "flex items-center",
                        style: "gap: calc(var(--mbv-cell-col) / 2);",
                        button {
                            "type": "button",
                            class: "flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium",
                            style: "background: {CHROME_SECONDARY}; border-color: {CHROME_BORDER}; color: {role_var(GuiRole::TextPrimary)};",
                            "data-target": "restart",
                            onmounted: region_set.mark(RegionTarget::Attach(
                                AttachTargetId::Restart,
                            )),
                            RotateCcw { size: 14, color: role_var(GuiRole::Accent) }
                            "Restart"
                            span {
                                class: "font-mono text-xs tabular-nums",
                                style: "color: {role_var(GuiRole::TextMuted)};",
                                "[R]"
                            }
                        }
                        button {
                            "type": "button",
                            class: "flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium",
                            style: "background: {CHROME_SECONDARY}; border-color: {CHROME_BORDER}; color: {role_var(GuiRole::TextPrimary)};",
                            "data-target": "quit",
                            onmounted: region_set.mark(RegionTarget::Attach(AttachTargetId::Quit)),
                            X { size: 14, color: role_var(GuiRole::StatusError) }
                            "Quit"
                            span {
                                class: "font-mono text-xs tabular-nums",
                                style: "color: {role_var(GuiRole::TextMuted)};",
                                "[Q]"
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
