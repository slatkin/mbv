//! The player panel's top block (`replace-tauri-with-blitz` task 5.2, the
//! port of `QueuePlaybackPanel`'s artwork, title, and transport parts): the
//! expanded canvas artboard's stage with its overlays, the title block with
//! its media-kind icon, and the no-image column. The canvas, not the TUI
//! band, is the reference; the paint uses the theme's roles and chrome fills
//! through `--mbv-<name>` properties only (design D7), but for the two
//! media-kind icons: their accent sand is a fixed paint no role names.
//!
//! The stage is the block's topmost content: the panel paints no header row
//! above it (`move-playback-header-into-queue-footer` D6), so the column's top
//! edge under the window's global header is the stage itself. The playback
//! target's chooser and the collapse control live in the Queue footer's band
//! with their regions (`queue_footer.rs`); this block reports neither. The
//! canvas paints no queue-fold control here (`main` audit 4); `q` stays a
//! chord and the folded line owns the expand control.
//!
//! The stage, its two overlay layers, and the control row are this module's
//! second file, `queue_playback_panel/art_stage.rs` (`gui-art-player-overlay`
//! design D1, D8): the stage is the visual slot's wrapper, the media box owns
//! the clip, the top layer holds the track chips and the episode badge, and
//! the bottom layer holds the solid scrim and the control row. The panel owns
//! every mark of its one region set and reports them in one contribution.
//!
//! The scrubber — the track, the played fill, the thumb, and the time tip —
//! is this module's third file, `queue_playback_panel/scrubber.rs` (task
//! 5.3; `gui-queue-panel-redesign` design D4): the component paints it as
//! the control rows' seek row, and that row is the `seekbar` region of this
//! panel's one report, so the panel marks it and hands the closure down.
//!
//! A press on the bar or a button forwards like every other pointer event:
//! the component interprets no gesture, it reports its regions and the core
//! resolves the press against the hit map (design D4).

use dioxus_native::prelude::*;

use mbv::NowPlayingMediaKind;

use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::{GuiOverlayFrame, GuiRowMenuEntry};
use crate::bridge::layout::{QueuePlaybackTargetId, RegionTarget};
use crate::window::components::art_image::ArtEvents;
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::icons::{EighthNotes, MonitorPlay};
use crate::window::input::f32_from_f64;
use crate::window::layout::LayoutEpoch;
use crate::window::layout::region_set::{MarkHandler, use_region_set};
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::theme::{EfToken, ef_var};
use crate::window::truncate::{TextStyle, Truncate};

mod art_stage;
mod scrubber;

use art_stage::{ArtStage, ChipsAndBadge, ControlRow, ControlRowHandles, player_scale};

/// The title block's media-kind icon size, in CSS pixels (user request
/// 2026-11-20): a large glyph against the block's 18 px title.
const KIND_ICON_SIZE: u16 = 28;
/// The media-kind icon's colour: the panel accent's sand, the user's named
/// hex for both kind icons (2026-11-20). No theme role names it.
const KIND_ICON_COLOR: &str = "#DABC7F";

/// The title block's leading icon (user request 2026-11-20): the eighth-notes
/// pair for audio, the playing monitor for video, both in the accent sand.
fn kind_icon(kind: NowPlayingMediaKind) -> Element {
    let audio = kind == NowPlayingMediaKind::Audio;
    rsx! {
        if audio {
            EighthNotes {
                size: KIND_ICON_SIZE,
                color: KIND_ICON_COLOR.to_owned(),
            }
        } else {
            MonitorPlay {
                size: KIND_ICON_SIZE,
                color: KIND_ICON_COLOR.to_owned(),
            }
        }
    }
}

/// The artwork the stage paints: `Some` for the two states that carry a media
/// box, `None` for the states that paint the no-image column (design D6).
fn stage_art(art: Option<&GuiArtRef>) -> Option<GuiArtRef> {
    match art {
        Some(art @ (GuiArtRef::Image { .. } | GuiArtRef::Loading)) => Some(art.clone()),
        Some(GuiArtRef::Placeholder { .. }) | None => None,
    }
}

/// The control row's scale (`gui-art-player-overlay` design D3, D4): the
/// stage's measured height through [`player_scale`], re-read on the stage's
/// mount and on every layout move, written only when the value moves. The
/// panel keeps the stage's mounted event for this measure alone: the region
/// set measures the stage for its `visualSlot` region and holds no scale, so
/// the height the scale divides comes from the panel's own handle, as
/// `track_width` measures the box it sizes.
fn track_stage_scale(mounted: Signal<Option<Event<MountedData>>>, mut scale: Signal<f32>) {
    let epoch = consume_context::<LayoutEpoch>();
    use_effect(move || {
        // Reading the epoch re-measures the stage on every layout move.
        let _epoch = epoch.0.cloned();
        let mounted = mounted.cloned();
        spawn(async move {
            let Some(mounted) = mounted else {
                return;
            };
            let Ok(rect) = mounted.get_client_rect().await else {
                return;
            };
            // A stage with no laid-out height carries no aspect to measure: the
            // art has not decoded yet, or its load failed and the media box is
            // empty. The last scale stands then, so the controls do not
            // collapse into a box that is not there. The formula itself has no
            // floor (design D3).
            let stage_height = f32_from_f64(rect.height());
            if stage_height <= 0.0 {
                return;
            }
            let measured = player_scale(stage_height);
            if scale.peek().total_cmp(&measured).is_ne() {
                scale.set(measured);
            }
        });
    });
}

/// The player panel's top block (task 5.2). The block reads the `playback`
/// and `overlay` sections and the playhead's anchor (the paused fact); the
/// region set's one effect reports the marks its own render makes, and the
/// layout epoch — the `ResizeObserver` the former effect watched (D9) —
/// re-reports on every layout change. The artwork's own load answers subscribe
/// too: its loaded natural size moves the rows below it, the fact the former
/// client `ResizeObserver` on the art element watched.
#[component]
pub fn QueuePlaybackPanel() -> Element {
    let sections = consume_context::<Sections>();
    let art_events = consume_context::<ArtEvents>();
    let playhead = consume_context::<Playhead>();

    // The control row's scale (design D3): layout, not interaction state. The
    // panel measures the stage and writes it; the stage paints it as
    // `--player-scale`, so the rows take at most a third of the art. Its
    // measure needs the stage's own mounted event, kept here beside the set.
    let stage_scale: Signal<f32> = use_signal(|| 1.0);
    let stage_handle: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    track_stage_scale(stage_handle, stage_scale);

    // Each paint that moved a region reports (design D1): the region set's one
    // effect watches the `playback` and `overlay` sections, the artwork's own
    // load answers, and the layout epoch via the set's own read. The stage
    // comes first, so a press on a control over the artwork lands on the
    // control, not on the slot beneath it. The one overlay is the player
    // menu's entries.
    let watch_playback = sections.playback;
    let watch_overlay = sections.overlay;
    let region_set = use_region_set(1, move || {
        let playback = watch_playback.cloned();
        // The player menu opens and closes with the overlay section (the
        // menu's overlay contribution retracts while none is open), so a batch
        // that carries it re-reports.
        let _overlay = watch_overlay.cloned();
        // The art element's own resizes report (design D4) — the artwork's
        // loaded size moves the rows beside it. The load answers ride the art
        // store (D7), so subscribing to the painted URL's answer covers the
        // load and the swap.
        let src = playback.artwork.as_ref().and_then(GuiArtRef::src);
        let _art = art_events.status(src.unwrap_or_default());
    });

    let playback = sections.playback.cloned();
    // Whether the transport is paused: the playhead anchor's fact (design
    // D5). `true` while no anchor holds — the idle transport paints a play
    // control.
    let paused = playhead.anchor.cloned().is_none_or(|anchor| anchor.paused);
    let title = playback.now_playing_title.as_ref();
    let stage = stage_art(playback.artwork.as_ref());
    // The block paints no control row when neither a title block nor a
    // startable transport paints.
    let controls_paint = title.is_some() || playback.controls.can_start;
    // The marks, in the report's order (design D1): the stage first, then the
    // transport controls and the seekbar, in the order the former effect
    // pushed them. Only a painted element is marked, so a block that paints
    // no control row and no stage reports neither.
    let mut stage_handle = stage_handle;
    let stage_mounted = if title.is_some() && stage.is_some() {
        let mark = region_set.mark(RegionTarget::QueuePlayback(
            QueuePlaybackTargetId::VisualSlot,
        ));
        MarkHandler::new(move |event: Event<MountedData>| {
            stage_handle.set(Some(event.clone()));
            mark.call(event);
        })
    } else {
        MarkHandler::default()
    };
    let rows = if controls_paint {
        ControlRowHandles {
            play_pause: region_set.mark(RegionTarget::QueuePlayback(
                QueuePlaybackTargetId::PlayPause,
            )),
            previous: region_set.mark(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Previous)),
            seek_back: region_set
                .mark(RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack)),
            seek_forward: region_set.mark(RegionTarget::QueuePlayback(
                QueuePlaybackTargetId::SeekForward,
            )),
            next: region_set.mark(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next)),
            seekbar: region_set.mark(RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar)),
        }
    } else {
        ControlRowHandles::default()
    };
    // The two arrangements (design D6): the stage for a media box, and the
    // no-image column — the chips-and-badge line, then the control row —
    // for `Placeholder` and `None`. The no-image column carries no stage, so
    // no auto-hide attribute applies there (design D2).
    let art_section = if let Some(art) = stage {
        rsx! {
            ArtStage {
                art,
                episode_badge: playback.episode_badge.clone(),
                chips: playback.track_chips.clone(),
                overlay: sections.overlay.cloned(),
                paused,
                availability: playback.availability,
                auto_hide: playback.controls.auto_hide,
                scale: stage_scale(),
                stage_mounted,
                rows,
            }
        }
    } else {
        rsx! {
            div {
                class: "flex flex-col",
                style: "padding: 10px 0 0;",
                div {
                    style: "padding: 0 16px;",
                    ChipsAndBadge {
                        chips: playback.track_chips.clone(),
                        overlay: sections.overlay.cloned(),
                        episode_badge: playback.episode_badge.clone(),
                    }
                }
                ControlRow {
                    paused,
                    availability: playback.availability,
                    rows,
                }
            }
        }
    };

    // The open player menu (gui-player-overlay-menu): the core's cursor, the
    // right-click's window anchor, and the entry the playback section's
    // checked fact marks. The card paints in a fixed wrapper at the window's
    // origin, as the Library's right-click menu does, so the anchor addresses
    // the window, not the panel's own box. Its presses resolve through the
    // hit map: the panel marks the entry in its overlay contribution, and the
    // core runs the entry's execution and closes the menu with it.
    let player_menu = match sections.overlay.cloned().as_ref() {
        Some(GuiOverlayFrame::PlayerMenu { cursor, x, y }) => Some((*cursor, *x, *y)),
        _ => None,
    };

    rsx! {
        // The player panel block on the canvas's backdrop: the stage or the
        // no-image column, then the title and parent line
        // (`align-gui-with-design-canvas` U3; `Main.dc.html` is the
        // reference). The canvas paints no outer pad and no gutter between
        // these blocks: the stage is full-bleed at the column's top edge
        // (D6), and the title carries its own 16 px insets.
        // The no-image column and the stopped state paint their rows at full
        // size (design D6, D7): the panel's own `--player-scale` is 1.0, and
        // only the stage's bottom layer overrides it with the measured value.
        div {
            class: "flex flex-col",
            style: "--player-scale: 1;",
            if let Some(title) = title {
                {art_section}
                // The title block of the canvas's layering (design D2): the
                // panel fill, its 9/20/11 insets (user request 2026-11-24,
                // tightened from the canvas's 16/20/18 so the block sits
                // shorter), the rule that separates it
                // from the queue header, the title in the mono face's 18 px/700
                // caps, and the parent line under it in the mono face's 14 px/400
                // (user request 2026-11-24). The title is uppercased here, not by
                // `text-transform`, so the `Truncate` measure reads the glyphs it
                // paints. The canvas draws the parent line as an `<a href="#">`,
                // but the GUI presents no surface to go to: the static line
                // carries no region and the press is a no-op. The block's
                // leading edge carries the item's media-kind icon (user request
                // 2026-11-20): the eighth-notes pair for audio, the playing
                // monitor for video, both in the accent sand.
                div {
                    class: "flex items-center",
                    style: format!(
                        "padding: 9px 20px 11px; gap: 10px; background: {}; border-bottom: 1px solid {};",
                        ef_var(EfToken::Bg0),
                        ef_var(EfToken::Bg3)
                    ),
                    {kind_icon(playback.now_playing_media)}
                    div {
                        class: "flex min-w-0 flex-1 flex-col",
                        style: "gap: 0;",
                        // The title wraps rather than truncating (user request
                        // 2026-11-24): the block is one row of the column, so a
                        // long name takes the lines it needs, and a long token
                        // breaks rather than overflowing the column.
                        span {
                            style: format!(
                                "font-family: var(--font-mono); font-size: 18px; font-weight: 700; line-height: 1.15; letter-spacing: 0.06em; text-transform: uppercase; overflow-wrap: break-word; color: {};",
                                ef_var(EfToken::Fg)
                            ),
                            "{title}"
                        }
                        if let Some(parent_line) = playback.parent_line.as_ref() {
                            Truncate {
                                text: parent_line.clone(),
                                css: format!(
                                    "font-family: var(--font-mono); font-size: 14px; font-weight: 400; line-height: 1.35; color: {};",
                                    ef_var(EfToken::Red)
                                ),
                                style: TextStyle::mono(14.0, 400.0),
                            }
                        }
                    }
                }
            } else if playback.controls.can_start {
                // The stopped state (design D7): the control row alone,
                // with no stage, no chips line, and no title. The scrubber
                // paints its empty track, because the playhead predicts no
                // progress.
                ControlRow {
                    paused,
                    availability: playback.availability,
                    rows,
                }
            }
            if let Some((cursor, x, y)) = player_menu {
                {
                    let top = format!("{y}px");
                    let left = format!("{x}px");
                    let entries = vec![GuiRowMenuEntry {
                        label: "Auto-hide controls".to_owned(),
                        key_hint: Some("Enter".to_owned()),
                        destructive: false,
                        index: None,
                        checked: Some(playback.controls.auto_hide),
                    }];
                    // The player menu's entries mark the set's one overlay, one
                    // per painted row in row order (design D1): the entry's
                    // region merges after the panel's and the queue's regions
                    // and wins the hit map where the card paints over them,
                    // and a closed menu marks none, so the overlay retracts.
                    let row_marks = entries
                        .iter()
                        .map(|_| {
                            region_set.mark_over(
                                0,
                                RegionTarget::QueuePlayback(
                                    QueuePlaybackTargetId::PlayerMenuEntry,
                                ),
                            )
                        })
                        .collect::<Vec<_>>();
                    rsx! {
                        div {
                            class: "fixed",
                            style: "left: 0; top: 0; width: 0; height: 0;",
                            QueueRowMenu {
                                cursor: Some(cursor),
                                entries,
                                top,
                                left: Some(left),
                                label: "Player panel".to_owned(),
                                row_marks,
                            }
                        }
                    }
                }
            }
        }
    }
}
