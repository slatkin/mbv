//! The Queue footer (task 5.6, the port of `QueueFooter`): one 40 px
//! band at the queue column's bottom, with the canvas's top border and side
//! padding, that merges what the TUI paints as the queue footer's status
//! pills and the Status bar's prefix row — the playback target and the
//! collapse control, the queue source, the dirty mark, the prefix-armed
//! state, and the shown queue's totals.
//!
//! The left group leads with the collapse control that the player panel's
//! header used to paint (`move-playback-header-into-queue-footer` D1, D2),
//! whose press runs the same toggle as the `z` chord. It compacts to the
//! band's 28 px control height, keeps its aria-label and its
//! `Collapse panel (z)` title, and paints a muted `ChevronsRight` glyph — the
//! inverse of the collapsed rail's `ChevronsLeft` expand control (user request
//! 2026-11-20). The playback target control moved to the Queue header
//! (`queue_target_control.rs`, user request 2026-11-20). Then the queue source
//! and its state follow: the autosave indicator (while save-on-consume is on
//! for a saved playlist), the playlist text, and the dirty mark (while the
//! queue differs from its saved playlist), which sits beside the source it
//! describes. The canvas paints the source as plain text with no leading icon,
//! and the text reports no hit region — the GUI presents no playlists surface,
//! so a press on it would have nowhere to go, and a reported target that can
//! never fire is a lie in the layout report. (The TUI's pill opens its
//! Playlists overlay.)
//!
//! The centre carries the armed prefix badge and nothing else. The badge is
//! the same `PREFIX` badge the TUI's status row paints, and it paints for as
//! long as prefix mode stays armed: the line shows no toast, so no arrival
//! drops it. The centre is empty while prefix mode is not armed, as the canvas
//! draws it.
//!
//! The right group is the queue's times (gui-named-queues task 8.5, design
//! D9): the marked rows' total beside the shown queue's total, so a reader
//! compares what a marked selection would play against the whole queue. The
//! marked total paints only while rows are marked — `Sel` names it — and a sum
//! over an unknown runtime carries a `+` as a lower bound. The filter narrows
//! neither sum. The footer carries no Player-owner label and no liveness dot:
//! the shown queue's owner reads from the playback target in the Queue header.
//!
//! The armed prefix is one fact, and it paints once a frame (design D10):
//! while the Library panel shows, its Library Status bar paints it, so this
//! line paints none and the band under the Library column owns it. While the
//! Library panel does not show, this line paints it as the TUI's status row
//! does. Toasts show as floating cards over the window, never in this line.
//!
//! The band paints the canvas's type over the queue column's floor (mono
//! 12 px, the 1 px `--ef-bg3` top border and the `--ef-bg0` fill design D2 and
//! D1 name of the redesign), so the footer reads as the column's own band under
//! the list. The text keeps the secondary-title token
//! (`--canvas-type-secondary`, the queue source is a secondary title) and the
//! canvas's time token for the totals (design D14: the colour carries the data
//! type). The slice's `GuiStatusBar` carries no volume,
//! mute, or visual-mode content, so the line paints no interactive status pill.
//! It reports the collapse control's region through the shared layout reporter,
//! so the merged report accounts for the mounted surface.

use dioxus_native::prelude::*;

use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::components::prefix_badge::PrefixBadge;
use crate::window::icons::{ChevronsRight, SaveCheck, SaveOff};
use crate::window::layout::region_set::use_region_set;
use crate::window::pointer_target::PointerZone;
use crate::window::sections::Sections;
use crate::window::theme::{EfToken, ef_var, role_var};
use crate::window::time::format_duration_short;

/// The props of the Queue footer (the `QueueFooter` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueFooterProps {
    /// Whether the Library panel shows beside this one: the Library Status
    /// bar owns the armed prefix then, so the footer line does not paint it.
    #[props(default)]
    pub library_visible: bool,
}

/// A footer time: the queue's shared `H:MM:SS` duration formatter, marked as a
/// lower bound with a `+` while a slot's runtime is unknown, so a reader knows
/// the sum is not the whole story (design D9).
fn footer_time(secs: i64, partial: bool) -> String {
    let text = format_duration_short(secs.max(0));
    if partial {
        return format!("{text}+");
    }
    text
}

/// The Queue footer: the canvas footer band, painted from the `footer`,
/// `status`, and `summary` sections. Every paint that moved a region reports —
/// the band's contribution carries the collapse control's region.
#[component]
pub fn QueueFooter(props: QueueFooterProps) -> Element {
    let sections = consume_context::<Sections>();

    // Each paint that moved a region reports (design D1): the region set's one
    // effect watches the footer, the summary, the Status bar, and the layout
    // epoch. The band's one mark is the collapse control, painted in this
    // render.
    let watch_footer = sections.footer;
    let watch_summary = sections.summary;
    let watch_status = sections.status;
    let region_set = use_region_set(0, move || {
        let _footer = watch_footer.cloned();
        let _summary = watch_summary.cloned();
        let _status = watch_status.cloned();
    });

    let footer = sections.footer.cloned();
    let summary = sections.summary.cloned();
    let status = sections.status.cloned();
    // Whether this line paints the armed prefix badge: prefix mode is armed,
    // and the Library panel does not show (design D10).
    let prefix = !props.library_visible && status.prefix_armed;
    // The queue's total, then the marked rows' total while rows are marked:
    // the marked one first, so a reader meets the selection beside the whole
    // queue (design D9).
    let total = footer_time(summary.total_secs, summary.total_partial);
    let marked = summary
        .marked_secs
        .map(|secs| footer_time(secs, summary.marked_partial));

    rsx! {
        // The canvas footer band: 40 px, 16 px side padding, the canvas's
        // 1 px top border, and the canvas's mono type. The left group leads
        // with the playback target and the collapse control, then the queue's
        // source with its autosave and dirty states; the centre carries the
        // prefix badge, and the right group the queue's totals.
        // The open chooser's popover is the band's child, anchored above it.
        div {
            class: "relative flex items-center font-mono whitespace-nowrap",
            style: format!(
                "height: 40px; padding-inline: 16px; font-size: 12px; border-top: 1px solid {}; background: {};",
                ef_var(EfToken::Bg3),
                ef_var(EfToken::Bg0)
            ),
            "data-footer-line": "",
            // The Queue zone (`gui-pointer-target-interaction` D1) reaches the
            // footer band: it sits under the Queue list box, outside the box's
            // own element, so it carries the marker too — and the popover,
            // which opens above it, stays in the zone with it.
            "data-pointer-zone": PointerZone::Queue.token(),
            // The band's content: the left, centre, and right groups, clipped
            // to the band. The popover is the band's own child, outside this
            // clipped box, so the clip never cuts it.
            div {
                class: "flex h-full w-full min-w-0 items-center overflow-hidden",
                // The left group: the playback target, the collapse control,
                // then the autosave indicator, the queue's source text — no
                // leading icon, as the canvas paints it — and the dirty mark
                // beside the source it describes.
                div {
                    class: "flex min-w-0 items-center",
                    style: "gap: 12px;",
                    button {
                        "type": "button",
                        class: "flex shrink-0 items-center justify-center",
                        style: "width: 28px; height: 28px;",
                        "data-target": "collapse",
                        "aria-label": "Collapse panel",
                        title: "Collapse panel (z)",
                        onmounted: region_set.mark(RegionTarget::QueuePlayback(
                            QueuePlaybackTargetId::Collapse,
                        )),
                        ChevronsRight { size: 18, stroke_width: 1.8, color: role_var(GuiRole::TextMuted) }
                    }
                    if let Some(autosave) = footer.autosave.as_ref() {
                        SaveCheck {
                            size: 14,
                            color: role_var(autosave.role),
                            class: "shrink-0".to_owned(),
                        }
                    }
                    if !footer.playlist.text.is_empty() {
                        // The queue's source is a secondary title, so it paints
                        // the secondary token (design D14); the window paints
                        // no role here.
                        span {
                            class: "truncate",
                            style: "color: var(--canvas-type-secondary);",
                            "data-footer-source": "",
                            "{footer.playlist.text}"
                        }
                    }
                    if let Some(dirty) = footer.dirty.as_ref() {
                        // The dirty mark sits beside the source: the icon then
                        // its word, in the pill's role.
                        span {
                            class: "flex shrink-0 items-center",
                            style: "gap: 8px;",
                            "data-footer-dirty": "",
                            SaveOff {
                                size: 14,
                                color: role_var(dirty.role),
                                class: "shrink-0".to_owned(),
                            }
                            span {
                                class: dirty.bold.then(|| "font-bold".to_owned()),
                                style: "color: {role_var(dirty.role)};",
                                "{dirty.text}"
                            }
                        }
                    }
                }
                // The centre: the armed prefix badge and nothing else; empty
                // while prefix mode is not armed. Toasts show as floating
                // cards, never in this line.
                div {
                    class: "flex min-w-0 flex-1 items-center justify-center",
                    style: "gap: 12px; padding-inline: 16px;",
                    if prefix {
                        PrefixBadge { class: "shrink-0 font-bold" }
                    }
                }
                // The right group: the marked rows' total while rows are
                // marked, then the shown queue's total, flush at the row's
                // right edge. Both are times, so they paint the canvas's time
                // token (design D14: the colour carries the data type); `Sel`
                // names the marked one.
                div {
                    class: "flex shrink-0 items-center",
                    style: "gap: 16px;",
                    if let Some(marked) = marked {
                        span {
                            class: "flex items-center",
                            style: "gap: 6px;",
                            "data-footer-marked": "",
                            span {
                                style: "color: var(--canvas-type-secondary);",
                                "data-footer-marked-label": "",
                                "Sel"
                            }
                            span {
                                style: "color: var(--canvas-type-time);",
                                "data-footer-marked-value": "",
                                "{marked}"
                            }
                        }
                    }
                    span {
                        style: "color: var(--canvas-type-time);",
                        "data-footer-total": "",
                        "{total}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
