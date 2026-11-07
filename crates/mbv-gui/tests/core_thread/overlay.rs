//! The open overlay's pointer contracts (add-tauri-gui-client row 6.5, and
//! the review of row 6.4): a press outside the open menu, popover, or chooser
//! dismisses it without reaching the surface beneath, and a chooser or
//! popover row's press addresses the choice the last emitted frame painted —
//! a list change between the paint and the press selects that choice or
//! nothing, never the neighbour the index would name against a fresh read.

use std::sync::mpsc::Receiver;

use mbv::{
    ClientTickHost, set_cast_receivers_for_test, set_subtitle_tracks_for_test,
    stub_app_with_cast_target, stub_app_with_track_choices,
};
use mbv_gui::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};
use rstest::rstest;

use crate::{measured_layout, press_at, region_at, test_core, test_core_with};

/// Which overlay a dismissal case opens.
#[derive(Debug, Clone, Copy)]
enum Overlay {
    RowMenu,
    TrackPopover,
    TargetChooser,
}

/// The core a dismissal case drives, with the surface state its opener needs.
fn overlay_core(kind: Overlay) -> crate::TestCore {
    match kind {
        Overlay::RowMenu => test_core(4),
        Overlay::TrackPopover => test_core_with(stub_app_with_track_choices().0),
        Overlay::TargetChooser => test_core_with(stub_app_with_cast_target().0),
    }
}

/// The region whose press opens the case's overlay.
fn opener_region(kind: Overlay) -> RegionTarget {
    match kind {
        Overlay::RowMenu => RegionTarget::Queue(QueueTargetId::RowActions(2)),
        Overlay::TrackPopover => RegionTarget::QueuePlayback(QueuePlaybackTargetId::AudioChip),
        Overlay::TargetChooser => {
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget)
        }
    }
}

/// A press outside an open overlay dismisses it: the canvas's outside press,
/// the way the TUI's blocking context menu closes. The press is consumed, so
/// it reaches no surface (add-tauri-gui-client row 6.5).
#[rstest]
#[case::row_menu(Overlay::RowMenu)]
#[case::track_popover(Overlay::TrackPopover)]
#[case::target_chooser(Overlay::TargetChooser)]
fn an_outside_press_dismisses_the_open_overlay(#[case] kind: Overlay) {
    let mut test = overlay_core(kind);
    test.tx
        .send(measured_layout(
            4,
            vec![
                region_at(0.0, 0.0, opener_region(kind)),
                region_at(0.0, 100.0, RegionTarget::Queue(QueueTargetId::Slot(3))),
            ],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    let opened = test.next_frame();
    assert!(opened.overlay.is_some(), "the opener opened its overlay");

    test.tx.send(press_at(10.0, 110.0)).unwrap();
    assert!(test.core.step());
    assert!(
        test.next_frame().overlay.is_none(),
        "the outside press dismissed the overlay"
    );
}

/// The press that dismisses an overlay does not activate the row beneath it:
/// the canvas treats the open overlay as modal, as the TUI's blocking context
/// menu does, so the queue cursor stays where it was.
#[test]
fn the_dismissing_press_does_not_activate_the_row_beneath() {
    let mut test = test_core(4);
    test.tx
        .send(measured_layout(
            4,
            vec![
                region_at(0.0, 0.0, RegionTarget::Queue(QueueTargetId::RowActions(2))),
                region_at(0.0, 100.0, RegionTarget::Queue(QueueTargetId::Slot(3))),
            ],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    assert!(test.next_frame().overlay.is_some());

    test.tx.send(press_at(10.0, 110.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert!(frame.overlay.is_none());
    assert_eq!(frame.queue.cursor, Some(1), "the row beneath took no press");
}

/// A cast receiver's id and name, as the replacement helper takes them.
const BEDROOM: (&str, &str) = ("receiver-2", "Bedroom");
const LIVING_ROOM: (&str, &str) = ("receiver-1", "Living Room");

/// The cast receivers every `SelectPlaybackOutput` the Owner received names.
fn cast_selections(cmd_rx: &Receiver<mbv_ctrl::CtrlCmd>) -> Vec<String> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Cast {
                receiver_id,
                ..
            }) => Some(receiver_id),
            _ => None,
        })
        .collect()
}

/// The target ids every `SetSub` the Owner received names.
fn subtitle_switches(cmd_rx: &Receiver<mbv_ctrl::CtrlCmd>) -> Vec<i64> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::SetSub(id)) => Some(id),
            _ => None,
        })
        .collect()
}

/// A chooser row's press selects the choice the last emitted frame painted,
/// not the row a fresh list read would put at that index: a receiver added
/// ahead of the painted one cannot capture the press (regression for the row
/// 6.4 review's neighbour selection).
#[test]
fn a_chooser_press_selects_the_painted_choice_after_the_list_grows() {
    let (app, cmd_rx) = stub_app_with_cast_target();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    // The chooser opens over the one receiver the frame paints, and its row
    // is reported as painted.
    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                50.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    // A receiver arrives ahead of the painted one; the window has painted
    // neither it nor a new region yet.
    set_cast_receivers_for_test(test.core.app_mut(), vec![BEDROOM, LIVING_ROOM]);

    test.tx.send(press_at(10.0, 60.0)).unwrap();
    assert!(test.core.step());
    assert!(test.next_frame().overlay.is_none());
    assert_eq!(
        cast_selections(&cmd_rx),
        vec!["receiver-1"],
        "the press selects the receiver the frame painted"
    );
}

/// A chooser row's press whose painted choice left the list selects nothing
/// (regression for the row 6.4 review: it must not select the neighbour).
#[test]
fn a_chooser_press_selects_nothing_when_the_painted_choice_is_gone() {
    let (app, cmd_rx) = stub_app_with_cast_target();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                50.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(0)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    set_cast_receivers_for_test(test.core.app_mut(), vec![]);

    test.tx.send(press_at(10.0, 60.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();
    assert_eq!(cast_selections(&cmd_rx), Vec::<String>::new());
}

/// A subtitle popover row's press switches to the track the last emitted
/// frame painted, not the track a fresh list read would put at that index
/// (regression for the row 6.4 review's neighbour selection).
#[test]
fn a_popover_press_selects_the_painted_track_after_the_list_grows() {
    let (mut app, cmd_rx) = stub_app_with_track_choices();
    set_subtitle_tracks_for_test(&mut app, vec![(7, "Old".to_string(), false)]);
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::SubtitleChip),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                50.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(1)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    // A track arrives ahead of the painted one: index 1 is a different
    // track in the live list.
    set_subtitle_tracks_for_test(
        test.core.app_mut(),
        vec![(8, "New".to_string(), false), (7, "Old".to_string(), false)],
    );

    test.tx.send(press_at(10.0, 60.0)).unwrap();
    assert!(test.core.step());
    assert!(test.next_frame().overlay.is_none());
    assert_eq!(
        subtitle_switches(&cmd_rx),
        vec![7],
        "the press switches the track the frame painted"
    );
}

/// A popover row's press whose painted track left the list switches nothing
/// (regression for the row 6.4 review).
#[test]
fn a_popover_press_switches_nothing_when_the_painted_track_is_gone() {
    let (mut app, cmd_rx) = stub_app_with_track_choices();
    set_subtitle_tracks_for_test(&mut app, vec![(7, "Old".to_string(), false)]);
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::SubtitleChip),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                50.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(1)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    set_subtitle_tracks_for_test(test.core.app_mut(), vec![]);

    test.tx.send(press_at(10.0, 60.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();
    assert_eq!(subtitle_switches(&cmd_rx), Vec::<i64>::new());
}
