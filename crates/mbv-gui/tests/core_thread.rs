//! Public-API tests of the GUI core thread (`mbv_gui::core_thread`): the
//! step unit the run loop drives, over a mocked channel and a mocked frame
//! sink (design D13 — no Tauri runtime, no webview, no live Owner).

use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use mbv::stub_app_with_queue;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::frame::queue::GuiQueueRow;
use mbv_gui::bridge::layout::{LayoutRect, LayoutRegion, LayoutReport, ListId, ListRows};
use mbv_gui::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};
use mbv_gui::core_thread::{CoreInput, CoreThread, FrameSink};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PointerEvent, PointerKind, Position};

/// A key press with no modifier held, as the translated `key` command
/// carries it.
fn key_press() -> CoreInput {
    CoreInput::Key(KeyEvent::new(Key::Char('n'), KeyModifiers::NONE))
}

/// A layout report for a window the front end has not measured into regions
/// yet, as the report on mount carries.
fn layout_report() -> CoreInput {
    CoreInput::Layout(LayoutReport {
        lists: vec![],
        regions: vec![],
        width: 800.0,
    })
}

/// A layout report the front end measured: `capacity` whole queue rows and
/// one painted region, as a report carries for a measured window.
fn measured_layout(capacity: usize, regions: Vec<LayoutRegion>) -> CoreInput {
    CoreInput::Layout(LayoutReport {
        lists: vec![ListRows {
            list: ListId::Queue,
            whole_rows: capacity,
        }],
        regions,
        width: 800.0,
    })
}

/// A painted region for `target` at the given logical-pixel rectangle.
fn region_at(x: f32, y: f32, target: RegionTarget) -> LayoutRegion {
    LayoutRegion {
        rect: LayoutRect {
            x,
            y,
            width: 100.0,
            height: 24.0,
        },
        target,
    }
}

/// A primary press with no modifier held, as the translated `pointer`
/// command carries it.
fn press_at(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::Press,
        Position { x, y },
        KeyModifiers::NONE,
    ))
}

/// A wheel notch downwards, as the translated `pointer` command carries it.
fn wheel_down_at(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::ScrollDown,
        Position { x, y },
        KeyModifiers::NONE,
    ))
}

/// The sink emitted no frame: the channel is empty while the core's sink
/// sender is still alive.
fn assert_no_frame(frames: &mpsc::Receiver<GuiFrame>) {
    assert_eq!(frames.try_recv(), Err(TryRecvError::Empty));
}

/// A core thread over a stub queue of `slots`: the sender for its channel,
/// the capture end of its frame sink, and the App's waker. The wait cadence
/// is zeroed, so a step with nothing queued is non-blocking.
struct TestCore {
    core: CoreThread,
    tx: mpsc::Sender<CoreInput>,
    waker: mbv::ClientWaker,
    frames: mpsc::Receiver<GuiFrame>,
}

fn test_core(slots: usize) -> TestCore {
    let app = stub_app_with_queue(slots);
    let waker = app.client_waker();
    let (tx, rx) = mpsc::channel();
    let (frame_tx, frame_rx) = mpsc::channel();
    let sink: FrameSink = Box::new(move |frame| {
        let _ = frame_tx.send(frame);
    });
    let mut core = CoreThread::new(app, rx, sink);
    core.set_idle_cadence_for_test(Duration::ZERO);
    core.install_waker(tx.clone());
    TestCore {
        core,
        tx,
        waker,
        frames: frame_rx,
    }
}

/// One input on the channel yields exactly one frame from one step, and the
/// frame carries the Queue rows the projection pushed; a step with nothing
/// queued changes nothing and emits no frame (design D3).
#[test]
fn one_input_yields_one_frame() {
    let mut test = test_core(2);
    test.tx.send(key_press()).unwrap();
    assert!(test.core.step());
    let frame = test.frames.try_recv().unwrap();
    assert_eq!(frame.queue.rows.len(), 2);
    assert_no_frame(&test.frames);
    assert!(test.core.step());
    assert_no_frame(&test.frames);
}

/// A waker wake feeds the channel and the next step ticks on it (design D3).
#[test]
fn waker_wake_yields_a_tick() {
    let mut test = test_core(2);
    test.waker.wake();
    assert!(test.core.step());
    test.frames.try_recv().unwrap();
    assert_no_frame(&test.frames);
}

/// A `report_layout` emits the current frame with no state change behind it
/// (design D3).
#[test]
fn layout_report_emits_a_frame_without_a_state_change() {
    let mut test = test_core(2);
    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    test.frames.try_recv().unwrap();
    assert_no_frame(&test.frames);
}

/// The first emitted frame carries the theme; a later frame does not repeat
/// it (design D7).
#[test]
fn only_the_first_frame_carries_the_theme() {
    let mut test = test_core(2);
    test.tx.send(key_press()).unwrap();
    assert!(test.core.step());
    let first = test.frames.try_recv().unwrap();
    assert!(first.theme.is_some());
    test.waker.wake();
    assert!(test.core.step());
    let second = test.frames.try_recv().unwrap();
    assert!(second.theme.is_none());
}

/// A `report_layout` re-arms the theme (design D7): the front end sends the
/// report when it mounts, so the frame after a webview reload carries the
/// theme again.
#[test]
fn a_layout_report_after_the_first_frame_carries_the_theme_again() {
    let mut test = test_core(2);
    test.tx.send(key_press()).unwrap();
    assert!(test.core.step());
    assert!(test.frames.try_recv().unwrap().theme.is_some());
    test.waker.wake();
    assert!(test.core.step());
    assert!(test.frames.try_recv().unwrap().theme.is_none());
    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    assert!(test.frames.try_recv().unwrap().theme.is_some());
}

/// A reported capacity of 14 sets the paging stride (task row 2.4): a
/// `PageDown` moves the cursor one row short of the reported viewport, not to
/// the list's end. The stub numbers its slots from 1, so the thirteenth row
/// carries slot 14.
#[test]
fn a_reported_capacity_sets_the_paging_stride() {
    let mut test = test_core(20);
    test.tx.send(measured_layout(14, vec![])).unwrap();
    assert!(test.core.step());
    test.frames.try_recv().unwrap();

    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::PageDown,
            KeyModifiers::NONE,
        )))
        .unwrap();
    assert!(test.core.step());
    let frame = test.frames.try_recv().unwrap();
    assert_eq!(frame.queue.cursor, Some(14));
    assert_eq!(frame.queue.rows.len(), 14);
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
    test.frames.try_recv().unwrap();

    test.tx.send(press_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.frames.try_recv().unwrap();
    assert_eq!(frame.queue.cursor, Some(2));

    test.tx.send(press_at(500.0, 500.0)).unwrap();
    assert!(test.core.step());
    let frame = test.frames.try_recv().unwrap();
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
    test.frames.try_recv().unwrap();

    test.tx.send(press_at(20.0, 110.0)).unwrap();
    assert!(test.core.step());
    let frame = test.frames.try_recv().unwrap();
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
    test.frames.try_recv().unwrap();

    test.tx.send(wheel_down_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());
    let frame = test.frames.try_recv().unwrap();
    let GuiQueueRow::Item { slot, .. } = &frame.queue.rows[0] else {
        panic!("the first viewport row is an item row");
    };
    // One wheel notch is the uniform three-row step: the viewport's first
    // row moves from slot 1 to slot 4.
    assert_eq!(*slot, 4);
    assert_eq!(frame.queue.cursor, Some(1));
}
