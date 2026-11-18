//! The scrubber seam (task 5.3): the port of `QueuePlaybackPanel`'s
//! scrubber block — the 6 px track, the intro and credits blocks, the played
//! fill, the 2×14 chapter ticks, the 16 px thumb, the hover position line,
//! the hover preview with the trickplay tile, and the elapsed and
//! `−remaining` labels.
//!
//! The scrubber paints the canvas geometry. Its hover preview paints from
//! the `hover` section (the core resolves the pointer move against the
//! painted bar), so the component holds no hover state; the trickplay tile
//! stays a `[trickplay]` box until a sheet is cached. A press on the bar
//! forwards like every other pointer event: the component interprets no
//! gesture, it reports its regions and the core resolves the press against
//! the hit map (design D4).

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::playback::{GuiChapter, GuiScrubberHover, GuiSegmentMark};
use crate::bridge::frame::theme::GuiRole;
use crate::window::playhead::{Playhead, ticks_f64};
use crate::window::sections::Sections;
use crate::window::theme::role_var;
use crate::window::time::{clamp_ratio, minus};

#[cfg(test)]
mod tests;

/// The seekbar region's element handle (design D4): the scrubber's slider
/// box is the `seekbar` region of the panel's one report, so the panel holds
/// the handle and the scrubber sets it on mount.
#[derive(Props, Clone, PartialEq)]
pub struct ScrubberProps {
    /// The `seekbar` element's mounted event, `None` while the element is
    /// absent.
    #[props(default)]
    pub seekbar_mounted: Signal<Option<Event<MountedData>>>,
}

/// The hover preview's painted width: the canvas's `w-32` box.
const TILE_WIDTH_PX: f64 = 128.0;

/// One tick count as f64, so the sprite fractions divide without repeated
/// casts.
fn ticks(value: u32) -> f64 {
    mbv_emby_model::i64_to_f64_saturating(i64::from(value))
}

/// The chapter boundaries the bar paints as 2×14 ticks, skipping the first
/// boundary (the bar's own left edge) and any outside the runtime, in bar
/// fractions.
fn visible_chapters(chapters: &[GuiChapter], runtime: i64) -> Vec<f64> {
    chapters
        .iter()
        .filter(|chapter| chapter.start_ticks > 0 && chapter.start_ticks < runtime)
        .map(|chapter| clamp_ratio(ticks_f64(chapter.start_ticks), ticks_f64(runtime)))
        .collect()
}

/// One mark's start in bar fractions, `None` while unknown or outside the
/// runtime.
fn mark_left(mark: Option<&GuiSegmentMark>, runtime: i64) -> Option<f64> {
    let mark = mark?;
    if runtime <= 0 {
        return None;
    }
    Some(clamp_ratio(ticks_f64(mark.start_ticks), ticks_f64(runtime)))
}

/// The hover preview's tile crop (task 6.7): the thumbnail's place in its
/// sheet, scaled to the preview box, as the sprite the paint crops. The
/// sprite loads the sheet by its URL. `None` while the sheet is still
/// loading, so the preview paints the in-flight box.
fn tile_crop(hover: &GuiScrubberHover) -> Option<String> {
    let tile = hover.tile.as_ref()?;
    let src = match &tile.sheet {
        GuiArtRef::Image { src } | GuiArtRef::Placeholder { src } => src.clone(),
        GuiArtRef::Loading => return None,
    };
    if tile.width == 0 {
        return None;
    }
    let scale = TILE_WIDTH_PX / ticks(tile.width);
    Some(format!(
        "width: {TILE_WIDTH_PX}px; height: {}px; border: 1px solid var(--mbv-border); \
         border-radius: 4px; background-image: url('{src}'); background-size: {}px auto; \
         background-position: -{}px -{}px;",
        ticks(tile.height) * scale,
        ticks(tile.columns) * TILE_WIDTH_PX,
        ticks(tile.column) * TILE_WIDTH_PX,
        ticks(tile.row) * ticks(tile.height) * scale,
    ))
}

/// The scrubber (`main` audits 9–12): the 46 px block with the track at
/// 12 px, the intro and credits blocks, the played fill, the chapter ticks,
/// the pointer's position line, the thumb, and the elapsed and
/// `−remaining` labels at 28 px. The block's own rect is the
/// `seekbar` region; a press resolves there in the core.
///
/// Every length in the block multiplies by the control rows' `--player-scale`
/// (`gui-art-player-overlay` design D3): the rows shrink as one unit, so the
/// track, the ticks, the thumb, and the labels keep their places inside the
/// block at any scale. The hover preview above the bar is not a row size and
/// keeps its pixels.
///
/// The played fraction and the clocks are the playhead's prediction (design
/// D5); the chapter, mark, and hover facts are the frame's.
#[component]
pub fn Scrubber(mut props: ScrubberProps) -> Element {
    let sections = consume_context::<Sections>();
    let playhead = consume_context::<Playhead>();

    let playback = sections.playback.cloned();
    let runtime = playhead.runtime.cloned();
    // The played fraction of the scrubber, clamped to `[0, 1]`: the
    // playhead's prediction (design D5).
    let progress_pct = playhead.fraction.cloned() * 100.0;
    let elapsed = playhead.elapsed.cloned();
    let remaining = playhead.remaining.cloned();
    // The scrubber's hover preview, painted from the hover section: the
    // pointer's fraction, the time it names, the chapter covering it, and
    // the trickplay tile. `None` while the pointer is away from the bar.
    let hover_preview = sections.hover.cloned();

    let chapter_ticks = visible_chapters(&playback.chapters, runtime);
    let intro_left = mark_left(playback.intro_mark.as_ref(), runtime);
    let credits_left = mark_left(playback.credits_mark.as_ref(), runtime);
    let tile_crop = hover_preview.as_ref().and_then(tile_crop);
    let hover_pct = hover_preview.as_ref().map(|hover| hover.fraction * 100.0);
    let muted = role_var(GuiRole::TextMuted);
    let title_fg = role_var(GuiRole::PlaybackTitleFg);

    rsx! {
        div {
            style: "padding: calc(6px * var(--player-scale)) calc(16px * var(--player-scale)) calc(2px * var(--player-scale));",
            div {
                class: "relative",
                role: "slider",
                tabindex: 0,
                "aria-label": "Seek",
                "aria-valuemin": "0",
                "aria-valuemax": "100",
                "aria-valuenow": "{progress_pct.round()}",
                style: "height: calc(46px * var(--player-scale));",
                "data-target": "seekbar",
                onmounted: move |event| props.seekbar_mounted.set(Some(event)),
                if let Some(hover) = hover_preview.clone() {
                    // The hover preview (`main` audit 12): the 128×72 trickplay
                    // tile stacked over the label chip, which reads the time
                    // and the covering chapter's number, not its name. A
                    // hovered tile paints the sheet by its URL, and the
                    // canvas `[trickplay]` box while the sheet's fetch is
                    // still in flight; a position the item offers no tile
                    // for paints the label chip alone (the spec's
                    // no-trickplay scenario).
                    HoverPreview { hover, tile_crop }
                }
                div {
                    class: "absolute inset-x-0",
                    "data-track": "",
                    style: "top: calc(12px * var(--player-scale)); height: calc(6px * var(--player-scale)); background: var(--mbv-progressTrack); border-radius: calc(3px * var(--player-scale));",
                }
                if let Some(left) = intro_left {
                    div {
                        class: "absolute",
                        style: "left: {left * 100.0}%; top: calc(12px * var(--player-scale)); width: calc(6px * var(--player-scale)); height: calc(6px * var(--player-scale)); background: var(--mbv-progressPercent); border-radius: calc(3px * var(--player-scale)) 0 0 calc(3px * var(--player-scale));",
                        title: "Intro",
                    }
                }
                div {
                    class: "absolute left-0",
                    "data-played": "",
                    style: "top: calc(12px * var(--player-scale)); width: {progress_pct}%; height: calc(6px * var(--player-scale)); background: var(--mbv-accent); border-radius: calc(3px * var(--player-scale));",
                }
                for ratio in chapter_ticks.iter().copied() {
                    div {
                        class: "absolute",
                        style: "left: {ratio * 100.0}%; top: calc(8px * var(--player-scale)); width: calc(2px * var(--player-scale)); height: calc(14px * var(--player-scale)); background: {muted};",
                    }
                }
                if let Some(left) = credits_left {
                    div {
                        class: "absolute",
                        style: "left: {left * 100.0}%; top: calc(12px * var(--player-scale)); width: calc(4px * var(--player-scale)); height: calc(6px * var(--player-scale)); background: var(--mbv-progressPercent);",
                        title: "Credits",
                    }
                }
                if let Some(hover_pct) = hover_pct {
                    div {
                        class: "pointer-events-none absolute",
                        "data-hover-line": "",
                        style: "left: {hover_pct}%; top: calc(4px * var(--player-scale)); width: calc(1px * var(--player-scale)); height: calc(22px * var(--player-scale)); background: {title_fg};",
                    }
                }
                div {
                    class: "absolute -translate-x-1/2 rounded-full",
                    "data-thumb": "",
                    style: "left: {progress_pct}%; top: calc(7px * var(--player-scale)); width: calc(16px * var(--player-scale)); height: calc(16px * var(--player-scale)); background: {title_fg}; box-shadow: 0 0 0 calc(3px * var(--player-scale)) var(--mbv-queuePlaybackBand);",
                }
                div {
                    class: "absolute inset-x-0 flex justify-between font-mono tabular-nums",
                    style: "top: calc(28px * var(--player-scale)); font-size: calc(12px * var(--player-scale));",
                    // The clocks sit on the stage's dark scrim, so they paint
                    // the primary text role for contrast rather than the time
                    // token; the remaining label dims slightly (0.85) to
                    // stay secondary. The clocks are the playhead's
                    // prediction (design D5).
                    span {
                        style: "color: var(--mbv-textPrimary);",
                        "{elapsed}"
                    }
                    span {
                        style: "color: var(--mbv-textPrimary); opacity: 0.85;",
                        "{minus(&remaining)}"
                    }
                }
            }
        }
    }
}

/// The hover preview's props: the hovered position's facts and the tile crop
/// the sprite paints, `None` while the sheet is in flight.
#[derive(Props, Clone, PartialEq)]
struct HoverPreviewProps {
    /// The hovered position the `hover` section carries.
    hover: GuiScrubberHover,
    /// The hovered tile's sprite styles, `None`
    /// while no tile is offered or its sheet is still loading.
    tile_crop: Option<String>,
}

impl std::fmt::Debug for HoverPreviewProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HoverPreviewProps")
            .field("hover", &self.hover)
            .field("tile_crop", &self.tile_crop)
            .finish()
    }
}

/// The hover preview's block: the tile (cropped sprite or the in-flight box)
/// stacked over the label chip.
#[component]
fn HoverPreview(props: HoverPreviewProps) -> Element {
    let hover_pct = props.hover.fraction * 100.0;
    // The preview label reads the canvas's `12:31 · Ch. 2` form: the time
    // and the chapter's number, not its full name (`main` audit 12).
    let label = match props.hover.chapter_number {
        Some(number) => format!("{} \u{b7} Ch. {number}", props.hover.time_label),
        None => props.hover.time_label.clone(),
    };
    let muted_fg = role_var(GuiRole::TextSecondary);
    let title_fg = role_var(GuiRole::PlaybackTitleFg);
    rsx! {
        div {
            class: "pointer-events-none absolute z-10 flex -translate-x-1/2 flex-col items-center",
            style: "left: {hover_pct}%; top: -94px; width: 128px; gap: 4px;",
            if props.hover.tile.is_some() {
                if let Some(crop) = props.tile_crop.clone() {
                    div {
                        class: "overflow-hidden bg-no-repeat",
                        style: "{crop}",
                    }
                } else {
                    div {
                        class: "flex items-center justify-center font-mono",
                        style: "width: 128px; height: 72px; font-size: 10px; background: var(--mbv-secondary); border: 1px solid var(--mbv-border); border-radius: 4px; color: {muted_fg};",
                        "[trickplay]"
                    }
                }
            }
            div {
                class: "font-mono",
                style: "font-size: 11px; padding: 2px 6px; border-radius: 3px; background: var(--mbv-background); color: {title_fg};",
                "{label}"
            }
        }
    }
}
