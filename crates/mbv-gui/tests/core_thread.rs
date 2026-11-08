//! Public-API tests of the GUI core thread (`mbv_gui::core_thread`): the
//! step unit the run loop drives, over a mocked channel and a mocked frame
//! sink (design D13 — no Tauri runtime, no webview, no live Owner).
//!
//! The pointer half — the gestures a `CoreInput::Pointer` recognizes and
//! resolves against the reported regions — lives in
//! `core_thread/pointer.rs`. The inline `mod` makes the child files
//! resolve under `tests/core_thread/` (one binary per crate; no `mod.rs`).

use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use mbv::{
    App, load_config, stub_app_with_pending_settings_save, stub_app_with_playing_queue,
    stub_app_with_queue,
};
use mbv_config::{
    LaunchPanelFocus, TUI_LAUNCH_STATE_VERSION, TabIdentity, TestStateDirGuard, TuiLaunchState,
    load_tui_launch_state, save_tui_launch_state,
};
use mbv_gui::bridge::frame::playback::GuiCardArtwork;
use mbv_gui::bridge::frame::{GuiFrame, GuiWindowFrame};
use mbv_gui::bridge::layout::{
    LayoutRect, LayoutRegion, LayoutReport, LayoutSize, ListId, ListRows, RegionTarget,
};
use mbv_gui::core_thread::{CoreInput, CoreThread, FrameSink, QuitSink};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PointerEvent, PointerKind, Position};

mod core_thread {
    mod overlay;
    mod pointer;
    mod queue;
}

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
        container_size: LayoutSize::default(),
    })
}

/// A layout report the front end measured: `capacity` whole queue rows, one
/// painted region, and the panel container's size, as a report carries for a
/// measured window.
pub(crate) fn measured_layout(capacity: usize, regions: Vec<LayoutRegion>) -> CoreInput {
    CoreInput::Layout(LayoutReport {
        lists: vec![ListRows {
            list: ListId::Queue,
            whole_rows: capacity,
        }],
        regions,
        container_size: LayoutSize {
            width: 800.0,
            height: 600.0,
        },
    })
}

/// A painted region for `target` at the given logical-pixel rectangle.
pub(crate) fn region_at(x: f32, y: f32, target: RegionTarget) -> LayoutRegion {
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
pub(crate) fn press_at(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::Press,
        Position { x, y },
        KeyModifiers::NONE,
    ))
}

/// A wheel notch downwards, as the translated `pointer` command carries it.
pub(crate) fn wheel_down_at(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::ScrollDown,
        Position { x, y },
        KeyModifiers::NONE,
    ))
}

/// A pointer move with no button held, as the translated `pointer` command
/// carries it.
pub(crate) fn move_at(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::Move,
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
pub(crate) struct TestCore {
    pub(crate) core: CoreThread,
    pub(crate) tx: mpsc::Sender<CoreInput>,
    waker: mbv::ClientWaker,
    frames: mpsc::Receiver<GuiWindowFrame>,
    quits: mpsc::Receiver<()>,
}

impl TestCore {
    /// The next emitted frame's slice, as every core-thread frame is one.
    pub(crate) fn next_frame(&self) -> GuiFrame {
        match self.frames.try_recv().expect("the core emitted a frame") {
            GuiWindowFrame::Slice(frame) => *frame,
            GuiWindowFrame::Attach(..) => panic!("the core thread emitted an attach state"),
        }
    }
}

pub(crate) fn test_core(slots: usize) -> TestCore {
    test_core_with(stub_app_with_queue(slots))
}

/// A core thread over a caller-built `App`, for the stubs the queue stub
/// does not cover.
pub(crate) fn test_core_with(app: App) -> TestCore {
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

/// A timeout tick whose render cadence elapsed emits the frame (design D3's
/// changed-state rule): the Owner applies the playhead's progress without an
/// event the core's drains see, so the shared cadence is what keeps the panel
/// moving while the window takes no input. Regression for the user report of
/// 2026-11-07: the now-playing panel froze until the window had focus, the
/// pointer moves being the only inputs a frame followed.
#[test]
fn a_cadence_due_timeout_tick_emits_the_frame() {
    let mut test = test_core(2);
    assert!(test.core.step());
    test.next_frame();

    // The cadence interval (1 s idle) elapsed since the last frame; the step
    // takes no input — the recv timeout only.
    test.core
        .set_last_frame_for_test(Instant::now().checked_sub(Duration::from_secs(2)).unwrap());
    assert!(test.core.step());
    test.next_frame();
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

/// A routed quit key fires the quit edge and ends the run (design D9): the
/// window closes through the same edge the mismatch state's Quit uses. The
/// chord is a rebound `quit` binding: the GUI-only key context captures the
/// default `q` for the queue fold (add-tauri-gui-client row 6.2), so the
/// quit mechanism routes on a chord the context does not claim — the same
/// configured-`[keys]`-override path a user's rebind takes.
#[test]
fn a_routed_quit_key_fires_the_quit_edge_and_ends_the_run() {
    let mut test = test_core_with(mbv::stub_app_with_rebound_quit("Shift+Q"));
    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::Char('Q'),
            KeyModifiers::SHIFT,
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

/// The run's exit boundary runs the shared Client exit teardown: a window
/// close flushes the pending settings save at the boundary. The stub arms
/// the save with a deadline far in the future, so only the boundary's flush
/// can have persisted it — the periodic maintenance the ticks run must not
/// win the race (the GUI exit persistence gap this wiring closes).
#[test]
fn the_window_close_flushes_the_pending_settings_save() {
    let _guard = TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_pending_settings_save());
    test.tx.send(CoreInput::Close).unwrap();
    test.core.run();
    test.quits.try_recv().expect("the quit edge fired");
    assert!(load_config().unwrap().consume_audio);
}

/// The GUI's exit never writes the TUI's launch snapshot: the snapshot is
/// the shell's alone (`tui-launch-state`), and the shared state dir makes a
/// GUI write a clobber of the next TUI launch's restored location. The
/// snapshot a test seeds survives the window's close.
#[test]
fn the_window_close_leaves_the_tui_launch_state_untouched() {
    let _guard = TestStateDirGuard::new();
    save_tui_launch_state(&TuiLaunchState {
        version: TUI_LAUNCH_STATE_VERSION,
        tab: TabIdentity::Home,
        panel_focus: LaunchPanelFocus::Library,
        selector: None,
        item: None,
    })
    .unwrap();
    let mut test = test_core(2);
    test.tx.send(CoreInput::Close).unwrap();
    test.core.run();
    assert_eq!(
        load_tui_launch_state().map(|state| state.tab),
        Some(TabIdentity::Home)
    );
}

/// The visual slot's artwork rides the frame when the image is cached
/// (design D10): the first frame carries the display encode as a `data:`
/// URL, and a later frame arms it again only when the art changed.
#[test]
fn a_cached_artwork_rides_the_frame_once() {
    let mut app = stub_app_with_playing_queue(1);
    app.enable_card_images_for_test();
    app.seed_card_image_for_test(
        mbv_images::emby_queue_landscape_cache_key("id"),
        image::DynamicImage::new(4, 4, image::ColorType::Rgb8),
    );
    let mut test = test_core_with(app);
    assert!(test.core.step());
    let frame = test.next_frame();
    match frame.playback.artwork {
        Some(GuiCardArtwork::Image(detail)) => {
            assert!(detail.starts_with("data:image/jpeg;base64,"));
        }
        artwork => panic!("the frame carries the cached artwork: {artwork:?}"),
    }
    test.waker.wake();
    assert!(test.core.step());
    let second = test.next_frame();
    assert!(second.playback.artwork.is_none());
}
