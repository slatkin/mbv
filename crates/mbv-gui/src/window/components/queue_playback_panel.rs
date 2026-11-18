//! The player panel's top block (`replace-tauri-with-blitz` task 5.2, the
//! port of `QueuePlaybackPanel`'s artwork, title, and transport
//! parts): the expanded canvas artboard's artwork with its episode badge,
//! title and parent line, and transport. The canvas, not the TUI band, is the
//! reference; the paint uses the theme's roles and chrome fills through
//! `--mbv-<name>` properties only (design D7).
//!
//! The artwork is the block's topmost content: the panel paints no header row
//! above it (`move-playback-header-into-queue-footer` D6), so the column's top
//! edge under the window's global header is the still slot itself. The
//! playback target's chooser and the collapse control moved to the Queue
//! footer's band with their regions (`queue_footer.rs`); this block reports
//! neither. The canvas paints no queue-fold control here (`main` audit 4); `q`
//! stays a chord and the folded line owns the expand control. The track chips
//! row and its popover live in `TrackChips` below the transport.
//!
//! The scrubber — the 6 px track, the intro and credits blocks, the played
//! fill, the chapter ticks, the thumb, the hover preview, and the clocks —
//! is this module's second file, `queue_playback_panel/scrubber.rs` (task
//! 5.3): the component paints it below the title block, and its slider box is
//! the `seekbar` region of this panel's one report, so the panel holds the
//! element handle the scrubber sets on mount.
//!
//! A press on the bar or a button forwards like every other pointer event:
//! the component interprets no gesture, it reports its regions and the core
//! resolves the press against the hit map (design D4).

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::playback::GuiTransportAvailability;
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{LayoutRegion, QueuePlaybackTargetId, RegionTarget};
use crate::window::components::art_image::{ArtEvents, ArtImage};
use crate::window::components::track_chips::TrackChips;
use crate::window::icons::{Pause, Play, SkipBack, SkipForward};
use crate::window::layout::LayoutContribution;
use crate::window::layout::{LayoutEpoch, LayoutKey, LayoutReporter, element_region};
use crate::window::natural_size::NaturalSizeBounds;
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::{glyph_color, role_var};
use crate::window::truncate::{TextStyle, Truncate};

mod scrubber;

use scrubber::Scrubber;

/// The URL the artwork reference names, `None` while it names none (a
/// `loading` reference paints no `img`, so no art answer can land for it).
fn art_src(art: Option<&GuiArtRef>) -> Option<String> {
    match art {
        Some(GuiArtRef::Image { src } | GuiArtRef::Placeholder { src }) => Some(src.clone()),
        Some(GuiArtRef::Loading) | None => None,
    }
}

/// The player panel's top block (task 5.2). The block reads the `playback`
/// and `overlay` sections and the playhead's anchor (the paused fact); each
/// paint that moved a region reports, and the layout epoch — the
/// `ResizeObserver` the former effect watched (D9) — re-reports on every
/// layout change. The artwork's own load answers subscribe too: its loaded
/// natural size moves the rows below it, the fact the former client `ResizeObserver`
/// on the art element watched.
#[component]
pub fn QueuePlaybackPanel() -> Element {
    let sections = consume_context::<Sections>();
    let epoch = consume_context::<LayoutEpoch>();
    let reporter = consume_context::<LayoutReporter>();
    let art_events = consume_context::<ArtEvents>();
    let playhead = consume_context::<Playhead>();
    let key = use_hook(LayoutKey::next);

    let art_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let play_pause_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let prev_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let seek_back_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let seek_forward_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let next_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let seekbar_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);

    // No region outlives the block: the contribution retracts on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    // Each paint that moved a region reports (design D4): this effect runs
    // after the DOM update, and the layout epoch reports layout-driven moves.
    // The reporter dedupes an unchanged contribution.
    let watch_playback = sections.playback;
    use_effect(move || {
        let playback = watch_playback.cloned();
        let _watch_epoch = epoch.0.cloned();
        // The art element's own resizes report (design D4) — the artwork's
        // loaded size moves the rows beside it. The load answers ride the art
        // store (D7), so subscribing to the painted URL's answer covers the
        // load and the swap.
        let _watch_art = art_events.status(&art_src(playback.artwork.as_ref()).unwrap_or_default());
        let play_pause = play_pause_mounted.cloned();
        let prev = prev_mounted.cloned();
        let seek_back = seek_back_mounted.cloned();
        let seek_forward = seek_forward_mounted.cloned();
        let next = next_mounted.cloned();
        let seekbar = seekbar_mounted.cloned();
        let art = art_mounted.cloned();
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            let mut regions: Vec<LayoutRegion> = Vec::new();
            let elements = [
                (play_pause, QueuePlaybackTargetId::PlayPause),
                (prev, QueuePlaybackTargetId::Previous),
                (seek_back, QueuePlaybackTargetId::SeekBack),
                (seek_forward, QueuePlaybackTargetId::SeekForward),
                (next, QueuePlaybackTargetId::Next),
                (seekbar, QueuePlaybackTargetId::Seekbar),
                (art, QueuePlaybackTargetId::VisualSlot),
            ];
            for (mounted, id) in elements {
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
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
    });

    let playback = sections.playback.cloned();
    // Whether the transport is paused: the playhead anchor's fact (design
    // D5). `true` while no anchor holds — the idle transport paints a play
    // control.
    let paused = playhead.anchor.cloned().is_none_or(|anchor| anchor.paused);

    rsx! {
        // The player panel block on the canvas's backdrop: the still slot,
        // then the title and parent line, the scrubber, and the transport
        // (`align-gui-with-design-canvas` U3; `Main.dc.html` is the
        // reference). The canvas paints no outer pad and no gutter between
        // these blocks: the still slot is full-bleed at the column's top edge
        // (D6), and the title, scrubber, and transport carry their own 16 px
        // insets.
        div {
            class: "flex flex-col",
            if let Some(title) = playback.now_playing_title.as_ref() {
                // The still slot (`main` audit 6); `StillSlot` holds its
                // shapes.
                if let Some(art) = playback.artwork.as_ref() {
                    StillSlot {
                        art: art.clone(),
                        art_mounted,
                        still_caption: playback.still_caption.clone(),
                        episode_badge: playback.episode_badge.clone(),
                    }
                }
                // The title at the canvas's 18 px/600 and its parent line in
                // the canvas link style at 13 px (`main` audit 8). The canvas
                // draws the parent line as an `<a href="#">`, but the GUI
                // presents no surface to go to: the static line carries no
                // region and the press is a no-op.
                div {
                    class: "flex min-w-0 flex-col",
                    style: "padding: 14px 16px 6px; gap: 2px;",
                    Truncate {
                        text: title.clone(),
                        css: "font-size: 18px; font-weight: 600; color: {role_var(GuiRole::PlaybackTitleFg)};"
                            .to_owned(),
                        style: TextStyle::sans(18.0, 600.0),
                    }
                    if let Some(parent_line) = playback.parent_line.as_ref() {
                        Truncate {
                            text: parent_line.clone(),
                            css: "font-size: 13px; color: var(--canvas-type-secondary);".to_owned(),
                            style: TextStyle::sans(13.0, 400.0),
                        }
                    }
                }
                if playback.show_controls {
                    // The scrubber (`main` audits 9–12): the 60 px block with
                    // the track at 22 px, the intro and credits blocks, the
                    // played fill, the chapter ticks, the pointer's position
                    // line, the thumb, and the elapsed and `−remaining`
                    // labels at 42 px. The block's own rect is the
                    // `seekbar` region; a press resolves there in the core.
                    Scrubber { seekbar_mounted }
                    // The transport (`main` audits 13–14): previous, `−10`,
                    // the 56 px round play/pause control, `+30`, and next, on the canvas gap and insets. The
                    // spec's list carries no
                    // stop control, so the canvas panel paints none.
                    TransportRow {
                        paused,
                        availability: playback.availability,
                        prev_mounted,
                        seek_back_mounted,
                        play_pause_mounted,
                        seek_forward_mounted,
                        next_mounted,
                    }
                    // The track chips row under the transport: the video
                    // label and the audio and subtitle chips with the open
                    // popover anchored below its chip, right-aligned.
                    // `trackChips` is `null` while the local player is
                    // inactive, so the row is absent then, as the
                    // status-indicator cluster is.
                    TrackChips {
                        chips: playback.track_chips.clone(),
                        overlay: sections.overlay.cloned(),
                    }
                }
            }
        }
    }
}

/// The transport row's props: the playhead's paused fact, the availability
/// the glyph colours gate on, and the five region handles the buttons set.
#[derive(Props, Clone, Copy, PartialEq)]
struct TransportRowProps {
    /// Whether the transport is paused: the playhead anchor's fact (D5).
    paused: bool,
    /// Which transport controls are actionable.
    availability: GuiTransportAvailability,
    prev_mounted: Signal<Option<Event<MountedData>>>,
    seek_back_mounted: Signal<Option<Event<MountedData>>>,
    play_pause_mounted: Signal<Option<Event<MountedData>>>,
    seek_forward_mounted: Signal<Option<Event<MountedData>>>,
    next_mounted: Signal<Option<Event<MountedData>>>,
}

impl std::fmt::Debug for TransportRowProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportRowProps")
            .field("paused", &self.paused)
            .field("availability", &self.availability)
            .finish_non_exhaustive()
    }
}

/// The transport row: previous, `−10`, the 56 px round play/pause control,
/// `+30`, and next, on the canvas gap and insets.
#[component]
fn TransportRow(mut props: TransportRowProps) -> Element {
    let glyph = glyph_color(props.availability.previous, GuiRole::TextPrimary);
    let next_glyph = glyph_color(props.availability.next, GuiRole::TextPrimary);
    rsx! {
        div {
            class: "flex items-center justify-center",
            style: "gap: 4px; padding: 0 16px 6px;",
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: 44px; height: 44px;",
                "data-target": "previous",
                "aria-label": "Previous",
                onmounted: move |event| props.prev_mounted.set(Some(event)),
                SkipBack { size: 20, fill: "{glyph}", color: "{glyph}" }
            }
            button {
                "type": "button",
                class: "flex items-center justify-center font-mono",
                style: "width: 44px; height: 44px; font-size: 11px;",
                "data-target": "seekBack",
                "aria-label": "Back 10 seconds",
                title: "Back 10 s",
                onmounted: move |event| props.seek_back_mounted.set(Some(event)),
                "−10"
            }
            button {
                "type": "button",
                class: "flex items-center justify-center rounded-full",
                style: "width: 56px; height: 56px; background: var(--mbv-accent);",
                "data-target": "playPause",
                "aria-label": if props.paused { "Play" } else { "Pause" },
                onmounted: move |event| props.play_pause_mounted.set(Some(event)),
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
            button {
                "type": "button",
                class: "flex items-center justify-center font-mono",
                style: "width: 44px; height: 44px; font-size: 11px;",
                "data-target": "seekForward",
                "aria-label": "Forward 30 seconds",
                title: "Forward 30 s",
                onmounted: move |event| props.seek_forward_mounted.set(Some(event)),
                "+30"
            }
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: 44px; height: 44px;",
                "data-target": "next",
                "aria-label": "Next",
                onmounted: move |event| props.next_mounted.set(Some(event)),
                SkipForward { size: 20, fill: "{next_glyph}", color: "{next_glyph}" }
            }
        }
    }
}

/// The still slot's props: the artwork the slot paints and the facts around
/// it, plus the visual-slot region handle.
#[derive(Props, Clone, PartialEq)]
struct StillSlotProps {
    /// The artwork reference the playback section carries.
    art: GuiArtRef,
    art_mounted: Signal<Option<Event<MountedData>>>,
    /// The still slot's caption while no image lands.
    still_caption: Option<String>,
    /// The season-and-episode badge, `None` for every kind but an Emby
    /// episode with an ordinal known.
    episode_badge: Option<String>,
}

impl std::fmt::Debug for StillSlotProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StillSlotProps")
            .field("art", &self.art)
            .field("still_caption", &self.still_caption)
            .field("episode_badge", &self.episode_badge)
            .finish_non_exhaustive()
    }
}

/// The still slot: the
/// artwork on the raised control fill, and the canvas caption while no image
/// lands. A loaded image spans the panel's full width (the TUI's visual slot
/// at full column width) with no natural-size cap, and its height follows
/// the artwork's aspect, so the rows below it move with the image. The
/// placeholder keeps the canvas's fixed 225 px box and hugs its image. The
/// wrapper is the visual slot's painted region. The season-and-episode badge
/// is the canvas chip at the slot's bottom left (`main` audit 7); a
/// non-episode item paints none.
#[component]
fn StillSlot(mut props: StillSlotProps) -> Element {
    let image = matches!(props.art, GuiArtRef::Image { .. });
    let (width_class, slot_style, image_style, bounds) = if image {
        (
            "w-full",
            "background: var(--mbv-secondary);",
            "width: 100%; height: auto;",
            None,
        )
    } else {
        (
            "w-fit",
            "height: 225px; background: var(--mbv-secondary);",
            "",
            Some(NaturalSizeBounds {
                width: None,
                height: Some(225.0),
            }),
        )
    };
    rsx! {
        div {
            class: "relative flex {width_class} max-w-full items-center justify-center overflow-hidden",
            style: "{slot_style}",
            "data-target": "visualSlot",
            onmounted: move |event| props.art_mounted.set(Some(event)),
            ArtImage {
                art: Some(props.art.clone()),
                alt: String::new(),
                class: "block".to_owned(),
                style: image_style.to_owned(),
                bounds,
                uncapped: image,
                fallback: rsx! {
                    // The canvas's source caption while no image lands
                    // (design D4a's still slot): the frame's still-caption
                    // fact, one `[...]` line per item kind; an item with no
                    // image source paints no caption.
                    if let Some(caption) = props.still_caption.as_ref() {
                        span {
                            class: "font-mono",
                            style: "font-size: 12px; color: {role_var(GuiRole::TextMuted)};",
                            "[{caption}]"
                        }
                    }
                },
            }
            if let Some(badge) = props.episode_badge.as_ref() {
                span {
                    class: "absolute font-mono",
                    style: "left: 12px; bottom: 10px; font-size: 11px; padding: 3px 6px; border-radius: 4px; background: var(--mbv-background); color: var(--canvas-type-count);",
                    "{badge}"
                }
            }
        }
    }
}
