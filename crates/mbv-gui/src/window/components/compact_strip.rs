//! The panel's compact strip (task 5.7, the port of `CompactStrip`;
//! add-tauri-gui-client row 6.6, design D10): the state a container shorter
//! than 520 px paints. The canvas's strip is a 96×54 still, the title, its
//! meta line (the parent line and the next item), a thin progress line, and
//! the transport's play/pause and next.
//!
//! The strip is size-driven, so it paints no collapse or fold control: the
//! `z` key still collapses it to the rail through the core, and the rail's
//! own control brings the panel back. Both controls report the
//! `QueuePlayback` target ids, so a press resolves through the same core
//! arms as the expanded header's controls.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::components::art_image::ArtImage;
use crate::window::icons::{Pause, Play, SkipForward};
use crate::window::layout::region_set::use_region_set;
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::{glyph_color, role_var};
use crate::window::truncate::{TextStyle, Truncate};

/// The still's canvas size (the spec's 96×54).
const STILL_WIDTH_PX: i32 = 96;
const STILL_HEIGHT_PX: i32 = 54;

/// The props of the compact strip (the `CompactStrip` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct CompactStripProps {
    /// The next item's title (`GuiQueueSummary.nextTitle`, review of this
    /// row's finding P2), `None` while no slot follows the playhead.
    #[props(default)]
    pub next: Option<String>,
    /// The artwork the window carries for the active item (design D10).
    #[props(default)]
    pub art: Option<GuiArtRef>,
}

/// The compact strip: the still at the leading edge, the title and meta
/// lines in the middle, and the transport at the trailing edge.
#[component]
pub fn CompactStrip(props: CompactStripProps) -> Element {
    let sections = consume_context::<Sections>();
    let playhead = consume_context::<Playhead>();

    // Each paint that moved a region reports (design D1): the region set's one
    // effect watches the playback band and the layout epoch. The marks run in
    // this render, in paint order — the still's slot, then the transport's two
    // controls — so the report keeps today's region order.
    let watch_playback = sections.playback;
    let region_set = use_region_set(0, move || {
        let _playback = watch_playback.cloned();
    });

    let playback = sections.playback.cloned();
    // Whether the transport is paused: the anchor's fact (design D5). `true`
    // while no anchor holds — the idle transport paints a play control.
    let paused = playhead.anchor.cloned().is_none_or(|anchor| anchor.paused);
    // The played fraction of the thin progress line, clamped to `[0, 1]`:
    // the playhead's prediction (design D5).
    let progress = playhead.fraction.cloned();
    let progress_style = format!(
        "width: {}%; background: var(--mbv-accent);",
        progress * 100.0
    );
    // The meta line: the parent line and the next item, whichever the frame
    // carries, joined as one truncating line.
    let meta = [
        playback.parent_line.clone(),
        props.next.as_ref().map(|next| format!("Next: {next}")),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let next_glyph = glyph_color(playback.availability.next, GuiRole::TextStrong);

    rsx! {
        // The strip: one row across the column's top, the still at the
        // leading edge, the title and meta lines in the middle, and the
        // transport at the trailing edge. The thin progress line sits under
        // the meta line.
        div {
            class: "flex items-center",
            style: "padding: var(--mbv-cell-row) calc(var(--mbv-cell-col) * 2); gap: calc(var(--mbv-cell-col) * 2); background: var(--mbv-queuePlaybackBand);",
            div {
                class: "flex shrink-0 items-center justify-center overflow-hidden rounded-sm",
                style: "width: {STILL_WIDTH_PX}px; height: {STILL_HEIGHT_PX}px;",
                "data-target": "visualSlot",
                onmounted: region_set.mark(RegionTarget::QueuePlayback(
                    QueuePlaybackTargetId::VisualSlot,
                )),
                ArtImage {
                    art: props.art.clone(),
                    alt: String::new(),
                    class: "h-full w-full object-cover".to_owned(),
                    fallback: rsx! {
                        div {
                            class: "h-full w-full",
                            style: "background: var(--mbv-textMuted);",
                        }
                    },
                }
            }
            div {
                class: "flex min-w-0 flex-1 flex-col",
                style: "gap: calc(var(--mbv-cell-row) / 4);",
                Truncate {
                    text: playback.now_playing_title.clone().unwrap_or_default(),
                    class: "font-semibold".to_owned(),
                    css: format!("color: {};", role_var(GuiRole::PlaybackTitleFg)),
                    style: TextStyle::sans(16.0, 600.0),
                }
                if !meta.is_empty() {
                    Truncate {
                        text: meta,
                        class: "text-xs".to_owned(),
                        css: format!("color: {};", role_var(GuiRole::PlaybackContextFg)),
                        style: TextStyle::sans(12.0, 400.0),
                    }
                }
                // The thin progress line: the played fraction over the track.
                span {
                    class: "block overflow-hidden rounded-full",
                    style: "height: 2px; background: var(--mbv-progressTrack);",
                    "data-progress": "",
                    span {
                        class: "block h-full rounded-full",
                        style: "{progress_style}",
                    }
                }
            }
            button {
                "type": "button",
                class: "flex shrink-0 items-center justify-center rounded-full p-1",
                "aria-label": if paused { "Play" } else { "Pause" },
                title: if paused { "Play" } else { "Pause" },
                style: "background: var(--mbv-secondary);",
                "data-target": "playPause",
                onmounted: region_set.mark(RegionTarget::QueuePlayback(
                    QueuePlaybackTargetId::PlayPause,
                )),
                if paused {
                    Play {
                        size: 16,
                        fill: role_var(GuiRole::Accent),
                        color: role_var(GuiRole::Accent),
                    }
                } else {
                    Pause {
                        size: 16,
                        fill: role_var(GuiRole::TextFocusAccent),
                        color: role_var(GuiRole::TextFocusAccent),
                    }
                }
            }
            button {
                "type": "button",
                class: "flex shrink-0 items-center justify-center",
                "data-target": "next",
                "aria-label": "Next",
                title: "Next",
                onmounted: region_set.mark(RegionTarget::QueuePlayback(
                    QueuePlaybackTargetId::Next,
                )),
                SkipForward {
                    size: 18,
                    fill: "{next_glyph}",
                    color: "{next_glyph}",
                }
            }
        }
    }
}
