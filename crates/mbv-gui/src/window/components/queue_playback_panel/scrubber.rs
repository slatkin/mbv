//! The scrubber seam (task 5.3, `gui-queue-panel-redesign` design D4): the
//! port of `QueuePlaybackPanel`'s scrubber block — one full-width seek row
//! holding the track, the played fill, and the thumb, with the time tip while
//! the pointer is on the row. The clocks are the transport row's
//! (`art_stage.rs`), so the row is the whole of this component.
//!
//! The scrubber paints the canvas geometry. Its hover state paints from the
//! `hover` section (the core resolves the pointer move against the painted
//! row), so the component holds no hover state and the track grows with the
//! tip. A press on the row forwards like every other pointer event: the
//! component interprets no gesture, it reports its region and the core
//! resolves the press against the hit map (design D4).

use dioxus_native::prelude::*;

use crate::bridge::frame::playback::GuiScrubberHover;
use crate::window::layout::region_set::MarkHandler;
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::{EfToken, ef_var};

#[cfg(test)]
mod tests;

/// The seek row's height at scale 1.0 (design D3, D4): the whole row is the
/// `seekbar` region, so this is the bar's hit area.
pub(super) const SEEK_ROW_HEIGHT_PX: f32 = 14.0;

/// The track's height at scale 1.0, at rest and while the pointer is on the
/// row (design D4).
const TRACK_REST_PX: f32 = 4.0;
const TRACK_HOVER_PX: f32 = 6.0;

/// The thumb's diameter at scale 1.0, at rest and while the pointer is on the
/// row (design D4).
const THUMB_REST_PX: f32 = 14.0;
const THUMB_HOVER_PX: f32 = 16.0;

/// The rest track's grey: `--ef-grey2` at 28 % (design D4).
const TRACK_OPACITY: f32 = 0.28;

/// The thumb's ring width at scale 1.0 (design D4): `--ef-bg-dim` around the
/// green circle.
const THUMB_RING_PX: f32 = 2.0;

/// The distance the time tip sits above the row's top edge, in unscaled
/// pixels: a tip is not a row size (design D4).
const HOVER_TIP_OFFSET_PX: f64 = -22.0;

/// The seekbar region's mark (design D1, D4): the seek row is the `seekbar`
/// region of the panel's one report, so the panel's region set marks it and
/// hands the closure to this component.
#[derive(Props, Clone, PartialEq)]
pub struct ScrubberProps {
    /// The `seekbar` element's mark.
    #[props(default)]
    pub seekbar_mounted: MarkHandler,
}

/// The scrubber (`main` audits 9–12; `gui-queue-panel-redesign` design D4):
/// one full-width row, 14 px tall at scale 1.0, holding the track, the played
/// fill, and the thumb. The row's own rect is the `seekbar` region; a press
/// resolves there in the core.
///
/// Every length in the row multiplies by the control rows' `--player-scale`
/// (`gui-art-player-overlay` design D3): the rows shrink as one unit, so the
/// track, the thumb, and the tip keep their places inside the row at any
/// scale. The time tip above the row is not a row size and keeps its pixels.
///
/// The played fraction is the playhead's; the hover facts are the frame's.
#[component]
pub fn Scrubber(props: ScrubberProps) -> Element {
    let sections = consume_context::<Sections>();
    let playhead = consume_context::<Playhead>();

    // The played fraction of the row, clamped to `[0, 1]`: the playhead's
    // prediction (design D5 of `gui-art-player-overlay`).
    let progress_pct = playhead.fraction.cloned() * 100.0;
    // The row's hover facts, painted from the hover section: the pointer's
    // fraction and the time it names. `None` while the pointer is away from
    // the row. The section drives the bar's hover chrome too, so the track
    // grows with the tip and the state survives a drag (design D4).
    let hover_preview = sections.hover.cloned();
    let hovered = hover_preview.is_some();
    let track_px = if hovered {
        TRACK_HOVER_PX
    } else {
        TRACK_REST_PX
    };
    let thumb_px = if hovered {
        THUMB_HOVER_PX
    } else {
        THUMB_REST_PX
    };
    // Every part of the bar centres on the row's middle line.
    let track_top_px = (SEEK_ROW_HEIGHT_PX - track_px) / 2.0;
    let thumb_top_px = (SEEK_ROW_HEIGHT_PX - thumb_px) / 2.0;
    let track_radius_px = track_px / 2.0;
    // The row answers the pointer by growing its track and showing its tip, so
    // the shared veil stays off it: `--hover-veil: transparent` is the one
    // exception the seekbar takes from the `box-shadow` rule.
    let row_style = format!(
        "height: calc({SEEK_ROW_HEIGHT_PX}px * var(--player-scale)); --hover-veil: transparent;"
    );
    let track_style = format!(
        "top: calc({track_top_px}px * var(--player-scale)); height: calc({track_px}px * var(--player-scale)); background: {}; opacity: {TRACK_OPACITY}; border-radius: calc({track_radius_px}px * var(--player-scale));",
        ef_var(EfToken::Grey2)
    );
    let played_style = format!(
        "top: calc({track_top_px}px * var(--player-scale)); height: calc({track_px}px * var(--player-scale)); background: {}; border-radius: calc({track_radius_px}px * var(--player-scale));",
        ef_var(EfToken::Green)
    );
    let thumb_style = format!(
        "top: calc({thumb_top_px}px * var(--player-scale)); width: calc({thumb_px}px * var(--player-scale)); height: calc({thumb_px}px * var(--player-scale)); background: {}; box-shadow: 0 0 0 calc({THUMB_RING_PX}px * var(--player-scale)) {};",
        ef_var(EfToken::Green),
        ef_var(EfToken::BgDim)
    );
    rsx! {
        div {
            class: "relative",
            role: "slider",
            tabindex: 0,
            "aria-label": "Seek",
            "aria-valuemin": "0",
            "aria-valuemax": "100",
            "aria-valuenow": "{progress_pct.round()}",
            style: "{row_style}",
            "data-target": "seekbar",
            onmounted: props.seekbar_mounted,
            if let Some(hover) = hover_preview.clone() {
                // The time tip at the pointer's position. No frame and no
                // chapter name ride it (design D4).
                HoverPreview { hover }
            }
            div {
                class: "absolute inset-x-0",
                "data-track": "",
                style: "{track_style}",
            }
            div {
                class: "absolute left-0",
                "data-played": "",
                style: "{played_style} width: {progress_pct}%;",
            }
            div {
                class: "absolute -translate-x-1/2 rounded-full",
                "data-thumb": "",
                style: "{thumb_style} left: {progress_pct}%;",
            }
        }
    }
}

/// The hover preview's props: the hovered position's facts.
#[derive(Props, Clone, PartialEq)]
struct HoverPreviewProps {
    /// The hovered position the `hover` section carries.
    hover: GuiScrubberHover,
}

impl std::fmt::Debug for HoverPreviewProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HoverPreviewProps")
            .field("hover", &self.hover)
            .finish()
    }
}

/// The hover preview's block: the time label chip at the pointer's position
/// (design D4), on the menu fill.
#[component]
fn HoverPreview(props: HoverPreviewProps) -> Element {
    let hover_pct = props.hover.fraction * 100.0;
    let offset = HOVER_TIP_OFFSET_PX;
    rsx! {
        div {
            class: "pointer-events-none absolute z-10 -translate-x-1/2",
            style: "left: {hover_pct}%; top: {offset}px;",
            div {
                class: "font-mono",
                style: format!(
                    "font-size: 11px; padding: 2px 6px; border-radius: 3px; background: {}; color: {};",
                    ef_var(EfToken::Bg2),
                    ef_var(EfToken::Fg)
                ),
                "{props.hover.time_label}"
            }
        }
    }
}
