//! The player stage and its overlay layers (`gui-art-player-overlay` design
//! D1, D6, D8): the stage wrapper, the clipped media box, the top layer with
//! the track chips and the episode badge, and the bottom layer with its
//! gradient and the two control rows.
//!
//! The stage is the visual slot: a `relative` wrapper with no clip that
//! carries the `visualSlot` region, so the panel reports one region for the
//! whole box. The media box is the stage's clipped sibling, so a chip popover
//! that hangs past the artwork is never cut. The top and bottom layers are
//! absolute siblings over the media box; a press on a control lands on the
//! control's own region, which the panel pushes after the stage's (design D4).
//!
//! The control rows are one component ([`ControlRows`]) because two
//! arrangements paint them: the stage's bottom layer, and the no-image column
//! the panel paints for `Placeholder` and `None` artwork, where no stage
//! exists (design D6, D7). [`ChipsAndBadge`] is shared the same way: the top
//! layer holds it, and the no-image column paints it as its own line.
//!
//! The sizes are the canvas's pixels. The measured `--player-scale` of design
//! D3 is not applied here yet (task 4.2).

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{GuiTrackChips, GuiTransportAvailability};
use crate::bridge::frame::theme::GuiRole;
use crate::window::components::art_image::ArtImage;
use crate::window::components::checkbox_box::{CheckboxBox, CheckboxBoxElement};
use crate::window::components::track_chips::TrackChips;
use crate::window::icons::{Pause, Play, SkipBack, SkipForward};
use crate::window::theme::{glyph_color, role_var};

use super::scrubber::Scrubber;

/// The mounted handles the control rows set: the panel holds every handle and
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
    /// The auto-hide checkbox.
    pub auto_hide: Signal<Option<Event<MountedData>>>,
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
    /// The stage's own mounted event: the `visualSlot` region.
    pub stage_mounted: Signal<Option<Event<MountedData>>>,
    /// The two control rows' handles.
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
            .finish_non_exhaustive()
    }
}

/// The stage: the visual slot's `relative` wrapper with no clip, holding the
/// clipped media box and the two absolute overlay layers. The top layer paints
/// the chips and the badge at all times; the bottom layer paints the gradient
/// and the two control rows.
#[component]
pub fn ArtStage(props: ArtStageProps) -> Element {
    let mut stage_mounted = props.stage_mounted;
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
            class: "relative w-full max-w-full",
            style: "background: var(--mbv-secondary);",
            "data-target": "visualSlot",
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
                class: "absolute inset-x-0 bottom-0",
                style: "padding-top: 28px; background: linear-gradient(to top, rgb(0 0 0 / 0.78), rgb(0 0 0 / 0));",
                ControlRows {
                    paused: props.paused,
                    availability: props.availability,
                    auto_hide: props.auto_hide,
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

/// The control rows' props.
#[derive(Props, Clone, PartialEq)]
pub struct ControlRowsProps {
    /// Whether the transport is paused.
    pub paused: bool,
    /// Which transport controls are actionable.
    pub availability: GuiTransportAvailability,
    /// Whether the auto-hide preference is on: the checkbox paints its mark.
    pub auto_hide: bool,
    /// The rows' region handles.
    pub rows: ControlRowHandles,
}

impl std::fmt::Debug for ControlRowsProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlRowsProps")
            .field("paused", &self.paused)
            .field("availability", &self.availability)
            .field("auto_hide", &self.auto_hide)
            .finish_non_exhaustive()
    }
}

/// The two control rows: row 1 holds play/pause and the scrubber, whose own
/// clocks paint the elapsed and `−remaining` labels; row 2 holds previous,
/// back 10 s, forward 30 s, next, and the auto-hide checkbox.
///
/// The stage's bottom layer paints them over the artwork, and the no-image
/// column paints them under its chips line at the same sizes (design D6). The
/// stopped state paints them alone (design D7); the scrubber then paints its
/// empty track, because the playhead predicts no progress.
#[component]
pub fn ControlRows(props: ControlRowsProps) -> Element {
    let mut play_pause_mounted = props.rows.play_pause;
    let mut prev_mounted = props.rows.previous;
    let mut seek_back_mounted = props.rows.seek_back;
    let mut seek_forward_mounted = props.rows.seek_forward;
    let mut next_mounted = props.rows.next;
    let mut auto_hide_mounted = props.rows.auto_hide;
    let seekbar_mounted = props.rows.seekbar;
    let glyph = glyph_color(props.availability.previous, GuiRole::TextPrimary);
    let next_glyph = glyph_color(props.availability.next, GuiRole::TextPrimary);
    let mark_style = format!(
        "width: 16px; height: 16px; box-sizing: border-box; border-radius: 4px; border: 1.5px solid {}; background: {};",
        if props.auto_hide {
            "var(--canvas-accent)"
        } else {
            "var(--canvas-ordinal)"
        },
        if props.auto_hide {
            "var(--canvas-accent)"
        } else {
            "transparent"
        },
    );
    rsx! {
        div {
            class: "flex items-center",
            style: "gap: 8px; padding: 0 8px;",
            button {
                "type": "button",
                class: "flex flex-none items-center justify-center rounded-full",
                style: "width: 56px; height: 56px; background: var(--mbv-accent);",
                "data-target": "playPause",
                "aria-label": if props.paused { "Play" } else { "Pause" },
                onmounted: move |event| play_pause_mounted.set(Some(event)),
                if props.paused {
                    Play {
                        size: 22,
                        fill: role_var(GuiRole::TextOnAccent),
                        color: role_var(GuiRole::TextOnAccent),
                    }
                } else {
                    Pause {
                        size: 22,
                        fill: role_var(GuiRole::TextOnAccent),
                        color: role_var(GuiRole::TextOnAccent),
                    }
                }
            }
            div {
                class: "min-w-0 flex-1",
                Scrubber { seekbar_mounted }
            }
        }
        div {
            class: "flex items-center justify-center",
            style: "gap: 4px; padding: 0 16px 6px;",
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: 44px; height: 44px;",
                "data-target": "previous",
                "aria-label": "Previous",
                onmounted: move |event| prev_mounted.set(Some(event)),
                SkipBack { size: 20, fill: "{glyph}", color: "{glyph}" }
            }
            button {
                "type": "button",
                class: "flex items-center justify-center font-mono",
                style: "width: 44px; height: 44px; font-size: 11px;",
                "data-target": "seekBack",
                "aria-label": "Back 10 seconds",
                title: "Back 10 s",
                onmounted: move |event| seek_back_mounted.set(Some(event)),
                "−10"
            }
            button {
                "type": "button",
                class: "flex items-center justify-center font-mono",
                style: "width: 44px; height: 44px; font-size: 11px;",
                "data-target": "seekForward",
                "aria-label": "Forward 30 seconds",
                title: "Forward 30 s",
                onmounted: move |event| seek_forward_mounted.set(Some(event)),
                "+30"
            }
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: 44px; height: 44px;",
                "data-target": "next",
                "aria-label": "Next",
                onmounted: move |event| next_mounted.set(Some(event)),
                SkipForward { size: 20, fill: "{next_glyph}", color: "{next_glyph}" }
            }
            // The auto-hide control: a checkbox with a tooltip and no visible
            // label (design D5). Its press toggles the core-owned preference
            // through the `autoHide` region; the box paints the setting.
            button {
                "type": "button",
                role: "checkbox",
                "aria-checked": "{props.auto_hide}",
                "aria-label": "Auto-hide controls",
                title: "Auto-hide controls",
                class: "flex items-center justify-center",
                style: "width: 44px; height: 44px;",
                "data-target": "autoHide",
                onmounted: move |event| auto_hide_mounted.set(Some(event)),
                CheckboxBox {
                    checked: props.auto_hide,
                    element: CheckboxBoxElement::Span,
                    style: mark_style,
                    size: 11,
                    stroke_width: 3.5,
                    color: "var(--canvas-ink)".to_owned(),
                    aria_hidden: true,
                }
            }
        }
    }
}
