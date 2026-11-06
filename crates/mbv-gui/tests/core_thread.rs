//! Public-API tests of the GUI core thread (`mbv_gui::core_thread`): the
//! step unit the run loop drives, over a mocked channel and a mocked frame
//! sink (design D13 — no Tauri runtime, no webview, no live Owner).

use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use mbv::stub_app_with_queue;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::layout::LayoutReport;
use mbv_gui::core_thread::{CoreInput, CoreThread, FrameSink};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

/// A key press with no modifier held, as the translated `key` command
/// carries it.
fn key_press() -> CoreInput {
    CoreInput::Key(KeyEvent::new(Key::Char('n'), KeyModifiers::NONE))
}

/// The sink emitted no frame: the channel is empty while the core's sink
/// sender is still alive.
fn assert_no_frame(frames: &mpsc::Receiver<GuiFrame>) {
    assert_eq!(frames.try_recv(), Err(TryRecvError::Empty));
}

/// A core thread over a stub two-slot queue: the sender for its channel, the
/// capture end of its frame sink, and the App's waker. The wait cadence is
/// zeroed, so a step with nothing queued is non-blocking.
struct TestCore {
    core: CoreThread,
    tx: mpsc::Sender<CoreInput>,
    waker: mbv::ClientWaker,
    frames: mpsc::Receiver<GuiFrame>,
}

fn test_core() -> TestCore {
    let app = stub_app_with_queue(2);
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
    let mut test = test_core();
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
    let mut test = test_core();
    test.waker.wake();
    assert!(test.core.step());
    test.frames.try_recv().unwrap();
    assert_no_frame(&test.frames);
}

/// A `report_layout` emits the current frame with no state change behind it
/// (design D3).
#[test]
fn layout_report_emits_a_frame_without_a_state_change() {
    let mut test = test_core();
    test.tx
        .send(CoreInput::Layout(LayoutReport {
            lists: vec![],
            regions: vec![],
            width: 800.0,
        }))
        .unwrap();
    assert!(test.core.step());
    test.frames.try_recv().unwrap();
    assert_no_frame(&test.frames);
}

/// The first emitted frame carries the theme; a later frame does not repeat
/// it (design D7).
#[test]
fn only_the_first_frame_carries_the_theme() {
    let mut test = test_core();
    test.tx.send(key_press()).unwrap();
    assert!(test.core.step());
    let first = test.frames.try_recv().unwrap();
    assert!(first.theme.is_some());
    test.waker.wake();
    assert!(test.core.step());
    let second = test.frames.try_recv().unwrap();
    assert!(second.theme.is_none());
}
