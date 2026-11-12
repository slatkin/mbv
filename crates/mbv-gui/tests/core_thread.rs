//! Public-API tests of the GUI core thread (`mbv_gui::core_thread`): the
//! step unit the run loop drives, over a mocked channel and a mocked section
//! sink (design D13 — no Tauri runtime, no webview, no live Owner).
//!
//! The sink receives the section batches a tick sent (design D1). The
//! [`Held`] view merges them the way the front end's stores do, so a test
//! reads what the window holds: a section a batch omits keeps its last value.
//!
//! The pointer half — the gestures a `CoreInput::Pointer` recognizes and
//! resolves against the reported regions — lives in
//! `core_thread/pointer.rs`. The inline `mod` makes the child files
//! resolve under `tests/core_thread/` (one binary per crate; no `mod.rs`).

use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use mbv::{
    App, load_config, stub_app_with_local_daemon_events, stub_app_with_pending_settings_save,
    stub_app_with_playing_queue, stub_app_with_queue,
};
use mbv_config::{
    LaunchPanelFocus, TUI_LAUNCH_STATE_VERSION, TabIdentity, TestStateDirGuard, TuiLaunchState,
    gui_launch_state_path, load_tui_launch_state, save_tui_launch_state,
};
use mbv_emby::EmbyClient;
use mbv_gui::bridge::frame::GuiPanelToggles;
use mbv_gui::bridge::frame::chrome::GuiStatusBar;
use mbv_gui::bridge::frame::focus::GuiFocus;
use mbv_gui::bridge::frame::library::{GuiLibraryContent, GuiSearchFrame};
use mbv_gui::bridge::frame::overlay::GuiOverlayFrame;
use mbv_gui::bridge::frame::playback::{GuiQueuePlayback, GuiScrubberHover};
use mbv_gui::bridge::frame::queue::{GuiQueue, GuiQueueSummary};
use mbv_gui::bridge::frame::settings::GuiSettingsFrame;
use mbv_gui::bridge::layout::{
    LayoutRect, LayoutRegion, LayoutReport, LayoutSize, ListId, ListRows, RegionTarget,
};
use mbv_gui::bridge::sections::{GuiSection, GuiSectionBatch};
use mbv_gui::core_thread::{CoreInput, CoreThread, QuitSink, SectionSink};
use mbv_net::mock_http::MockHttp;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PointerEvent, PointerKind, Position};

mod core_thread {
    mod home;
    mod launch;
    mod library;
    mod overlay;
    mod pointer;
    mod queue;
    mod sections;
    mod settings;
}

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
pub(crate) const TEST_URL: &str = "http://127.0.0.1:1";

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

/// The sink sent no batch: the channel is empty while the core's sink sender
/// is still alive.
fn assert_no_batch(batches: &mpsc::Receiver<GuiSectionBatch>) {
    assert_eq!(batches.try_recv(), Err(TryRecvError::Empty));
}

/// One section's value from a batch's section field. A section that was never
/// sent names the missing section in the panic.
fn section_value<'a, T>(section: Option<&'a GuiSection<T>>, name: &str) -> &'a T {
    &section
        .unwrap_or_else(|| panic!("the window holds no {name} section"))
        .value
}

/// One optional section's value: a section that carries `null`, or a section
/// the window never received, reads as `None`.
fn optional_value<T>(section: Option<&GuiSection<Option<T>>>) -> Option<&T> {
    section.and_then(|section| section.value.as_ref())
}

/// What the window holds: every batch the core sent, merged the way the
/// front end's stores merge them. A present section replaces the held one;
/// an absent section keeps it (design D1).
#[derive(Debug, Clone, Default)]
pub(crate) struct Held {
    batch: GuiSectionBatch,
}

impl Held {
    /// Merge one batch into the held sections.
    fn merge(&mut self, batch: GuiSectionBatch) {
        let held = &mut self.batch;
        held.attach = batch.attach.or(held.attach.take());
        held.theme = batch.theme.or(held.theme.take());
        held.focus = batch.focus.or(held.focus.take());
        held.queue = batch.queue.or(held.queue.take());
        held.summary = batch.summary.or(held.summary.take());
        held.panel = batch.panel.or(held.panel.take());
        held.overlay = batch.overlay.or(held.overlay.take());
        held.playback = batch.playback.or(held.playback.take());
        held.hover = batch.hover.or(held.hover.take());
        held.footer = batch.footer.or(held.footer.take());
        held.status = batch.status.or(held.status.take());
        held.library = batch.library.or(held.library.take());
        held.search = batch.search.or(held.search.take());
        held.settings = batch.settings.or(held.settings.take());
    }

    /// The panel section's value: the toggles and the UI-reset counter.
    pub(crate) fn panel(&self) -> &GuiPanelToggles {
        section_value(self.batch.panel.as_ref(), "panel")
    }

    /// The Queue section's value.
    pub(crate) fn queue(&self) -> &GuiQueue {
        section_value(self.batch.queue.as_ref(), "queue")
    }

    /// The queue summary section's value.
    pub(crate) fn summary(&self) -> &GuiQueueSummary {
        section_value(self.batch.summary.as_ref(), "summary")
    }

    /// The focus section's value.
    pub(crate) fn focus(&self) -> &GuiFocus {
        section_value(self.batch.focus.as_ref(), "focus")
    }

    /// The open overlay, `None` while none is open.
    pub(crate) fn overlay(&self) -> Option<&GuiOverlayFrame> {
        optional_value(self.batch.overlay.as_ref())
    }

    /// The transport band's section value.
    pub(crate) fn playback(&self) -> &GuiQueuePlayback {
        section_value(self.batch.playback.as_ref(), "playback")
    }

    /// The scrubber's hover preview, `None` while the pointer is away.
    pub(crate) fn hover(&self) -> Option<&GuiScrubberHover> {
        optional_value(self.batch.hover.as_ref())
    }

    /// The Status bar section's value.
    pub(crate) fn status(&self) -> &GuiStatusBar {
        section_value(self.batch.status.as_ref(), "status")
    }

    /// The Library content, `None` while no presented destination is active.
    pub(crate) fn library(&self) -> Option<&GuiLibraryContent> {
        optional_value(self.batch.library.as_ref())
    }

    /// The Search section, `None` while the field is closed.
    pub(crate) fn search(&self) -> Option<&GuiSearchFrame> {
        optional_value(self.batch.search.as_ref())
    }

    /// The Settings screen, `None` while the screen is closed.
    pub(crate) fn settings(&self) -> Option<&GuiSettingsFrame> {
        optional_value(self.batch.settings.as_ref())
    }
}

/// A core thread over a stub queue of `slots`: the sender for its channel,
/// the capture end of its section sink, and the App's waker. The wait cadence
/// is zeroed, so a step with nothing queued is non-blocking.
pub(crate) struct TestCore {
    /// The scratch state directory the construction read: the core thread
    /// reads the GUI launch state at construction, so a test without its own
    /// guard must not restore the real machine's saved location — the
    /// first-group rule would then start a fetch the test never scripted.
    /// `None` when the test pinned its own guard before building the core.
    _state_dir: Option<TestStateDirGuard>,
    pub(crate) core: CoreThread,
    pub(crate) tx: mpsc::Sender<CoreInput>,
    waker: mbv::ClientWaker,
    batches: mpsc::Receiver<GuiSectionBatch>,
    held: Held,
    quits: mpsc::Receiver<()>,
}

impl TestCore {
    /// The next batch the core sent, merged into the window's held sections.
    /// Panics when the core sent none.
    pub(crate) fn next_batch(&mut self) -> GuiSectionBatch {
        let batch = self.batches.try_recv().expect("the core sent a batch");
        self.held.merge(batch.clone());
        batch
    }

    /// What the window holds after every batch the core has sent: the batches
    /// not yet read are merged first. A test that pumps steps between commands
    /// reads the state the last step left.
    pub(crate) fn held(&mut self) -> Held {
        while let Ok(batch) = self.batches.try_recv() {
            self.held.merge(batch);
        }
        self.held.clone()
    }
}

pub(crate) fn test_core(slots: usize) -> TestCore {
    test_core_with(stub_app_with_queue(slots))
}

/// The Emby client the GUI Music tests seed their library with (design D13):
/// built over the in-memory mock transport, so no request leaves the process.
pub(crate) fn test_emby_client(http: &MockHttp) -> EmbyClient {
    EmbyClient::new(mbv::Config {
        server_url: TEST_URL.into(),
        ..mbv::Config::default()
    })
    .with_test_agent(http.agent())
}

/// A core thread over a caller-built `App`, for the stubs the queue stub
/// does not cover.
pub(crate) fn test_core_with(app: App) -> TestCore {
    let waker = app.client_waker();
    let (tx, rx) = mpsc::channel();
    let (batch_tx, batch_rx) = mpsc::channel();
    let sink: SectionSink = Box::new(move |batch| {
        let _ = batch_tx.send(batch);
    });
    let (quit_tx, quit_rx) = mpsc::channel();
    let quit: QuitSink = Box::new(move || {
        let _ = quit_tx.send(());
    });
    // A test that pins its own guard keeps it: the guard installs only when
    // this thread has none, so an explicitly seeded launch state stands.
    let state_dir_ownership = TestStateDirGuard::new_if_unset();
    let mut core = CoreThread::new(app, rx, sink, quit);
    core.set_idle_cadence_for_test(Duration::ZERO);
    // Every core-thread test audits its section marks (design D3): a clean
    // gated section must match its fresh build, so a missed mark fails here.
    core.set_section_audit_for_test(true);
    core.install_waker(tx.clone());
    TestCore {
        _state_dir: state_dir_ownership,
        core,
        tx,
        waker,
        batches: batch_rx,
        held: Held::default(),
        quits: quit_rx,
    }
}

/// The first step sends the queue rows the projection pushed (design D2): the
/// queue section is built on the first tick, whatever the input.
#[test]
fn the_first_batch_carries_the_queue_rows() {
    let mut test = test_core(2);
    test.tx.send(key_press()).unwrap();
    assert!(test.core.step());
    assert_eq!(test.held().queue().rows.len(), 2);
}

/// An idle tick with nothing changed sends no batch (design D2): the timeout
/// tick builds the compared sections and finds each equal to its last value.
#[test]
fn an_idle_tick_with_nothing_changed_sends_no_batch() {
    let mut test = test_core(2);
    assert!(test.core.step());
    test.next_batch();

    assert!(test.core.step());
    assert_no_batch(&test.batches);
}

/// A layout report that moves nothing sends nothing (design D4): the report
/// applies the layout and marks no section, so an unchanged layout builds to
/// the values the window holds.
#[test]
fn a_layout_report_that_moves_nothing_sends_nothing() {
    let mut test = test_core(2);
    assert!(test.core.step());
    test.next_batch();

    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    assert_no_batch(&test.batches);
}

/// The theme section rides the first batch and is not sent again while the
/// palette is unchanged (design D2's gated theme).
#[test]
fn the_theme_section_is_sent_once() {
    let mut test = test_core(2);
    assert!(test.core.step());
    assert!(test.next_batch().theme.is_some());

    test.waker.wake();
    assert!(test.core.step());
    assert_no_batch(&test.batches);
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
    test.held();

    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::PageDown,
            KeyModifiers::NONE,
        )))
        .unwrap();
    assert!(test.core.step());
    let held = test.held();
    assert_eq!(held.queue().cursor, Some(14));
    assert_eq!(held.queue().rows.len(), 14);
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

/// An announced Owner shutdown ends the run through the quit edge
/// (`daemon-disconnect-handling` "Clients exit cleanly on an announced
/// shutdown"): the announce's exit arm sets the run-loop quit signal the
/// TUI's `'outer` loop breaks on (`quit_requested`), and the GUI Client's
/// run takes the same orderly exit — one quit edge (the window closes),
/// and the run's exit boundary saves the GUI's launch location before the
/// teardown. The announce is injected on the stub Owner's event channel; no
/// live Owner, window, or timer runs.
///
/// Regression for the user report of 2026-11-08: an Owner exit left the GUI
/// window open, the run still ticking a dead Owner link.
#[test]
fn an_announced_owner_shutdown_ends_the_run_and_saves_the_launch_location() {
    let _guard = TestStateDirGuard::new();
    let (app, events) = stub_app_with_local_daemon_events(2);
    events
        .send(mbv_ctrl::player::PlayerEvent::DaemonShutdownAnnounced)
        .unwrap();
    let mut test = test_core_with(app);
    assert!(
        !gui_launch_state_path().exists(),
        "nothing writes the GUI launch location while the window is open"
    );

    test.core.run();

    test.quits.try_recv().expect("the quit edge fired");
    test.quits
        .try_recv()
        .expect_err("the run ended on one quit edge");
    assert!(
        gui_launch_state_path().exists(),
        "the run's exit boundary saved the GUI launch location"
    );
}

/// The visual slot's artwork rides the playback section when the image is
/// cached (design D10): the first batch carries the cached image's own bytes
/// as a `data:` URL, and a tick that changes nothing sends no playback
/// section.
#[test]
fn a_cached_artwork_rides_the_playback_section() {
    let mut app = stub_app_with_playing_queue(1);
    app.enable_card_images_for_test();
    app.seed_card_image_for_test(
        mbv_images::emby_queue_landscape_cache_key("id"),
        image::DynamicImage::new(4, 4, image::ColorType::Rgb8),
    );
    let mut test = test_core_with(app);
    assert!(test.core.step());
    match test.held().playback().artwork.clone() {
        Some(mbv_gui::bridge::frame::playback::GuiCardArtwork::Image(detail)) => {
            assert!(detail.starts_with("data:image/png;base64,"));
        }
        artwork => panic!("the window holds the cached artwork: {artwork:?}"),
    }
    test.waker.wake();
    assert!(test.core.step());
    assert_no_batch(&test.batches);
}
