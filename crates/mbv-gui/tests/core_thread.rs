//! Public-API tests of the GUI core thread (`mbv_gui::core_thread`): the
//! step unit the run loop drives, over a mocked channel and a mocked frame
//! sink (design D13 — no Tauri runtime, no webview, no live Owner).

use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use mbv::{App, stub_app_with_prefix_and_toast, stub_app_with_queue};
use mbv_gui::bridge::frame::queue::GuiQueueRow;
use mbv_gui::bridge::frame::{GuiFrame, GuiWindowFrame};
use mbv_gui::bridge::layout::{LayoutRect, LayoutRegion, LayoutReport, ListId, ListRows};
use mbv_gui::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};
use mbv_gui::core_thread::{CoreInput, CoreThread, FrameSink, QuitSink};
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
fn assert_no_frame(frames: &mpsc::Receiver<GuiWindowFrame>) {
    assert_eq!(frames.try_recv(), Err(TryRecvError::Empty));
}

/// A core thread over a stub queue of `slots`: the sender for its channel,
/// the capture end of its frame sink, and the App's waker. The wait cadence
/// is zeroed, so a step with nothing queued is non-blocking.
struct TestCore {
    core: CoreThread,
    tx: mpsc::Sender<CoreInput>,
    waker: mbv::ClientWaker,
    frames: mpsc::Receiver<GuiWindowFrame>,
    quits: mpsc::Receiver<()>,
}

impl TestCore {
    /// The next emitted frame's slice, as every core-thread frame is one.
    fn next_frame(&self) -> GuiFrame {
        match self.frames.try_recv().expect("the core emitted a frame") {
            GuiWindowFrame::Slice(frame) => *frame,
            GuiWindowFrame::Attach(..) => panic!("the core thread emitted an attach state"),
        }
    }
}

fn test_core(slots: usize) -> TestCore {
    test_core_with(stub_app_with_queue(slots))
}

/// A core thread over a caller-built `App`, for the stubs the queue stub
/// does not cover.
fn test_core_with(app: App) -> TestCore {
    let waker = app.client_waker();
    let (tx, rx) = mpsc::channel();
    let (frame_tx, frame_rx) = mpsc::channel();
    let sink: FrameSink = Box::new(move |frame| {
        let _ = frame_tx.send(frame);
    });
    let (quit_tx, quit_rx) = mpsc::channel();
    let quit: QuitSink = Box::new(move || {
        let _ = quit_tx.send(());
    });
    let mut core = CoreThread::new(app, rx, sink, quit);
    core.set_idle_cadence_for_test(Duration::ZERO);
    core.install_waker(tx.clone());
    TestCore {
        core,
        tx,
        waker,
        frames: frame_rx,
        quits: quit_rx,
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
    let frame = test.next_frame();
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
    test.next_frame();
    assert_no_frame(&test.frames);
}

/// A `report_layout` emits the current frame with no state change behind it
/// (design D3).
#[test]
fn layout_report_emits_a_frame_without_a_state_change() {
    let mut test = test_core(2);
    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    test.next_frame();
    assert_no_frame(&test.frames);
}

/// The first emitted frame carries the theme; a later frame does not repeat
/// it (design D7).
#[test]
fn only_the_first_frame_carries_the_theme() {
    let mut test = test_core(2);
    test.tx.send(key_press()).unwrap();
    assert!(test.core.step());
    let first = test.next_frame();
    assert!(first.theme.is_some());
    test.waker.wake();
    assert!(test.core.step());
    let second = test.next_frame();
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
    assert!(test.next_frame().theme.is_some());
    test.waker.wake();
    assert!(test.core.step());
    assert!(test.next_frame().theme.is_none());
    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    assert!(test.next_frame().theme.is_some());
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
    test.next_frame();

    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::PageDown,
            KeyModifiers::NONE,
        )))
        .unwrap();
    assert!(test.core.step());
    let frame = test.next_frame();
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

/// A routed quit key fires the quit edge and ends the run (design D9): the
/// window closes through the same edge the mismatch state's Quit uses.
#[test]
fn a_routed_quit_key_fires_the_quit_edge_and_ends_the_run() {
    let mut test = test_core(2);
    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::Char('q'),
            KeyModifiers::NONE,
        )))
        .unwrap();
    assert!(!test.core.step());
    test.quits.try_recv().expect("the quit edge fired");
}

/// The window's close ends the run (design D9): the close the window event
/// delivers is an input the core answers as a quit, so the worker stops and
/// the App's drop releases the Client — no sender drop can carry it, because
/// the App's waker keeps the channel connected for the thread's whole life.
#[test]
fn a_window_close_ends_the_run() {
    let mut test = test_core(2);
    test.tx.send(CoreInput::Close).unwrap();
    assert!(!test.core.step());
    test.quits.try_recv().expect("the quit edge ran");
}
