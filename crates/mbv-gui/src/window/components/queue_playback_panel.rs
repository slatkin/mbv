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
//! the bottom layer holds the solid scrim and the control row. The panel
//! keeps every mounted handle and reports the regions in one contribution.
//!
//! The scrubber — the track, the played fill, the thumb, and the time tip —
//! is this module's third file, `queue_playback_panel/scrubber.rs` (task
//! 5.3; `gui-queue-panel-redesign` design D4): the component paints it as
//! the control rows' seek row, and that row is the `seekbar` region of this
//! panel's one report, so the panel holds the element handle the scrubber
//! sets on mount.
//!
//! A press on the bar or a button forwards like every other pointer event:
//! the component interprets no gesture, it reports its regions and the core
//! resolves the press against the hit map (design D4).

use dioxus_native::prelude::*;

use mbv::NowPlayingMediaKind;

use crate::art::GuiArtRef;
use crate::bridge::frame::overlay::{GuiOverlayFrame, GuiRowMenuEntry};
use crate::bridge::layout::{LayoutRegion, QueuePlaybackTargetId, RegionTarget};
use crate::window::components::art_image::ArtEvents;
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::icons::{EighthNotes, MonitorPlay};
use crate::window::layout::LayoutContribution;
use crate::window::layout::{LayoutEpoch, LayoutKey, LayoutReporter, element_region};
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

/// The URL the artwork reference names, `None` while it names none (a
/// `loading` reference paints no `img`, so no art answer can land for it).
fn art_src(art: Option<&GuiArtRef>) -> Option<String> {
    match art {
        Some(GuiArtRef::Image { src } | GuiArtRef::Placeholder { src }) => Some(src.clone()),
        Some(GuiArtRef::Loading) | None => None,
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

/// The stage's region, with the control row's scale updated from its measured
/// height (`gui-art-player-overlay` design D3, D4): the stage leads the
/// panel's report, and its height decides how much of itself the two control
/// rows may take.
async fn stage_region(
    mounted: &Event<MountedData>,
    mut scale: Signal<f32>,
) -> Option<LayoutRegion> {
    let region = element_region(
        mounted,
        RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
    )
    .await?;
    // A stage with no laid-out height carries no aspect to measure: the art
    // has not decoded yet, or its load failed and the media box is empty. The
    // last scale stands then, so the controls do not collapse into a box that
    // is not there. The formula itself has no floor (design D3).
    if region.rect.height > 0.0 {
        let measured = player_scale(region.rect.height);
        if scale.peek().total_cmp(&measured).is_ne() {
            scale.set(measured);
        }
    }
    Some(region)
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
    // The player menu's entry rows (gui-player-overlay-menu): the handles the
    // menu's `row_slot` prop records, reported under the menu's own key while
    // it is open.
    let menu_row_handles: Signal<Vec<Option<Event<MountedData>>>> = use_signal(Vec::new);
    let menu_key = use_hook(LayoutKey::next);
    // The control row's scale (design D3): layout, not interaction state. The
    // region effect measures the stage and writes it; the stage paints it as
    // `--player-scale`, so the rows take at most a third of the art.
    let stage_scale: Signal<f32> = use_signal(|| 1.0);

    // No region outlives the block: the contribution retracts on unmount, as
    // the former client `onMount` teardown did.
    let mut drop_reporter = reporter.clone();

    // Each paint that moved a region reports (design D4): this effect runs
    // after the DOM update, and the layout epoch reports layout-driven moves.
    // The reporter dedupes an unchanged contribution. The stage comes first,
    // so a press on a control over the artwork lands on the control, not on
    // the slot beneath it.
    let watch_playback = sections.playback;
    let watch_overlay = sections.overlay;
    use_effect(move || {
        let playback = watch_playback.cloned();
        // The player menu opens and closes with the overlay section (the
        // menu's key retracts while none is open), so a batch that carries it
        // re-reports.
        let overlay = watch_overlay.cloned();
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
        let scale_signal = stage_scale;
        let menu_rows = menu_row_handles;
        let mut effect_reporter = reporter.clone();
        spawn(async move {
            let mut regions: Vec<LayoutRegion> = Vec::new();
            // The stage leads the report (design D4), and its measured height
            // is the scale's source (design D3): the rows take at most a
            // third of it.
            if let Some(mounted) = art
                && let Some(region) = stage_region(&mounted, scale_signal).await
            {
                regions.push(region);
            }
            let elements = [
                (play_pause, QueuePlaybackTargetId::PlayPause),
                (prev, QueuePlaybackTargetId::Previous),
                (seek_back, QueuePlaybackTargetId::SeekBack),
                (seek_forward, QueuePlaybackTargetId::SeekForward),
                (next, QueuePlaybackTargetId::Next),
                (seekbar, QueuePlaybackTargetId::Seekbar),
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
            // The player menu's entry reports under its own key, first
            // inserted while the menu is open, so it merges after the panel's
            // and the queue's regions and wins the hit map where the card
            // paints over them (the queue row menu's second-key technique).
            // A shrinking row list drops the stale handles (the track chips'
            // idiom).
            if matches!(overlay.as_ref(), Some(GuiOverlayFrame::PlayerMenu { .. })) {
                let mut menu_regions: Vec<LayoutRegion> = Vec::new();
                for mounted in menu_rows.peek().iter().flatten() {
                    menu_regions.extend(
                        element_region(
                            mounted,
                            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayerMenuEntry),
                        )
                        .await,
                    );
                }
                effect_reporter.report_layout(
                    menu_key,
                    LayoutContribution {
                        lists: Vec::new(),
                        regions: menu_regions,
                    },
                );
            } else {
                effect_reporter.retract_layout(menu_key);
            }
        });
    });

    use_drop(move || {
        drop_reporter.retract_layout(key);
        drop_reporter.retract_layout(menu_key);
    });

    let playback = sections.playback.cloned();
    // Whether the transport is paused: the playhead anchor's fact (design
    // D5). `true` while no anchor holds — the idle transport paints a play
    // control.
    let paused = playhead.anchor.cloned().is_none_or(|anchor| anchor.paused);
    let rows = ControlRowHandles {
        play_pause: play_pause_mounted,
        previous: prev_mounted,
        seek_back: seek_back_mounted,
        seek_forward: seek_forward_mounted,
        next: next_mounted,
        seekbar: seekbar_mounted,
    };
    // The two arrangements (design D6): the stage for a media box, and the
    // no-image column — the chips-and-badge line, then the control row —
    // for `Placeholder` and `None`. The no-image column carries no stage, so
    // no auto-hide attribute applies there (design D2).
    let art_section = if let Some(art) = stage_art(playback.artwork.as_ref()) {
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
                stage_mounted: art_mounted,
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
    // hit map: the panel reports the entry's region under the menu key, and
    // the core runs the entry's execution and closes the menu with it.
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
            if let Some(title) = playback.now_playing_title.as_ref() {
                {art_section}
                // The title block of the canvas's layering (design D2): the
                // panel fill, its 16/20/18 insets, the rule that separates it
                // from the queue header, the title in the canvas's 18 px/600
                // foreground, and the parent line in the canvas link red
                // (`main` audit 8). The canvas draws the parent line as an
                // `<a href="#">`, but the GUI presents no surface to go to:
                // the static line carries no region and the press is a no-op.
                // The block's leading edge carries the item's media-kind icon
                // (user request 2026-11-20): the eighth-notes pair for audio,
                // the playing monitor for video, both in the accent sand.
                div {
                    class: "flex items-center",
                    style: format!(
                        "padding: 16px 20px 18px; gap: 12px; background: {}; border-bottom: 1px solid {};",
                        ef_var(EfToken::Bg0),
                        ef_var(EfToken::Bg3)
                    ),
                    {kind_icon(playback.now_playing_media)}
                    div {
                        class: "flex min-w-0 flex-1 flex-col",
                        style: "gap: 2px;",
                        Truncate {
                            text: title.clone(),
                            css: format!(
                                "font-size: 18px; font-weight: 600; color: {};",
                                ef_var(EfToken::Fg)
                            ),
                            style: TextStyle::sans(18.0, 600.0),
                        }
                        if let Some(parent_line) = playback.parent_line.as_ref() {
                            Truncate {
                                text: parent_line.clone(),
                                css: format!("font-size: 13px; color: {};", ef_var(EfToken::Red)),
                                style: TextStyle::sans(13.0, 400.0),
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
                    rsx! {
                        div {
                            class: "fixed",
                            style: "left: 0; top: 0; width: 0; height: 0;",
                            QueueRowMenu {
                                cursor: Some(cursor),
                                entries: vec![GuiRowMenuEntry {
                                    label: "Auto-hide controls".to_owned(),
                                    key_hint: Some("Enter".to_owned()),
                                    destructive: false,
                                    index: None,
                                    checked: Some(playback.controls.auto_hide),
                                }],
                                top,
                                left: Some(left),
                                label: "Player panel".to_owned(),
                                row_slot: menu_row_handles,
                            }
                        }
                    }
                }
            }
        }
    }
}
