//! The scrubber seam (task 5.3): the port of `QueuePlaybackPanel`'s
//! scrubber block — the 6 px track, the played fill, the 16 px thumb, and
//! the elapsed and `−remaining` labels.
//!
//! The scrubber paints the canvas geometry. Its hover preview paints from
//! the `hover` section (the core resolves the pointer move against the
//! painted bar), so the component holds no hover state. A press on the bar
//! forwards like every other pointer event: the component interprets no
//! gesture, it reports its regions and the core resolves the press against
//! the hit map (design D4).

use dioxus_native::prelude::*;

use crate::bridge::frame::playback::GuiScrubberHover;
use crate::bridge::frame::theme::GuiRole;
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::role_var;
use crate::window::time::minus;

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
    /// The content the clock line paints between the elapsed and remaining
    /// clocks: the transport controls, which sit on the same line as the
    /// clocks and just under the bar. `None` paints the clocks alone.
    #[props(default)]
    pub center: Option<Element>,
}

/// The scrubber (`main` audits 9–12): the 24 px bar block with the track at
/// 12 px, the played fill, and the thumb; under it a 30 px clock line with
/// the elapsed and `−remaining` labels at its ends and the transport
/// controls (`center`) between them. The bar block's own rect is the
/// `seekbar` region; a press resolves there in the core.
///
/// Every length in the block multiplies by the control rows' `--player-scale`
/// (`gui-art-player-overlay` design D3): the rows shrink as one unit, so the
/// track, the thumb, and the labels keep their places inside the block at
/// any scale. The hover preview above the bar is not a row size and keeps
/// its pixels.
///
/// The played fraction and the clocks are the playhead's prediction (design
/// D5); the hover facts are the frame's.
#[component]
pub fn Scrubber(mut props: ScrubberProps) -> Element {
    let sections = consume_context::<Sections>();
    let playhead = consume_context::<Playhead>();

    // The played fraction of the scrubber, clamped to `[0, 1]`: the
    // playhead's prediction (design D5).
    let progress_pct = playhead.fraction.cloned() * 100.0;
    let elapsed = playhead.elapsed.cloned();
    let remaining = playhead.remaining.cloned();
    // The scrubber's hover preview, painted from the hover section: the
    // pointer's fraction and the time it names. `None` while the pointer is
    // away from the bar.
    let hover_preview = sections.hover.cloned();
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
                style: "height: calc(24px * var(--player-scale));",
                "data-target": "seekbar",
                onmounted: move |event| props.seekbar_mounted.set(Some(event)),
                if let Some(hover) = hover_preview.clone() {
                    // The hover preview: the time label chip at the pointer's
                    // position. No frame and no chapter name ride it (design
                    // D4).
                    HoverPreview { hover }
                }
                div {
                    class: "absolute inset-x-0",
                    "data-track": "",
                    style: "top: calc(12px * var(--player-scale)); height: calc(6px * var(--player-scale)); background: var(--mbv-progressTrack); border-radius: calc(3px * var(--player-scale));",
                }
                div {
                    class: "absolute left-0",
                    "data-played": "",
                    style: "top: calc(12px * var(--player-scale)); width: {progress_pct}%; height: calc(6px * var(--player-scale)); background: var(--mbv-accent); border-radius: calc(3px * var(--player-scale));",
                }
                div {
                    class: "absolute -translate-x-1/2 rounded-full",
                    "data-thumb": "",
                    style: "left: {progress_pct}%; top: calc(7px * var(--player-scale)); width: calc(16px * var(--player-scale)); height: calc(16px * var(--player-scale)); background: {title_fg}; box-shadow: 0 0 0 calc(3px * var(--player-scale)) var(--mbv-queuePlaybackBand);",
                }
            }
            // The clock line sits under the bar, outside the `seekbar` box so a
            // press on a control here never reads as a seek. The clocks take
            // the line's two ends and the transport controls its middle. The
            // clocks paint the primary text role for contrast on the stage's
            // dark scrim rather than the time token; the remaining label dims
            // slightly (0.85) to stay secondary. They are the playhead's
            // prediction (design D5).
            div {
                class: "flex items-center font-mono tabular-nums",
                style: "height: calc(30px * var(--player-scale)); font-size: calc(12px * var(--player-scale));",
                span {
                    class: "flex-1",
                    style: "color: var(--mbv-textPrimary);",
                    "{elapsed}"
                }
                if let Some(center) = props.center.clone() {
                    {center}
                }
                span {
                    class: "flex-1 text-right",
                    style: "color: var(--mbv-textPrimary); opacity: 0.85;",
                    "{minus(&remaining)}"
                }
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

/// The hover preview's block: the time label chip at the pointer's position.
#[component]
fn HoverPreview(props: HoverPreviewProps) -> Element {
    let hover_pct = props.hover.fraction * 100.0;
    let title_fg = role_var(GuiRole::PlaybackTitleFg);
    rsx! {
        div {
            class: "pointer-events-none absolute z-10 -translate-x-1/2",
            style: "left: {hover_pct}%; top: -22px;",
            div {
                class: "font-mono",
                style: "font-size: 11px; padding: 2px 6px; border-radius: 3px; background: var(--mbv-background); color: {title_fg};",
                "{props.hover.time_label}"
            }
        }
    }
}
