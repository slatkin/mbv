//! The region-order contracts `QueuePlaybackPanel.test.ts` deferred until
//! this panel existed: an open chooser's rows reporting after the queue's and
//! the footer's, and the full player panel's region order over the expanded
//! artboard — the player block's stage, then its transport, seek, and seekbar
//! controls, the queue header's and rows' regions, then the footer band's
//! moved playback-target and collapse controls
//! (`move-playback-header-into-queue-footer` D4; the stage's lead comes from
//! `gui-art-player-overlay` D4; the auto-hide control lives in the player
//! menu, gui-player-overlay-menu).

use blitz_dom::LocalName;
use dioxus_native::prelude::*;

use super::harness::*;
use crate::art::GuiArtRef;
use crate::bridge::frame::playback::GuiPlayerControls;
use crate::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};
use crate::window::sections::SectionRef;

/// Contract (`QueuePlaybackPanel.test.ts`, main cafcbc239): a chooser row
/// paints the brand mark of its target kind.
#[test]
fn a_chooser_row_paints_the_brand_mark_of_its_target_kind() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.overlay.set(Some(chooser()));
    });

    assert_eq!(
        fixture.all("[data-choice='0'] [data-brand='emby']").len(),
        1
    );
    assert_eq!(
        fixture.all("[data-choice='1'] [data-brand='cast']").len(),
        1
    );
}

/// Contract (`QueuePlaybackPanel.test.ts`, `move-playback-header-into-queue-
/// footer` D4): an open chooser reports its rows after the queue and footer
/// regions — the hit map's last push wins where a chooser row paints over a
/// queue row beneath it. The footer band's control regions lead the queue's
/// rows; the band's own second key carries the chooser's rows last.
#[test]
fn an_open_chooser_reports_its_rows_after_the_queue_and_footer_regions() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.queue.set(SectionRef::new(queue(vec![item(1)])));
        sections.summary.set(summary());
        playhead.stamp(anchor);
    });
    let mut sections = fixture.sections.clone();
    fixture.in_runtime(move || sections.overlay.set(Some(chooser())));

    let targets = fixture.last_reported_targets();
    let first_choice = targets
        .iter()
        .position(|target| {
            matches!(
                target,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(_))
            )
        })
        .expect("the chooser reports its rows");
    let before = &targets[..first_choice];
    for wanted in [
        RegionTarget::Queue(QueueTargetId::Filter),
        RegionTarget::Queue(QueueTargetId::Slot(1)),
        RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
        RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
    ] {
        assert!(before.contains(&wanted), "{targets:?}");
    }
    assert_eq!(
        &targets[first_choice..],
        &[
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(1)),
        ]
    );
}

/// Contract (`gui-art-player-overlay` D2, task 4.1): the stage carries
/// `data-auto-hide` and `data-playing` only while those facts hold — each is
/// an empty-valued attribute, never `"false"`, because the stylesheet's hover
/// rules test presence — and the no-image column, which paints no stage,
/// carries neither.
#[test]
fn the_stage_carries_the_auto_hide_and_playing_attributes_only_while_they_hold() {
    let mut fixture = built_sized(panel_tree_app);
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    let mut sections = fixture.sections.clone();
    let mut playing_frame = playing();
    playing_frame.controls = GuiPlayerControls {
        auto_hide: true,
        can_start: false,
    };
    fixture.in_runtime(move || {
        sections.playback.set(playing_frame);
        playhead.stamp(anchor);
    });

    let stage = "[data-target='visualSlot']";
    assert_eq!(
        fixture.attr_of(stage, LocalName::from("data-auto-hide")),
        Some(String::new()),
        "the stage carries the auto-hide attribute with no value"
    );
    assert_eq!(
        fixture.attr_of(stage, LocalName::from("data-playing")),
        Some(String::new()),
        "the playing transport carries the playing attribute with no value"
    );

    let mut sections = fixture.sections.clone();
    let mut placeholder = playing();
    placeholder.artwork = Some(GuiArtRef::Placeholder {
        src: "mbv-art://localhost/card/k".to_owned(),
    });
    placeholder.controls = GuiPlayerControls {
        auto_hide: true,
        can_start: false,
    };
    fixture.in_runtime(move || sections.playback.set(placeholder));

    assert!(!fixture.presents("[data-target='visualSlot']"));
    assert!(!fixture.presents("[data-auto-hide]"));
    assert!(!fixture.presents("[data-playing]"));
}

/// Contract (`QueuePlaybackPanel.test.ts`, `move-playback-header-into-queue-
/// footer` D4; `gui-art-player-overlay` D4, task 3.4a): a paint reports the
/// stage's `VisualSlot` region first, then the player panel's play/pause,
/// seek, and seekbar regions, then the queue header's and rows' regions,
/// then the footer band's playback-target and collapse regions — the three
/// panels mount in that order under the expanded artboard, and the stage
/// leads its own block so a control over the artwork wins the press.
#[test]
fn a_paint_reports_the_player_panels_regions_then_the_queue_rows() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.queue.set(SectionRef::new(queue(vec![item(1)])));
        sections.summary.set(summary());
        playhead.stamp(anchor);
    });
    // The chooser stays closed: no chooser regions join the order.

    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Previous),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekForward),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar),
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::Queue(QueueTargetId::NewQueue),
            RegionTarget::Queue(QueueTargetId::Slot(1)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
        ]
    );
}
