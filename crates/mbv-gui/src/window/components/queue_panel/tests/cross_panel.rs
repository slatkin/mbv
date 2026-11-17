//! The region-order contracts `QueuePlaybackPanel.test.ts` deferred until
//! this panel existed: an open chooser's rows reporting after the queue's,
//! and the full player panel's region order over the expanded artboard.

use dioxus_native::prelude::*;

use super::harness::*;
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

/// Contract (`QueuePlaybackPanel.test.ts`): an open chooser reports its rows
/// after the queue and footer regions — the hit map's last push wins where
/// a chooser row paints over a queue row beneath it. The header's `filter`
/// region leads the queue's, the footer's line reports none.
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

/// Contract (`QueuePlaybackPanel.test.ts`): a paint reports the player
/// panel's transport, seek, header, seekbar, and target regions, then the
/// queue header's and rows' regions — the two panels mount in that order
/// under the expanded artboard.
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
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Previous),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekForward),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Next),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::VisualSlot),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::Collapse),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::Queue(QueueTargetId::Slot(1)),
        ]
    );
}
