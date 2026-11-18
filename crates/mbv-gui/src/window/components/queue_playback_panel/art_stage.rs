//! The player stage and its overlay layers (`gui-art-player-overlay` design
//! D1, D6, D8): the stage wrapper, the clipped media box, the top layer with
//! the track chips and the episode badge, and the bottom layer with its
//! gradient and the control row.
//!
//! The stage is the visual slot: a `relative` wrapper with no clip that
//! carries the `visualSlot` region, so the panel reports one region for the
//! whole box. The media box is the stage's clipped sibling, so a chip popover
//! that hangs past the artwork is never cut. The top and bottom layers are
//! absolute siblings over the media box; a press on a control lands on the
//! control's own region, which the panel pushes after the stage's (design D4).
//!
//! The control row is one component ([`ControlRow`]) because two
//! arrangements paint it: the stage's bottom layer, and the no-image column
//! the panel paints for `Placeholder` and `None` artwork, where no stage
//! exists (design D6, D7). [`ChipsAndBadge`] is shared the same way: the top
//! layer holds it, and the no-image column paints it as its own line.
//!
//! The sizes are the canvas's pixels times the measured `--player-scale`
//! (design D3): the panel holds the scale in a window signal and the bottom
//! layer carries it, so the rows and everything inside them shrink as one
//! unit while the no-image column paints it at full size.
//!
//! The auto-hide control is not here: the player panel's right-click menu
//! carries it (gui-player-overlay-menu), so the bottom layer paints one
//! row and the stage's height limit names that row alone.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{GuiTrackChips, GuiTransportAvailability};
use crate::bridge::frame::theme::GuiRole;
use crate::window::components::art_image::ArtImage;
use crate::window::components::track_chips::TrackChips;
use crate::window::icons::{Pause, Play, SkipBack, SkipForward};
use crate::window::theme::{glyph_color, role_var};

use super::scrubber::Scrubber;

#[cfg(test)]
mod tests;

/// The bottom layer's full height at scale 1.0 (design D3): the layer's own
/// 6 px top pad over the scrubber block's 6 px pad, the 24 px bar, the 30 px
/// clock line, and the 2 px tail pad. The scale's one-third share is
/// measured against this height, and the pad scales with the row, so the
/// layer takes no more than one third of the stage at every height.
pub(super) const BOTTOM_FULL_HEIGHT_PX: f32 = 68.0;

/// The control row's scale for a stage `stage_height` px tall (design D3):
/// the row takes at most a third of the stage and never grows past its full
/// size. There is no floor, so the one-third limit holds at every height.
pub(super) fn player_scale(stage_height: f32) -> f32 {
    (stage_height / 3.0 / BOTTOM_FULL_HEIGHT_PX).min(1.0)
}

/// The mounted handles the control row sets: the panel holds every handle and
/// hands the same set to the stage and to the no-image column (design D8), so
/// one region report covers both arrangements.
#[derive(Clone, Copy, PartialEq)]
pub struct ControlRowHandles {
    /// The play/pause button.
    pub play_pause: Signal<Option<Event<MountedData>>>,
    /// The previous button.
    pub previous: Signal<Option<Event<MountedData>>>,
    /// The back-10 s button.
    pub seek_back: Signal<Option<Event<MountedData>>>,
    /// The forward-30 s button.
    pub seek_forward: Signal<Option<Event<MountedData>>>,
    /// The next button.
    pub next: Signal<Option<Event<MountedData>>>,
    /// The scrubber's slider box.
    pub seekbar: Signal<Option<Event<MountedData>>>,
}

/// The stage's props: the artwork, the two overlay layers' facts, and the
/// handles the panel reports.
#[derive(Props, Clone, PartialEq)]
pub struct ArtStageProps {
    /// The artwork the media box paints: `Image` or `Loading` only, because
    /// the panel paints the no-image column for the other states (design D6).
    pub art: GuiArtRef,
    /// The season-and-episode badge, `None` for every kind but an Emby
    /// episode with an ordinal known.
    pub episode_badge: Option<String>,
    /// The track chips the top layer paints, `None` while the local player is
    /// inactive.
    pub chips: Option<GuiTrackChips>,
    /// The open overlay the chips read, `None` while none is open.
    pub overlay: Option<GuiOverlayFrame>,
    /// Whether the transport is paused: the playhead anchor's fact (design
    /// D5).
    pub paused: bool,
    /// Which transport controls are actionable.
    pub availability: GuiTransportAvailability,
    /// Whether the core-owned auto-hide preference is on (design D5).
    pub auto_hide: bool,
    /// The control row's scale (design D3): the measured stage height's
    /// one-third share of the rows' full height, never above 1.0.
    pub scale: f32,
    /// The stage's own mounted event: the `visualSlot` region.
    pub stage_mounted: Signal<Option<Event<MountedData>>>,
    /// The control row's handles.
    pub rows: ControlRowHandles,
}

impl std::fmt::Debug for ArtStageProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtStageProps")
            .field("art", &self.art)
            .field("episode_badge", &self.episode_badge)
            .field("paused", &self.paused)
            .field("availability", &self.availability)
            .field("auto_hide", &self.auto_hide)
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

/// The stage: the visual slot's `relative` wrapper with no clip, holding the
/// clipped media box and the two absolute overlay layers. The top layer paints
/// the chips and the badge at all times; the bottom layer paints the gradient
/// and the control row.
#[component]
pub fn ArtStage(props: ArtStageProps) -> Element {
    let mut stage_mounted = props.stage_mounted;
    let scale = format!("{:.4}", props.scale);
    // The media box's content: the image, or the empty 16:9 box a loading
    // reference reserves at the panel's width (design D6).
    let media = if matches!(props.art, GuiArtRef::Image { .. }) {
        rsx! {
            ArtImage {
                art: Some(props.art.clone()),
                alt: String::new(),
                class: "block".to_owned(),
                style: "width: 100%; height: auto;".to_owned(),
                uncapped: true,
            }
        }
    } else {
        rsx! {
            div { style: "width: 100%; aspect-ratio: 16 / 9;" }
        }
    };
    rsx! {
        div {
            class: "stage relative w-full max-w-full",
            style: "background: var(--mbv-secondary);",
            "data-target": "visualSlot",
            // Each attribute is present only while its fact holds and absent
            // otherwise, never `"false"`: the stylesheet's selectors test
            // presence (design D2).
            "data-auto-hide": props.auto_hide.then_some(""),
            "data-playing": (!props.paused).then_some(""),
            onmounted: move |event| stage_mounted.set(Some(event)),
            // The clip belongs to the media box, never to the stage: the
            // overlay layers and the chip popovers paint past its edges
            // (design D1).
            div { class: "overflow-hidden w-full", {media} }
            // The top layer anchors the chips and the badge. It is a sibling
            // of the clipped media box, so a chip's popover hangs past the
            // artwork's bottom edge and, at its own above-panel stacking
            // class, paints over the panel's content and the queue rows.
            div {
                class: "absolute inset-x-0 top-0",
                style: "padding: 10px 12px;",
                ChipsAndBadge {
                    chips: props.chips.clone(),
                    overlay: props.overlay.clone(),
                    episode_badge: props.episode_badge.clone(),
                }
            }
            div {
                class: "bottom absolute inset-x-0 bottom-0",
                style: "padding-top: calc(6px * var(--player-scale)); background: rgb(0 0 0 / 0.6); --player-scale: {scale};",
                ControlRow {
                    paused: props.paused,
                    availability: props.availability,
                    rows: props.rows,
                }
            }
        }
    }
}

/// The chips-and-badge line's props.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct ChipsAndBadgeProps {
    /// The track chips, `None` while the local player is inactive.
    pub chips: Option<GuiTrackChips>,
    /// The open overlay the chips read.
    pub overlay: Option<GuiOverlayFrame>,
    /// The season-and-episode badge, `None` while the item names none.
    pub episode_badge: Option<String>,
}

/// The chips-and-badge line: the track chips at the left and the episode badge
/// at the right. The stage's top layer and the panel's no-image column paint
/// the same line (design D6).
#[component]
pub fn ChipsAndBadge(props: ChipsAndBadgeProps) -> Element {
    rsx! {
        div {
            class: "flex w-full items-start justify-between",
            style: "gap: 8px;",
            div {
                class: "min-w-0",
                TrackChips { chips: props.chips.clone(), overlay: props.overlay.clone() }
            }
            if let Some(badge) = props.episode_badge.as_ref() {
                span {
                    class: "shrink-0 font-mono",
                    style: "font-size: 11px; padding: 3px 6px; border-radius: 4px; background: var(--mbv-background); color: var(--canvas-type-count);",
                    "{badge}"
                }
            }
        }
    }
}

/// The control row's props.
#[derive(Props, Clone, PartialEq)]
pub struct ControlRowProps {
    /// Whether the transport is paused.
    pub paused: bool,
    /// Which transport controls are actionable.
    pub availability: GuiTransportAvailability,
    /// The row's region handles.
    pub rows: ControlRowHandles,
}

impl std::fmt::Debug for ControlRowProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlRowProps")
            .field("paused", &self.paused)
            .field("availability", &self.availability)
            .finish_non_exhaustive()
    }
}

/// The control row: play/pause and the scrubber, whose own clock line
/// carries the transport controls between the elapsed and `−remaining`
/// labels. The stage's bottom layer paints it over the artwork at the
/// measured `--player-scale`, and the no-image column paints it under its
/// chips line at full size (design D3, D6). The stopped state paints it
/// alone (design D7); the scrubber then paints its empty track, because the
/// playhead predicts no progress.
#[component]
pub fn ControlRow(props: ControlRowProps) -> Element {
    let mut play_pause_mounted = props.rows.play_pause;
    let mut prev_mounted = props.rows.previous;
    let mut seek_back_mounted = props.rows.seek_back;
    let mut seek_forward_mounted = props.rows.seek_forward;
    let mut next_mounted = props.rows.next;
    let seekbar_mounted = props.rows.seekbar;
    let glyph = glyph_color(props.availability.previous, GuiRole::TextPrimary);
    let next_glyph = glyph_color(props.availability.next, GuiRole::TextPrimary);
    // The transport controls ride the scrubber's clock line, between the two
    // clocks, so they sit tight under the bar.
    let transport = rsx! {
        div {
            class: "flex items-center justify-center",
            style: "gap: calc(4px * var(--player-scale));",
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: calc(30px * var(--player-scale)); height: calc(30px * var(--player-scale));",
                "data-target": "previous",
                "aria-label": "Previous",
                onmounted: move |event| prev_mounted.set(Some(event)),
                SkipBack {
                    class: "player-glyph",
                    size: 20,
                    fill: "{glyph}",
                    color: "{glyph}",
                }
            }
            button {
                "type": "button",
                class: "flex items-center justify-center font-mono",
                style: "width: calc(30px * var(--player-scale)); height: calc(30px * var(--player-scale)); font-size: calc(11px * var(--player-scale));",
                "data-target": "seekBack",
                "aria-label": "Back 10 seconds",
                title: "Back 10 s",
                onmounted: move |event| seek_back_mounted.set(Some(event)),
                "−10"
            }
            button {
                "type": "button",
                class: "flex items-center justify-center font-mono",
                style: "width: calc(30px * var(--player-scale)); height: calc(30px * var(--player-scale)); font-size: calc(11px * var(--player-scale));",
                "data-target": "seekForward",
                "aria-label": "Forward 30 seconds",
                title: "Forward 30 s",
                onmounted: move |event| seek_forward_mounted.set(Some(event)),
                "+30"
            }
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: calc(30px * var(--player-scale)); height: calc(30px * var(--player-scale));",
                "data-target": "next",
                "aria-label": "Next",
                onmounted: move |event| next_mounted.set(Some(event)),
                SkipForward {
                    class: "player-glyph",
                    size: 20,
                    fill: "{next_glyph}",
                    color: "{next_glyph}",
                }
            }
        }
    };
    rsx! {
        div {
            class: "flex items-center",
            style: "gap: calc(8px * var(--player-scale)); padding: 0 calc(8px * var(--player-scale));",
            button {
                "type": "button",
                class: "flex flex-none items-center justify-center rounded-full",
                style: "width: calc(56px * var(--player-scale)); height: calc(56px * var(--player-scale)); background: var(--mbv-accent);",
                "data-target": "playPause",
                "aria-label": if props.paused { "Play" } else { "Pause" },
                onmounted: move |event| play_pause_mounted.set(Some(event)),
                if props.paused {
                    Play {
                        class: "player-play-glyph",
                        size: 22,
                        fill: role_var(GuiRole::TextOnAccent),
                        color: role_var(GuiRole::TextOnAccent),
                    }
                } else {
                    Pause {
                        class: "player-play-glyph",
                        size: 22,
                        fill: role_var(GuiRole::TextOnAccent),
                        color: role_var(GuiRole::TextOnAccent),
                    }
                }
            }
            div {
                class: "min-w-0 flex-1",
                Scrubber { seekbar_mounted, center: transport }
            }
        }
    }
}
