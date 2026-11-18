//! The player panel's top block (`replace-tauri-with-blitz` task 5.2, the
//! port of `QueuePlaybackPanel`'s artwork, title, and transport parts): the
//! expanded canvas artboard's stage with its overlays, the title and parent
//! line, and the no-image column. The canvas, not the TUI band, is the
//! reference; the paint uses the theme's roles and chrome fills through
//! `--mbv-<name>` properties only (design D7).
//!
//! The stage is the block's topmost content: the panel paints no header row
//! above it (`move-playback-header-into-queue-footer` D6), so the column's top
//! edge under the window's global header is the stage itself. The playback
//! target's chooser and the collapse control live in the Queue footer's band
//! with their regions (`queue_footer.rs`); this block reports neither. The
//! canvas paints no queue-fold control here (`main` audit 4); `q` stays a
//! chord and the folded line owns the expand control.
//!
//! The stage, its two overlay layers, and the control rows are this module's
//! second file, `queue_playback_panel/art_stage.rs` (`gui-art-player-overlay`
//! design D1, D8): the stage is the visual slot's wrapper, the media box owns
//! the clip, the top layer holds the track chips and the episode badge, and
//! the bottom layer holds the gradient and the two control rows. The panel
//! keeps every mounted handle and reports the regions in one contribution.
//!
//! The scrubber — the 6 px track, the intro and credits blocks, the played
//! fill, the chapter ticks, the thumb, the hover preview, and the clocks — is
//! this module's third file, `queue_playback_panel/scrubber.rs` (task 5.3):
//! the component paints it in the control rows' first row, and its slider box
//! is the `seekbar` region of this panel's one report, so the panel holds the
//! element handle the scrubber sets on mount.
//!
//! A press on the bar or a button forwards like every other pointer event:
//! the component interprets no gesture, it reports its regions and the core
//! resolves the press against the hit map (design D4).

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::layout::{LayoutRegion, QueuePlaybackTargetId, RegionTarget};
use crate::window::components::art_image::ArtEvents;
use crate::window::layout::LayoutContribution;
use crate::window::layout::{LayoutEpoch, LayoutKey, LayoutReporter, element_region};
use crate::window::playhead::Playhead;
use crate::window::sections::Sections;
use crate::window::truncate::{TextStyle, Truncate};

mod art_stage;
mod scrubber;

use art_stage::{ArtStage, ChipsAndBadge, ControlRowHandles, ControlRows, player_scale};

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

/// The stage's region, with the control rows' scale updated from its measured
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
    let auto_hide_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // The control rows' scale (design D3): layout, not interaction state. The
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
        let auto_hide = auto_hide_mounted.cloned();
        let art = art_mounted.cloned();
        let scale_signal = stage_scale;
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
                (auto_hide, QueuePlaybackTargetId::AutoHide),
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
    // Whether the transport plays (design D2): the anchor's same fact, so a
    // paused or idle transport paints no `data-playing` attribute.
    let playing = !paused;
    // The control rows paint while something plays and while the shown local
    // queue can start (design D7). They are not behind `show_controls`: the
    // stopped state carries `show_controls: false` and still paints them.
    let controls_shown = playback.controls.can_start || playback.now_playing_title.is_some();
    let rows = ControlRowHandles {
        play_pause: play_pause_mounted,
        previous: prev_mounted,
        seek_back: seek_back_mounted,
        seek_forward: seek_forward_mounted,
        next: next_mounted,
        seekbar: seekbar_mounted,
        auto_hide: auto_hide_mounted,
    };
    // The two arrangements (design D6): the stage for a media box, and the
    // no-image column — the chips-and-badge line, then the control rows —
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
                playing,
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
                style: "padding: 10px 16px 0;",
                ChipsAndBadge {
                    chips: playback.track_chips.clone(),
                    overlay: sections.overlay.cloned(),
                    episode_badge: playback.episode_badge.clone(),
                }
                ControlRows {
                    paused,
                    availability: playback.availability,
                    auto_hide: playback.controls.auto_hide,
                    rows,
                }
            }
        }
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
            if playback.now_playing_title.is_some() {
                {art_section}
                // The title at the canvas's 18 px/600 and its parent line in
                // the canvas link style at 13 px (`main` audit 8). The canvas
                // draws the parent line as an `<a href="#">`, but the GUI
                // presents no surface to go to: the static line carries no
                // region and the press is a no-op.
                div {
                    class: "flex min-w-0 flex-col",
                    style: "padding: 14px 16px 6px; gap: 2px;",
                    if let Some(title) = playback.now_playing_title.as_ref() {
                        Truncate {
                            text: title.clone(),
                            css: "font-size: 18px; font-weight: 600; color: {role_var(GuiRole::PlaybackTitleFg)};"
                                .to_owned(),
                            style: TextStyle::sans(18.0, 600.0),
                        }
                    }
                    if let Some(parent_line) = playback.parent_line.as_ref() {
                        Truncate {
                            text: parent_line.clone(),
                            css: "font-size: 13px; color: var(--canvas-type-secondary);".to_owned(),
                            style: TextStyle::sans(13.0, 400.0),
                        }
                    }
                }
            } else if controls_shown {
                // The stopped state (design D7): the two control rows alone,
                // with no stage, no chips line, and no title. The scrubber
                // paints its empty track, because the playhead predicts no
                // progress.
                ControlRows {
                    paused,
                    availability: playback.availability,
                    auto_hide: playback.controls.auto_hide,
                    rows,
                }
            }
        }
    }
}
