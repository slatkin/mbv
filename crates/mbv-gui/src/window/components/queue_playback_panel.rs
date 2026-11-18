//! The player panel's top block (`replace-tauri-with-blitz` task 5.2, the
//! port of `QueuePlaybackPanel`'s artwork, title, and transport
//! parts): the expanded canvas artboard's header, artwork with its episode
//! badge, title and parent line, and transport. The canvas, not the TUI
//! band, is the reference; the paint uses the theme's roles and chrome fills
//! through `--mbv-<name>` properties only (design D7).
//!
//! The header's playback target reports `playbackTarget`; its press opens
//! the core's chooser, whose popover paints under the header from the
//! overlay section's `targetChooser` variant. Each row reports its
//! `targetChoice` index — the core resolves a row press against the same
//! live choice list the frame carries — under a second report key, first
//! inserted when the chooser opens: this component mounts before the queue
//! panel and footer, so its own key lands before their regions in the merged
//! report, and only a key inserted after them puts the rows last — the hit
//! map's last push wins where a row paints over a queue row beneath it. The
//! header's collapse control reports the id the core's `z` toggle runs from.
//! The canvas paints no queue-fold control in the header (`main` audit 4);
//! `q` stays a chord and the folded line owns the expand control. The window
//! holds no open, cursor, or hover state; Esc and a press elsewhere close
//! the popover through the core's key and pointer paths. The track chips row
//! and its popover live in `TrackChips` below the transport.
//!
//! The scrubber — the 6 px track, the intro and credits blocks, the played
//! fill, the chapter ticks, the thumb, the hover preview, and the clocks —
//! is this module's second file, `queue_playback_panel/scrubber.rs` (task
//! 5.3): the component paints it below the header's title block, and its
//! slider box is the `seekbar` region of this panel's one report, so the
//! panel holds the element handle the scrubber sets on mount.
//!
//! A press on the bar or a button forwards like every other pointer event:
//! the component interprets no gesture, it reports its regions and the core
//! resolves the press against the hit map (design D4).

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{
    GuiNowPlayingStatus, GuiTargetChoice, GuiTargetKey, GuiTransportAvailability,
};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::layout::{LayoutRegion, QueuePlaybackTargetId, RegionTarget};
use crate::window::brand_icons::{TargetBrand, TargetBrandIcon};
use crate::window::components::art_image::{ArtEvents, ArtImage};
use crate::window::components::row_handle::record_row_handle;
use crate::window::components::track_chips::TrackChips;
use crate::window::icons::{
    Cast, Check, ChevronDown, Maximize2, Monitor, Pause, Play, SkipBack, SkipForward,
};
use crate::window::layout::LayoutContribution;
use crate::window::layout::{LayoutEpoch, LayoutKey, LayoutReporter, element_region};
use crate::window::natural_size::NaturalSizeBounds;
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::{glyph_color, role_var};
use crate::window::truncate::{TextStyle, Truncate};

mod scrubber;

use scrubber::Scrubber;

/// The header's status word (`header_status_word`).
fn status_word(status: GuiNowPlayingStatus) -> &'static str {
    match status {
        GuiNowPlayingStatus::Playing => "PLAYING",
        GuiNowPlayingStatus::Paused => "PAUSED",
        GuiNowPlayingStatus::Idle => "IDLE",
    }
}

/// The status word's semantic colour (`status_color`).
fn status_color(status: GuiNowPlayingStatus) -> String {
    match status {
        GuiNowPlayingStatus::Playing => role_var(GuiRole::Accent),
        GuiNowPlayingStatus::Paused => role_var(GuiRole::TextFocusAccent),
        GuiNowPlayingStatus::Idle => role_var(GuiRole::TextMuted),
    }
}

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
    let chooser_key = use_hook(LayoutKey::next);

    let target_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let collapse_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let art_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let play_pause_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let prev_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let seek_back_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let seek_forward_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let next_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let seekbar_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // The open chooser's painted rows, by their choice index (the former client
    // `querySelectorAll('[data-choice]')`).
    let chooser_rows: Signal<Vec<Option<Event<MountedData>>>> = use_signal(Vec::new);

    // No region outlives the block: the contributions retract on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    // The playback target chooser: the overlay section's `targetChooser`
    // variant, `None` while another overlay (or none) is up.
    let chooser = match sections.overlay.cloned() {
        Some(GuiOverlayFrame::TargetChooser { cursor, choices }) => Some((cursor, choices)),
        _ => None,
    };

    // Each paint that moved a region reports (design D4): this effect runs
    // after the DOM update, and the layout epoch reports layout-driven moves.
    // Reading the overlay registers the chooser's rows, which appear and
    // disappear with that section; the chooser's key retracts while none is
    // open. The reporter dedupes an unchanged contribution.
    let watch_playback = sections.playback;
    let watch_overlay = sections.overlay;
    use_effect(move || {
        let playback = watch_playback.cloned();
        let overlay_open = watch_overlay.cloned();
        let _watch_epoch = epoch.0.cloned();
        // The art element's own resizes report (design D4) — the artwork's
        // loaded size moves the rows beside it. The load answers ride the art
        // store (D7), so subscribing to the painted URL's answer covers the
        // load and the swap.
        let _watch_art = art_events.status(&art_src(playback.artwork.as_ref()).unwrap_or_default());
        let chooser_open = matches!(
            overlay_open.as_ref(),
            Some(GuiOverlayFrame::TargetChooser { .. })
        );
        let rows = chooser_rows.cloned();
        let play_pause = play_pause_mounted.cloned();
        let prev = prev_mounted.cloned();
        let seek_back = seek_back_mounted.cloned();
        let seek_forward = seek_forward_mounted.cloned();
        let next = next_mounted.cloned();
        let seekbar = seekbar_mounted.cloned();
        let art = art_mounted.cloned();
        let collapse = collapse_mounted.cloned();
        let target = target_mounted.cloned();
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
                (collapse, QueuePlaybackTargetId::Collapse),
                (target, QueuePlaybackTargetId::PlaybackTarget),
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

    let playback = sections.playback.cloned();
    // Whether the transport is paused: the playhead anchor's fact (design
    // D5). `true` while no anchor holds — the idle transport paints a play
    // control.
    let paused = playhead.anchor.cloned().is_none_or(|anchor| anchor.paused);

    rsx! {
        // The player panel block on the canvas's backdrop: the header row,
        // then the still slot, the title and parent line, the
        // scrubber, and the transport (`align-gui-with-design-canvas` U3;
        // `Main.dc.html` is the reference). The canvas paints no outer pad
        // and no gutter between these blocks: the header and the still slot
        // are full-bleed, and the title, scrubber, and transport carry their
        // own 16 px insets.
        div {
            class: "flex flex-col",
            // The header row: the playback target at the left, then the
            // state label and the collapse control at the right. The target
            // button is the `playbackTarget` region; its press opens the
            // core's chooser, whose popover paints below it from the overlay
            // section's `targetChooser`. Each chooser row is its own
            // `targetChoice` region; Esc and an outside press close the
            // popover in the core. The canvas paints no queue-fold control
            // here (`main` audit 4): `q` stays a chord.
            //
            // The header is as tall as the Library top bar (56 px), so the
            // two bottom borders line up across the window; its controls
            // centre vertically.
            HeaderRow {
                host: playback.host.clone(),
                host_is_remote: playback.host_is_remote,
                status: playback.status,
                chooser: chooser.clone(),
                target_mounted,
                collapse_mounted,
                chooser_rows,
            }
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

/// The open playback-target chooser's props: the core's cursor and live
/// choice list, and the row handles the panel's chooser regions read.
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

/// The chooser's popover. Each row's
/// `onmounted` hands its event to the panel's chooser-regions signal, under
/// the row's `data-choice` index.
#[component]
fn TargetChooserPopover(props: TargetChooserPopoverProps) -> Element {
    rsx! {
        div {
            class: "absolute top-full left-0 z-20 mt-1 flex min-w-56 flex-col rounded-md border py-1 shadow-md",
            style: "background: var(--mbv-popover); border-color: var(--mbv-border);",
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

/// The header row's props: the playback target's facts, the open chooser, and
/// the region handles the header's controls set.
#[derive(Props, Clone, PartialEq)]
struct HeaderRowProps {
    /// The playback target's host label. Empty when there is none.
    host: String,
    /// Whether the playback target is remote. The flag picks the icon's
    /// colour.
    host_is_remote: bool,
    /// The now-playing status the header row shows.
    status: GuiNowPlayingStatus,
    /// The open playback-target chooser: the core's cursor and live choice
    /// list, `None` while no chooser is up.
    chooser: Option<(usize, Vec<GuiTargetChoice>)>,
    target_mounted: Signal<Option<Event<MountedData>>>,
    collapse_mounted: Signal<Option<Event<MountedData>>>,
    chooser_rows: Signal<Vec<Option<Event<MountedData>>>>,
}

impl std::fmt::Debug for HeaderRowProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderRowProps")
            .field("host", &self.host)
            .field("host_is_remote", &self.host_is_remote)
            .field("status", &self.status)
            .field("chooser", &self.chooser)
            .finish_non_exhaustive()
    }
}

/// The header row: the playback target at the
/// left, then the state label and the collapse control at the right. The
/// target button is the `playbackTarget` region; its press opens the core's
/// chooser, whose popover paints below it from the overlay section's
/// `targetChooser` variant. Each chooser row is its own `targetChoice`
/// region; Esc and an outside press close the popover in the core. The
/// canvas paints no queue-fold control here (`main` audit 4): `q` stays a
/// chord. The header is as tall as the Library top bar (56 px), so the two
/// bottom borders line up across the window; its controls centre vertically.
#[component]
fn HeaderRow(mut props: HeaderRowProps) -> Element {
    let target_color = if props.host_is_remote {
        role_var(GuiRole::PlaybackTargetRemoteFg)
    } else {
        role_var(GuiRole::PlaybackTargetLocalFg)
    };
    let status_word = status_word(props.status);
    let status_color = status_color(props.status);
    rsx! {
        div {
            class: "relative flex items-center justify-between whitespace-nowrap",
            style: "height: 56px; padding: 0 8px 0 4px; border-bottom: 1px solid var(--mbv-border);",
            button {
                "type": "button",
                class: "flex min-w-0 items-center",
                style: "height: 44px; padding: 0 10px; gap: 8px; font-size: 13px; color: {target_color};",
                "data-target": "playbackTarget",
                "aria-label": "Cast to device",
                onmounted: move |event| props.target_mounted.set(Some(event)),
                Cast { size: 18, stroke_width: 1.8, color: "currentColor" }
                if !props.host.is_empty() {
                    // The name paints as plain clipped text, never through
                    // `Truncate`: this box is sized by the text it paints, so
                    // a measured cut would shrink the box and ratchet the name
                    // down to `…` on the next session switch. The Queue
                    // header's title follows the same rule.
                    span {
                        class: "min-w-0 overflow-hidden whitespace-nowrap",
                        "{props.host}"
                    }
                }
                ChevronDown { size: 12, color: "currentColor" }
            }
            div {
                class: "flex items-center",
                style: "gap: 4px;",
                span {
                    class: "font-mono",
                    style: "font-size: 11px; letter-spacing: 0.08em; padding: 0 8px; color: {status_color};",
                    "\u{25CF} {status_word}"
                }
                button {
                    "type": "button",
                    class: "flex items-center justify-center",
                    style: "width: 44px; height: 44px;",
                    "data-target": "collapse",
                    "aria-label": "Collapse panel",
                    title: "Collapse panel (z)",
                    onmounted: move |event| props.collapse_mounted.set(Some(event)),
                    Maximize2 { size: 18, stroke_width: 1.8, color: role_var(GuiRole::TextMuted) }
                }
            }
            if let Some((cursor, choices)) = props.chooser {
                // The playback-target chooser: the core's live choice list,
                // the attached output marked with a check. The rows report
                // their `targetChoice` indices under the chooser's own key,
                // last over every region beneath them (the module comment
                // names why).
                TargetChooserPopover {
                    cursor,
                    choices,
                    chooser_rows: props.chooser_rows,
                }
            }
        }
    }
}
