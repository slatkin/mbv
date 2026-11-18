//! The Queue footer (task 5.6, the port of `QueueFooter`): one 40 px
//! band at the queue column's bottom, with the canvas's top border and side
//! padding, that merges what the TUI paints as the queue footer's status
//! pills and the Status bar's prefix and toast rows — the playback target and
//! the collapse control, the queue source, the dirty mark, the prefix-armed
//! state, the toasts, and the shown queue's totals.
//!
//! The left group leads with the two queue-wide controls the player panel's
//! header used to paint (`move-playback-header-into-queue-footer` D1, D2): the
//! playback target, whose press opens the core's chooser, then the collapse
//! control, whose press runs the same toggle as the `z` chord. Both compact to
//! the band's 28 px control height; the target keeps its Cast glyph, host
//! label, and chevron, and the collapse control keeps its aria-label, its
//! `Collapse panel (z)` title, and its muted `Maximize2` glyph. Then the queue
//! source and its state follow: the autosave indicator (while save-on-consume
//! is on for a saved playlist), the playlist text, and the dirty mark (while
//! the queue differs from its saved playlist), which sits beside the source it
//! describes. The canvas paints the source as plain text with no leading icon,
//! and the text reports no hit region — the GUI presents no playlists surface,
//! so a press on it would have nowhere to go, and a reported target that can
//! never fire is a lie in the layout report. (The TUI's pill opens its
//! Playlists overlay.)
//!
//! The chooser's popover is this band's, anchored above it (`bottom-full`,
//! D3): the band sits at the window's bottom edge, so the popover opens upward
//! from the pressed control instead of down. Each row reports its
//! `targetChoice` index — the core resolves a row press against the same live
//! choice list the frame carries — under a second report key the band owns,
//! first inserted when the chooser opens: the band mounts after the queue
//! panel, so its own key lands after the queue rows in the merged report, and
//! the chooser's key lands after the band's — the hit map's last push wins
//! where a row paints over a queue row beneath it. The window holds no open,
//! cursor, or hover state; Esc and a press elsewhere close the popover through
//! the core's key and pointer paths.
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
//! the shown queue's owner reads from the playback target at its left.
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
//! mute, or visual-mode content, so the line paints no interactive status pill.
//! It reports the playback target's and the collapse control's regions, and the
//! open chooser's rows under its own second key, through the shared layout
//! reporter, so the merged report accounts for the mounted surface.

use dioxus_native::prelude::*;

use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{GuiTargetChoice, GuiTargetKey};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{LayoutRegion, QueuePlaybackTargetId, RegionTarget};
use crate::window::brand_icons::{TargetBrand, TargetBrandIcon};
use crate::window::components::row_handle::record_row_handle;
use crate::window::icons::{Cast, Check, ChevronDown, Maximize2, Monitor, SaveCheck, SaveOff};
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};
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
/// `status`, `summary`, `playback`, and `overlay` sections. Every paint that
/// moved a region reports — the band's contribution carries the playback
/// target's and the collapse control's regions, and the open chooser's rows
/// report under the band's second key.
#[component]
pub fn QueueFooter(props: QueueFooterProps) -> Element {
    let sections = consume_context::<Sections>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);
    let chooser_key = use_hook(LayoutKey::next);

    let mut target_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut collapse_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // The open chooser's painted rows, by their choice index (the former client
    // `querySelectorAll('[data-choice]')`).
    let chooser_rows: Signal<Vec<Option<Event<MountedData>>>> = use_signal(Vec::new);

    // No region outlives the band: the contributions retract on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    // The playback target chooser: the overlay section's `targetChooser`
    // variant, `None` while another overlay (or none) is up.
    let chooser = match sections.overlay.cloned() {
        Some(GuiOverlayFrame::TargetChooser { cursor, choices }) => Some((cursor, choices)),
        _ => None,
    };

    let watch_footer = sections.footer;
    let watch_summary = sections.summary;
    let watch_status = sections.status;
    let watch_playback = sections.playback;
    let watch_overlay = sections.overlay;
    let watch_epoch = epoch.0;
    use_effect(move || {
        let _footer = watch_footer.cloned();
        let _summary = watch_summary.cloned();
        let _status = watch_status.cloned();
        let _playback = watch_playback.cloned();
        let overlay_open = watch_overlay.cloned();
        let _epoch = watch_epoch.cloned();
        // Reading the overlay registers the chooser's rows, which appear and
        // disappear with that section; the chooser's key retracts while none
        // is open.
        let chooser_open = matches!(
            overlay_open.as_ref(),
            Some(GuiOverlayFrame::TargetChooser { .. })
        );
        let rows = chooser_rows.cloned();
        let target = target_mounted.cloned();
        let collapse = collapse_mounted.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            // The band's own regions, in paint order: the playback target,
            // then the collapse control.
            let mut regions: Vec<LayoutRegion> = Vec::new();
            for (mounted, id) in [
                (target, QueuePlaybackTargetId::PlaybackTarget),
                (collapse, QueuePlaybackTargetId::Collapse),
            ] {
                if let Some(mounted) = mounted {
                    regions.extend(element_region(&mounted, RegionTarget::QueuePlayback(id)).await);
                }
            }
            effect_reporter.report_layout(
                key,
                LayoutContribution {
                    lists: Vec::new(),
                    regions,
                },
            );
            if chooser_open {
                let mut chooser_regions: Vec<LayoutRegion> = Vec::new();
                for (index, mounted) in rows.iter().enumerate() {
                    if let Some(mounted) = mounted {
                        chooser_regions.extend(
                            element_region(
                                mounted,
                                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(
                                    index,
                                )),
                            )
                            .await,
                        );
                    }
                }
                effect_reporter.report_layout(
                    chooser_key,
                    LayoutContribution {
                        lists: Vec::new(),
                        regions: chooser_regions,
                    },
                );
            } else {
                effect_reporter.retract_layout(chooser_key);
            }
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
        drop_reporter.retract_layout(chooser_key);
    });

    let footer = sections.footer.cloned();
    let summary = sections.summary.cloned();
    let status = sections.status.cloned();
    let playback = sections.playback.cloned();
    // The target control's colour: the flag picks the local or the remote
    // role.
    let target_color = if playback.host_is_remote {
        role_var(GuiRole::PlaybackTargetRemoteFg)
    } else {
        role_var(GuiRole::PlaybackTargetLocalFg)
    };
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
        // 1 px top border, and the canvas's mono type. The left group leads
        // with the playback target and the collapse control, then the queue's
        // source with its autosave and dirty states; the centre carries the
        // prefix badge and the toast, and the right group the queue's totals.
        // The open chooser's popover is the band's child, anchored above it.
        div {
            class: "relative flex items-center font-mono whitespace-nowrap",
            style: "height: 40px; padding-inline: 16px; font-size: 12px; border-top: 1px solid var(--canvas-border); background: var(--mbv-queueFooterBand);",
            "data-footer-line": "",
            // The Queue zone (`gui-pointer-target-interaction` D1) reaches the
            // footer band: it sits under the Queue list box, outside the box's
            // own element, so it carries the marker too — and the popover,
            // which opens above it, stays in the zone with it.
            "data-pointer-zone": PointerZone::Queue.token(),
            if let Some((cursor, choices)) = chooser {
                // The playback-target chooser: the core's live choice list,
                // the attached output marked with a check. The rows report
                // their `targetChoice` indices under the band's chooser key,
                // last over every region beneath them (the module comment
                // names why).
                TargetChooserPopover {
                    cursor,
                    choices,
                    chooser_rows,
                }
            }
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
                        class: "flex min-w-0 items-center",
                        style: "height: 28px; padding: 0 8px; gap: 6px; color: {target_color};",
                        "data-target": "playbackTarget",
                        "aria-label": "Cast to device",
                        onmounted: move |event| target_mounted.set(Some(event)),
                        Cast { size: 18, stroke_width: 1.8, color: "currentColor" }
                        if !playback.host.is_empty() {
                            // The band's 12 px mono type is the label's
                            // inherited face and size: the box carries no
                            // style of its own.
                            Truncate {
                                text: playback.host.clone(),
                                style: TextStyle::mono(12.0, 400.0),
                            }
                        }
                        ChevronDown { size: 12, color: "currentColor" }
                    }
                    button {
                        "type": "button",
                        class: "flex shrink-0 items-center justify-center",
                        style: "width: 28px; height: 28px;",
                        "data-target": "collapse",
                        "aria-label": "Collapse panel",
                        title: "Collapse panel (z)",
                        onmounted: move |event| collapse_mounted.set(Some(event)),
                        Maximize2 { size: 18, stroke_width: 1.8, color: role_var(GuiRole::TextMuted) }
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
                // The centre: the armed prefix badge (dropped while a toast
                // is live, the TUI's pill-drop rule), then the toast inline in
                // the line in its severity colour (the footer-line toast form
                // the spec permits); empty while neither is live.
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

/// The open playback-target chooser's props: the core's cursor and live
/// choice list, and the row handles the band's chooser regions read.
#[derive(Props, Clone, PartialEq)]
struct TargetChooserPopoverProps {
    /// The core's cursor row.
    cursor: usize,
    /// The choices the frame's `targetChoices` list carries.
    choices: Vec<GuiTargetChoice>,
    /// The painted rows' mounted events, by their choice index.
    chooser_rows: Signal<Vec<Option<Event<MountedData>>>>,
}

impl std::fmt::Debug for TargetChooserPopoverProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TargetChooserPopoverProps")
            .field("cursor", &self.cursor)
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// The chooser's popover, anchored above the footer band (design D3): the
/// band sits at the window's bottom edge, so the popover opens upward from the
/// pressed control, its left edge on the control's. Each row's `onmounted`
/// hands its event to the band's chooser-regions signal, under the row's
/// `data-choice` index.
#[component]
fn TargetChooserPopover(props: TargetChooserPopoverProps) -> Element {
    rsx! {
        div {
            class: "absolute bottom-full z-20 flex min-w-56 flex-col rounded-md border py-1 shadow-md",
            // The band's mono 12 px type is not the popover's: it keeps the
            // 16 px body face its rows painted in the header, which is the
            // face the rows' `Truncate` measure names.
            style: "margin-bottom: 4px; left: 16px; font-family: var(--font-sans); font-size: 16px; background: var(--mbv-popover); border-color: var(--mbv-border);",
            "data-target-chooser": "",
            for (index, choice) in props.choices.iter().enumerate() {
                {
                    let row_color = if index == props.cursor {
                        role_var(GuiRole::TextFocusAccent)
                    } else {
                        role_var(GuiRole::TextPrimary)
                    };
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex items-center text-left",
                            style: "padding: calc(var(--mbv-cell-row) / 4) calc(var(--mbv-cell-col) * 1); gap: var(--mbv-cell-col); color: {row_color};",
                            "data-choice": "{index}",
                            onmounted: {
                                let chooser_rows = props.chooser_rows;
                                move |event| record_row_handle(chooser_rows, index, event)
                            },
                            span {
                                class: "flex w-4 shrink-0 justify-center",
                                if choice.is_active {
                                    Check { size: 14, color: role_var(GuiRole::Accent) }
                                }
                            }
                            match &choice.key {
                                GuiTargetKey::Local => rsx! {
                                    Monitor { size: 14, color: role_var(GuiRole::PlaybackTargetLocalFg) }
                                },
                                GuiTargetKey::Cast(_) => rsx! {
                                    TargetBrandIcon { brand: TargetBrand::Cast }
                                },
                                GuiTargetKey::Emby(_) if choice.is_direct_peer => rsx! {
                                    TargetBrandIcon { brand: TargetBrand::DirectPeer }
                                },
                                GuiTargetKey::Emby(_) => rsx! {
                                    TargetBrandIcon { brand: TargetBrand::Emby }
                                },
                            }
                            Truncate {
                                text: choice.label.clone(),
                                // The chooser button names no font size: the
                                // label inherits the document's 16 px body face.
                                style: TextStyle::sans(16.0, 400.0),
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
