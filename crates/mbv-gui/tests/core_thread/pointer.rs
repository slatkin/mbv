//! The pointer half of the core-thread contract (design D4): a recognized
//! gesture resolves against the combined hit map of the last
//! `LayoutReport`, routes to the surface that owns the region, and the
//! leaf emits the same semantic `Msg`s the TUI component's pointer path
//! emits (`client-interaction-core`: pointer targets resolve from a
//! semantic hit map).

use mbv::{stub_app_with_cast_target, stub_app_with_prefix_and_toast, stub_app_with_track_choices};
use mbv_config::TestStateDirGuard;
use mbv_gui::bridge::frame::overlay::{GuiOverlayFrame, GuiTrackChipKind};
use mbv_gui::bridge::frame::queue::GuiQueueRow;
use mbv_gui::bridge::layout::{
    LayoutReport, LayoutSize, ListId, ListRows, QueuePlaybackTargetId, QueueTargetId, RegionTarget,
};
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

use crate::{
    TestCore, measured_layout, move_at, press_at, region_at, test_core, test_core_with,
    wheel_down_at,
};

/// Send one input, run one core step, and take the frame it emitted: the
/// core's one-input-one-frame contract, stated once for the tests that send a
/// short sequence.
fn step_input(test: &mut TestCore, input: CoreInput) {
    test.tx.send(input).unwrap();
    assert!(test.core.step());
    test.next_frame();
}

/// A report that carries a new container size re-binds the capacity and the
/// regions (task row 6.3, design D10): the front end's container resize
/// painted a new layout, so the report's whole rows replace the Queue
/// viewport's height and its regions rebuild the hit map — a press resolves
/// against the newest report's regions, and the first report's region
/// resolves to nothing.
#[test]
fn a_reported_container_size_rebinds_capacity_and_regions() {
    let mut test = test_core(20);
    test.tx
        .send(measured_layout(
            14,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::Queue(QueueTargetId::Slot(2)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();
    test.tx.send(press_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(test.next_frame().queue.cursor, Some(2));

    // The container resize painted a shorter list and moved the row
    // region: the next report carries the new layout.
    test.tx
        .send(CoreInput::Layout(LayoutReport {
            lists: vec![ListRows {
                list: ListId::Queue,
                whole_rows: 3,
            }],
            regions: vec![region_at(
                200.0,
                100.0,
                RegionTarget::Queue(QueueTargetId::Slot(1)),
            )],
            container_size: LayoutSize {
                width: 300.0,
                height: 200.0,
            },
        }))
        .unwrap();
    assert!(test.core.step());
    assert_eq!(test.next_frame().queue.rows.len(), 3);

    // The first report's region is gone: the press there resolves to
    // nothing, so the cursor stays put.
    test.tx.send(press_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(test.next_frame().queue.cursor, Some(2));

    // The resized layout's region resolves its slot.
    test.tx.send(press_at(250.0, 110.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(test.next_frame().queue.cursor, Some(1));
}

/// A press resolves against the last reported regions (task row 2.4): the
/// press inside the reported row region moves the cursor to that row's slot,
/// and a press on no reported region changes nothing (spec `gui-client`:
/// a press lands on no target resolves to nothing).
#[test]
fn a_press_resolves_against_the_last_reported_regions() {
    let mut test = test_core(3);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::Queue(QueueTargetId::Slot(2)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert_eq!(frame.queue.cursor, Some(2));

    test.tx.send(press_at(500.0, 500.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert_eq!(frame.queue.cursor, Some(2));
}

/// A press on the play/pause region dispatches the playback request while
/// the Queue panel holds focus (task row 2.4, design D4): the press routes
/// to the surface that owns the region, not to the focused one.
#[test]
fn a_press_on_the_play_pause_region_dispatches_the_playback_request() {
    let mut test = test_core(3);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                10.0,
                100.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlayPause),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(20.0, 110.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    let toast = frame.status.toast.expect("the toggle flashed a toast");
    assert_eq!(toast.text, "Pause requested");
}

/// The wheel over a painted queue row moves the viewport by the uniform
/// wheel step and leaves the selection alone (spec `gui-client`: the wheel
/// moves over the Queue panel by the uniform wheel step).
#[test]
fn the_wheel_over_a_reported_row_moves_the_viewport_by_the_wheel_step() {
    let mut test = test_core(20);
    test.tx
        .send(measured_layout(
            14,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::Queue(QueueTargetId::Slot(2)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(wheel_down_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    let GuiQueueRow::Item { slot, .. } = &frame.queue.rows[0] else {
        panic!("the first viewport row is an item row");
    };
    // One wheel notch is the uniform three-row step: the viewport's first
    // row moves from slot 1 to slot 4.
    assert_eq!(*slot, 4);
    assert_eq!(frame.queue.cursor, Some(1));
}

/// Any pointer event cancels an armed prefix, even one that resolves to no
/// gesture or no reported region — the shared contract `App::route_gui_input`'s
/// pointer arm carries, the same silent disarm every terminal mouse event
/// applies in the TUI (regression for the 9104ddaa5 review: the resolution
/// drop ran before the disarm, so an untargeted press left the prefix armed).
#[test]
fn an_untargeted_pointer_press_cancels_an_armed_prefix() {
    let mut test = test_core_with(stub_app_with_prefix_and_toast("prefix armed"));
    test.tx.send(press_at(500.0, 500.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert!(!frame.status.prefix_armed);
}

/// The header's playback-target press opens the chooser, and a chooser
/// row's press selects that output (add-tauri-gui-client row 6.4): the
/// press resolves the choice's index against the live target list and the
/// selection reaches the Owner as the `SelectPlaybackOutput` the TUI's F3
/// selection sends.
#[test]
fn the_target_press_opens_the_chooser_and_a_choice_selects_it() {
    let (app, cmd_rx) = stub_app_with_cast_target();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            3,
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
    let frame = test.next_frame();
    let Some(GuiOverlayFrame::TargetChooser { cursor, choices }) = &frame.overlay else {
        panic!("the target press opened the chooser: {:?}", frame.overlay);
    };
    assert_eq!(*cursor, 0);
    assert_eq!(choices.len(), 2, "the local row leads the remote target");
    assert_eq!(choices[1].label, "Living Room");

    // The chooser's paint reports its row: the choice region joins the
    // next report's hit map.
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                50.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TargetChoice(1)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 60.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert!(frame.overlay.is_none(), "the selection closed the chooser");
    let selections: Vec<_> = cmd_rx
        .try_iter()
        .filter(|cmd| matches!(cmd, mbv_ctrl::CtrlCmd::SelectPlaybackOutput(_)))
        .collect();
    assert_eq!(selections.len(), 1);
    assert!(matches!(
        selections[0],
        mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Cast {
            ref receiver_id,
            ..
        }) if receiver_id == "receiver-1"
    ));
}

/// The back and forward controls press the same command the
/// `seek_back_10` and `seek_forward_30` chords send (the `gui-client`
/// spec's "back 10 s, forward 30 s": the buttons and the chords do the
/// same work).
#[test]
fn the_seek_back_and_forward_presses_send_the_chords_seeks() {
    let (app, cmd_rx) = stub_app_with_cast_target();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            3,
            vec![
                region_at(
                    0.0,
                    0.0,
                    RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekBack),
                ),
                region_at(
                    0.0,
                    30.0,
                    RegionTarget::QueuePlayback(QueuePlaybackTargetId::SeekForward),
                ),
            ],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    // One step per input: the seek-back press, the seek-forward press, and
    // the two chords each land their own command.
    step_input(&mut test, press_at(10.0, 10.0));
    step_input(&mut test, press_at(10.0, 40.0));
    step_input(
        &mut test,
        CoreInput::Key(KeyEvent::new(Key::Char('['), KeyModifiers::NONE)),
    );
    step_input(
        &mut test,
        CoreInput::Key(KeyEvent::new(Key::Char(']'), KeyModifiers::NONE)),
    );

    let seeks: Vec<f64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::Seek(secs)) => Some(secs),
            _ => None,
        })
        .collect();
    assert_eq!(seeks, vec![-10.0, 30.0, -10.0, 30.0]);
}

/// A single press on the seekbar seeks to the pressed position (the
/// `gui-client` spec's "a press on the bar SHALL seek"): the press
/// column's fraction of the painted bar span resolves through the shared
/// seek rule, here the midpoint of the ten-minute stub.
#[test]
fn a_single_seekbar_press_seeks_to_the_pressed_position() {
    let (app, cmd_rx) = stub_app_with_cast_target();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    test.next_frame();

    let seeks: Vec<f64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::SeekAbsolute(secs)) => Some(secs),
            _ => None,
        })
        .collect();
    assert_eq!(seeks, vec![300.0]);
}

/// A move over the seekbar carries the hover to the frame, and a move away
/// clears it (design D4): the frame's preview facts are the pointer's
/// fraction of the painted bar and the time it names.
#[test]
fn a_move_over_the_seekbar_carries_the_hover_to_the_frame() {
    let (app, _cmd_rx) = stub_app_with_cast_target();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(move_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    let hover = frame.playback.hover.expect("the move set the hover");
    assert_eq!(hover.fraction, 0.5);
    // The frame carries the hover's time formatted by the TUI's shared
    // duration formatter, not the raw ticks.
    assert_eq!(hover.time_label, "5:00");
    // No queue item carries the playhead, so no chapter covers the point.
    assert_eq!(hover.chapter, None);

    test.tx.send(move_at(500.0, 500.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert_eq!(frame.playback.hover, None);
}

/// A chip press opens its track popover (the `gui-client` spec's "the
/// audio and subtitle chips SHALL each open a popover"), and the same
/// press on the open popover's chip closes it, as the playback-target
/// press toggles the chooser (add-tauri-gui-client row 6.4).
#[test]
fn a_chip_press_opens_its_popover_and_the_same_press_closes() {
    let (app, _cmd_rx) = stub_app_with_track_choices();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::AudioChip),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    let Some(GuiOverlayFrame::TrackPopover { chip, choices, .. }) = &frame.overlay else {
        panic!("the chip press opened its popover: {:?}", frame.overlay);
    };
    assert_eq!(*chip, GuiTrackChipKind::Audio);
    assert_eq!(
        choices
            .iter()
            .map(|choice| (choice.id, choice.chosen))
            .collect::<Vec<_>>(),
        vec![(1, true)]
    );

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert!(frame.overlay.is_none(), "the same press closed the popover");
}

/// A track popover row's press selects that choice through the popover's
/// Enter execution (add-tauri-gui-client row 6.4, the `gui-client` spec's
/// "the chosen track SHALL be sent to the Owner process"): the subtitle
/// popover's track row sends the `SetSub` the keyboard's Enter sends, and
/// the selection closes the popover.
#[test]
fn a_popover_row_press_sends_the_track_switch() {
    // The subtitle choice persists the picked track, so the state dir is
    // the isolated one.
    let _state_dir = TestStateDirGuard::new();
    let (app, cmd_rx) = stub_app_with_track_choices();
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            3,
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
    let frame = test.next_frame();
    let Some(GuiOverlayFrame::TrackPopover { chip, choices, .. }) = &frame.overlay else {
        panic!(
            "the chip press opened the subtitle popover: {:?}",
            frame.overlay
        );
    };
    assert_eq!(*chip, GuiTrackChipKind::Subtitles);
    assert_eq!(choices.len(), 2, "Off and the one subtitle track");

    // The popover's paint reports its rows: the track row's region joins
    // the next report's hit map.
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                50.0,
                RegionTarget::QueuePlayback(QueuePlaybackTargetId::TrackPopoverChoice(1)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx.send(press_at(10.0, 60.0)).unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
    assert!(frame.overlay.is_none(), "the selection closed the popover");
    let switches: Vec<i64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::SetSub(id)) => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(switches, vec![2]);
}
