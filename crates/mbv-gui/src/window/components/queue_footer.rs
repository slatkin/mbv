//! The Queue footer (task 5.6, the port of `QueueFooter`): one 40 px
//! band at the queue column's bottom, with the canvas's top border and side
//! padding, that merges what the TUI paints as the queue footer's status
//! pills and the Status bar's prefix and toast rows — the queue source, the
//! dirty mark, the prefix-armed state, the toasts, and the shown queue's
//! totals.
//!
//! The left group is the queue source and its state: the autosave indicator
//! (while save-on-consume is on for a saved playlist), then the playlist
//! text, then the dirty mark (while the queue differs from its saved
//! playlist), which sits beside the source it describes. The canvas paints
//! the source as plain text with no leading icon, and the text reports no hit
//! region — the GUI presents no playlists surface, so a press on it would
//! have nowhere to go, and a reported target that can never fire is a lie in
//! the layout report. (The TUI's pill opens its Playlists overlay.)
//!
//! The centre carries the armed prefix badge and the live toast. The badge is
//! the same `PREFIX` badge the TUI's status row paints, and it drops while a
//! toast is live — the TUI's pill-drop rule carried into the line. The toast
//! paints inline in it, centred in its severity colour: the spec permits the
//! footer to show toasts in the line or in a conventional GUI form. A live
//! toast moves no layout: the line is always painted and the toast only fills
//! its centre span. The centre is empty while the projection holds none of
//! those states, as the canvas draws it.
//!
//! The right group is the queue's times (gui-named-queues task 8.5, design
//! D9): the marked rows' total beside the shown queue's total, so a reader
//! compares what a marked selection would play against the whole queue. The
//! marked total paints only while rows are marked — `Sel` names it — and a sum
//! over an unknown runtime carries a `+` as a lower bound. The filter narrows
//! neither sum. The footer carries no Player-owner label and no liveness dot:
//! the shown queue's owner reads from the playback target in the panel's
//! header.
//!
//! The armed prefix and the live toast are each one fact, and each paints
//! once a frame (design D10): while the Library panel shows, its Library
//! Status bar replaces its hints with them, so this line paints neither and
//! the band under the Library column owns them. While the Library panel
//! does not show, this line paints them as the TUI's status row does.
//!
//! The band paints the canvas's type and border (mono 12 px, a 1 px top
//! border in the queue-local `--canvas-border` token) over the queue column's
//! backdrop: the `queueFooterBand` name it carries is not part of the
//! window's `--mbv-*` token set, so the band paints no fill of its own
//! (`window/theme.rs::write_chrome` holds the rule and the reason), as the
//! reference build painted it. The text keeps the secondary-title token
//! (`--canvas-type-secondary`, the queue source is a secondary title) and the
//! canvas's time token for the totals (design D14: the colour carries the data
//! type). The slice's `GuiStatusBar` carries no volume,
//! mute, or visual-mode content, so the line paints no interactive status pill
//! and reports no region at all — a point on the line resolves to nothing. It
//! still reports through the shared layout reporter so the merged report
//! accounts for the mounted surface.

use dioxus_native::prelude::*;

use crate::window::icons::{SaveCheck, SaveOff};
use crate::window::layout::{LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter};
use crate::window::pointer_target::PointerZone;
use crate::window::sections::Sections;
use crate::window::status::toast_color;
use crate::window::theme::role_var;
use crate::window::time::format_duration_short;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the Queue footer (the `QueueFooter` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct QueueFooterProps {
    /// Whether the Library panel shows beside this one: the Library Status
    /// bar owns the armed prefix and live toast then, so the footer line
    /// paints neither.
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
/// the line's contribution carries no region, so the reporter's dedupe holds
/// its one report for the surface's life.
#[component]
pub fn QueueFooter(props: QueueFooterProps) -> Element {
    let sections = consume_context::<Sections>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);

    // No region outlives the band: the contribution retracts on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    let watch_footer = sections.footer;
    let watch_summary = sections.summary;
    let watch_status = sections.status;
    let watch_epoch = epoch.0;
    use_effect(move || {
        let _footer = watch_footer.cloned();
        let _summary = watch_summary.cloned();
        let _status = watch_status.cloned();
        let _epoch = watch_epoch.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            // The painted regions this frame reports: none. The line's pills
            // are informational, so a point on it resolves to no target; the
            // report still accounts for the mounted surface.
            effect_reporter.report_layout(key, LayoutContribution::default());
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
    });

    let footer = sections.footer.cloned();
    let summary = sections.summary.cloned();
    let status = sections.status.cloned();
    // The toast this line paints: the section's live toast, or `None` while
    // the Library panel shows and its Status bar paints it (design D10).
    let toast = if props.library_visible {
        None
    } else {
        status.toast.clone()
    };
    // Whether this line paints the armed prefix badge: prefix mode is armed,
    // no toast is live (the line's pill-drop rule), and the Library panel
    // does not show (design D10).
    let prefix = !props.library_visible && status.prefix_armed && toast.is_none();
    // The queue's total, then the marked rows' total while rows are marked:
    // the marked one first, so a reader meets the selection beside the whole
    // queue (design D9).
    let total = footer_time(summary.total_secs, summary.total_partial);
    let marked = summary
        .marked_secs
        .map(|secs| footer_time(secs, summary.marked_partial));

    rsx! {
        // The canvas footer band: 40 px, 16 px side padding, the canvas's
        // 1 px top border, and the canvas's mono type. The left group is the
        // queue's source with its autosave and dirty states, the centre the
        // prefix badge and the toast, and the right group the queue's totals.
        div {
            class: "flex items-center overflow-hidden font-mono whitespace-nowrap",
            style: "height: 40px; padding-inline: 16px; font-size: 12px; border-top: 1px solid var(--canvas-border); background: var(--mbv-queueFooterBand);",
            "data-footer-line": "",
            // The Queue zone (`gui-pointer-target-interaction` D1) reaches the
            // footer band: it sits under the Queue list box, outside the box's
            // own element, so it carries the marker too.
            "data-pointer-zone": PointerZone::Queue.token(),
            // The left group: the autosave indicator, the queue's source text
            // — no leading icon, as the canvas paints it — then the dirty mark
            // beside the source it describes.
            div {
                class: "flex min-w-0 items-center",
                style: "gap: 12px;",
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
                    // The dirty mark sits beside the source: the icon then its
                    // word, in the pill's role.
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
            // The centre: the armed prefix badge (dropped while a toast is
            // live, the TUI's pill-drop rule), then the toast inline in the
            // line in its severity colour (the footer-line toast form the spec
            // permits); empty while neither is live.
            div {
                class: "flex min-w-0 flex-1 items-center justify-center",
                style: "gap: 12px; padding-inline: 16px;",
                if prefix {
                    span {
                        class: "shrink-0 font-bold",
                        style: "color: var(--mbv-accent);",
                        "PREFIX"
                    }
                }
                {
                    let toast_text = toast
                        .clone()
                        .map_or(String::new(), |toast| toast.text);
                    let toast_css = toast
                        .as_ref()
                        .map_or(String::new(), |toast| {
                            format!("color: {};", toast_color(toast.severity))
                        });
                    rsx! {
                        Truncate {
                            text: toast_text,
                            class: "min-w-0 text-center".to_owned(),
                            css: toast_css,
                            style: TextStyle::mono(12.0, 400.0),
                        }
                    }
                }
            }
            // The right group: the marked rows' total while rows are marked,
            // then the shown queue's total, flush at the row's right edge. Both
            // are times, so they paint the canvas's time token (design D14: the
            // colour carries the data type); `Sel` names the marked one.
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

#[cfg(test)]
mod tests;
