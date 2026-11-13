//! The region-order contracts `QueuePlaybackPanel.test.ts` deferred until
//! this panel existed: an open chooser's rows reporting after the queue's,
//! and the full player panel's region order over the expanded artboard.

use dioxus_native::prelude::*;

use super::harness::*;
use crate::bridge::frame::focus::{GuiFocus, GuiPanel};
use crate::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};

/// Contract (`QueuePlaybackPanel.test.ts`, deferred to 5.5): an open chooser
/// reports its rows after the queue's regions — the hit map's last push
/// wins where a chooser row paints over a queue row beneath it. The
/// footer's regions and the header's `rightColumn` and `filter` regions
/// join the front with their ports (task 5.6).
#[test]
fn an_open_chooser_reports_its_rows_after_the_queue_regions() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.queue.set(queue(vec![item(1)]));
        sections.summary.set(summary());
        sections.focus.set(GuiFocus {
            surface: None,
            panel: GuiPanel::Queue,
        });
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
    assert!(
        before.contains(&RegionTarget::Queue(QueueTargetId::Slot(1))),
        "{targets:?}"
    );
    assert_eq!(
        &targets[first_choice..],
        &[
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(1)),
        ]
    );
}

/// Contract (`QueuePlaybackPanel.test.ts`, deferred to 5.5): a paint reports
/// the player panel's transport, seek, header, seekbar, and target regions,
/// then the queue rows' slot regions — the two panels mount in that order
/// under the expanded artboard.
#[test]
fn a_paint_reports_the_player_panels_regions_then_the_queue_rows() {
    let mut fixture = built_sized(panel_tree_app);
    let mut sections = fixture.sections.clone();
    let anchor = anchor();
    let mut playhead = fixture.playhead.clone();
    fixture.in_runtime(move || {
        sections.playback.set(playing());
        sections.queue.set(queue(vec![item(1)]));
        sections.summary.set(summary());
        sections.focus.set(GuiFocus {
            surface: None,
            panel: GuiPanel::Queue,
        });
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
            RegionTarget::Queue(QueueTargetId::Slot(1)),
        ]
    );
}
