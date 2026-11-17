//! The panel's rail state (task 5.7, the port of `CollapsedRail`;
//! add-tauri-gui-client row 6.6, design D10): the panel collapsed to 72 px
//! by the `z` toggle. The canvas's rail is the artwork with the played
//! fraction as a ring around it, the transport's play/pause and next, the
//! remaining time, and the control that expands the panel again.
//!
//! The expand control reports the `collapse` target, so its press runs the
//! same `z` toggle the expanded header's control runs — one core path for
//! both directions, as the toggle's own rule ("pressing `z` again restores
//! the previous state") requires. The rail paints nothing else: the queue
//! and the footer belong to the expanded artboard, and the core keeps
//! their state while the rail is up (the `gui-client` spec: the window
//! holds no interaction state).

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::arrangement::QUEUE_RAIL_WIDTH_PX;
use crate::window::components::art_image::ArtImage;
use crate::window::icons::{ChevronsLeft, Pause, Play, SkipForward};
use crate::window::layout::{
    LayoutContribution, LayoutEpoch, LayoutKey, LayoutReporter, element_region,
};
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::{glyph_color, role_var};

/// The rail's width is the Queue panel's collapsed width (`arrangement.ts`),
/// the same number the window measures the Library column against. The ring
/// hugs the artwork, and its stroke spans the played fraction.
const RING_SIZE_PX: f32 = 52.0;
const RING_STROKE_PX: f32 = 3.0;
const RING_RADIUS_PX: f32 = (RING_SIZE_PX - RING_STROKE_PX) / 2.0;
const RING_CIRCUMFERENCE_PX: f32 = 2.0 * std::f32::consts::PI * RING_RADIUS_PX;
const ART_SIZE_PX: f32 = RING_SIZE_PX - 2.0 * RING_STROKE_PX - 4.0;

/// The props of the collapsed rail (the `CollapsedRail` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct CollapsedRailProps {
    /// The artwork the window carries for the active item (design D10):
    /// the playback section arms it on a change and `Panel` keeps it, so
    /// the rail paints the same art the expanded artboard showed. `None`
    /// while the window has carried none.
    #[props(default)]
    pub art: Option<GuiArtRef>,
}

/// The collapsed rail: the ring and transport centred, the remaining time
/// under them, and the expand control at the bottom.
#[component]
pub fn CollapsedRail(props: CollapsedRailProps) -> Element {
    let sections = consume_context::<Sections>();
    let playhead = consume_context::<Playhead>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let key = use_hook(LayoutKey::next);

    let mut art_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut play_pause_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut next_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut expand_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);

    // No region outlives the rail: the contribution retracts on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    // Each paint that moved a region reports (design D4): this effect runs
    // after the DOM update, and the layout epoch reports layout-driven moves.
    // The reporter dedupes an unchanged contribution.
    let watch_playback = sections.playback;
    let watch_epoch = epoch.0;
    use_effect(move || {
        let _playback = watch_playback.cloned();
        let _epoch = watch_epoch.cloned();
        let art = art_mounted.cloned();
        let play_pause = play_pause_mounted.cloned();
        let next = next_mounted.cloned();
        let expand = expand_mounted.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            // The painted regions (design D4): the transport's two controls,
            // the artwork slot, and the expand control.
            let mut regions: Vec<crate::bridge::layout::LayoutRegion> = Vec::new();
            for (mounted, id) in [
                (play_pause, QueuePlaybackTargetId::PlayPause),
                (next, QueuePlaybackTargetId::Next),
                (art, QueuePlaybackTargetId::VisualSlot),
                (expand, QueuePlaybackTargetId::Collapse),
            ] {
                if let Some(mounted) = mounted.as_ref() {
                    regions.extend(element_region(mounted, RegionTarget::QueuePlayback(id)).await);
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
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
    });

    let playback = sections.playback.cloned();
    // Whether the transport is paused: the anchor's fact (design D5). `true`
    // while no anchor holds — the idle transport paints a play control.
    let paused = playhead.anchor.cloned().is_none_or(|anchor| anchor.paused);
    // The played fraction the ring paints, clamped to `[0, 1]`: the
    // playhead's prediction (design D5).
    let progress = playhead.fraction.cloned();
    // The played arc's length in ring pixels.
    let played_arc = crate::window::input::f32_from_f64(progress) * RING_CIRCUMFERENCE_PX;
    let ring_centre = RING_SIZE_PX / 2.0;
    // The remaining time paints only while the runtime is known (the
    // the former client `$runtimeTicks > 0`).
    let show_remaining = playhead.runtime.cloned() > 0;
    let remaining = playhead.remaining.cloned();
    let next_glyph = glyph_color(playback.availability.next, GuiRole::TextStrong);

    rsx! {
        // The rail: one narrow column at the column's leading edge, the
        // ring and the transport centred, the remaining time under them, and
        // the expand control at the bottom.
        div {
            class: "flex h-full shrink-0 flex-col items-center",
            style: "width: {QUEUE_RAIL_WIDTH_PX}px; padding: var(--mbv-cell-row) 0; gap: var(--mbv-cell-row); background: var(--canvas-dim);",
            "data-panel-state": "rail",
            // The artwork with the played fraction as a ring around it: the
            // track circle in the progress-track role, the arc in the
            // accent. Each circle names `currentColor` and carries its role in
            // its own computed `color`: Blitz serialises the inline svg to usvg
            // for painting, and usvg cannot parse a `var()` in a `stroke`
            // attribute (it drops the stroke), while the serialiser resolves
            // `currentColor` per element.
            div {
                class: "relative flex shrink-0 items-center justify-center",
                style: "width: {RING_SIZE_PX}px; height: {RING_SIZE_PX}px;",
                "data-target": "visualSlot",
                onmounted: move |event| art_mounted.set(Some(event)),
                svg {
                    class: "absolute inset-0 -rotate-90",
                    view_box: "0 0 {RING_SIZE_PX} {RING_SIZE_PX}",
                    width: "{RING_SIZE_PX}",
                    height: "{RING_SIZE_PX}",
                    "aria-hidden": "true",
                    circle {
                        cx: "{ring_centre}",
                        cy: "{ring_centre}",
                        r: "{RING_RADIUS_PX}",
                        fill: "none",
                        stroke: "currentColor",
                        style: "color: var(--mbv-progressTrack);",
                        "stroke-width": "{RING_STROKE_PX}",
                    }
                    circle {
                        cx: "{ring_centre}",
                        cy: "{ring_centre}",
                        r: "{RING_RADIUS_PX}",
                        fill: "none",
                        stroke: "currentColor",
                        style: "color: var(--mbv-accent);",
                        "stroke-width": "{RING_STROKE_PX}",
                        "stroke-linecap": "round",
                        "stroke-dasharray": "{played_arc} {RING_CIRCUMFERENCE_PX}",
                    }
                }
                ArtImage {
                    art: props.art.clone(),
                    alt: String::new(),
                    class: "rounded-full object-cover".to_owned(),
                    style: format!("width: {ART_SIZE_PX}px; height: {ART_SIZE_PX}px"),
                    fallback: rsx! {
                        div {
                            class: "rounded-full",
                            style: "width: {ART_SIZE_PX}px; height: {ART_SIZE_PX}px; background: var(--mbv-textMuted);",
                        }
                    },
                }
            }
            // The transport's play/pause and next, the rail's two controls
            // (the spec's rail list).
            button {
                "type": "button",
                class: "flex items-center justify-center rounded-full p-1",
                "aria-label": if paused { "Play" } else { "Pause" },
                title: if paused { "Play" } else { "Pause" },
                style: "background: var(--mbv-secondary);",
                "data-target": "playPause",
                onmounted: move |event| play_pause_mounted.set(Some(event)),
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
                class: "flex items-center justify-center",
                "data-target": "next",
                "aria-label": "Next",
                title: "Next",
                onmounted: move |event| next_mounted.set(Some(event)),
                SkipForward {
                    size: 18,
                    fill: "{next_glyph}",
                    color: "{next_glyph}",
                }
            }
            if show_remaining {
                // The remaining time: the same `-M:SS` the expanded
                // scrubber's right end paints, from the playhead's
                // prediction (design D5). The rail paints a literal `-`
                // until the collapsed-states work picks the canvas's U+2212
                // convention up.
                span {
                    class: "font-mono text-xs tabular-nums",
                    style: "color: {role_var(GuiRole::PlaybackMetaFg)};",
                    "data-remaining": "",
                    "-{remaining}"
                }
            }
            span { class: "flex-1" }
            // The expand control: the `z` toggle's second direction, the
            // same `collapse` target the header's control reports.
            button {
                "type": "button",
                class: "flex items-center justify-center rounded-sm p-1",
                "data-target": "collapse",
                "aria-label": "Expand panel",
                title: "Expand panel (z)",
                onmounted: move |event| expand_mounted.set(Some(event)),
                ChevronsLeft {
                    size: 16,
                    color: role_var(GuiRole::TextMuted),
                }
            }
        }
    }
}
