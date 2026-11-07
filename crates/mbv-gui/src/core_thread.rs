//! The GUI Client's core thread (design D3 of `add-tauri-gui-client`): one
//! synchronous thread creates the `App`, attaches to the Owner, and runs
//! `ClientCore::tick` in a loop behind the window.
//!
//! Tauri command handlers never touch the `App`. They push a [`CoreInput`]
//! value into the thread's channel, and the thread waits on that channel with
//! a timeout of `CLIENT_IDLE_CADENCE`, so an input, a `ClientWaker` wake, or
//! the idle cadence each produce one tick. Every tick that changed state, and
//! every `report_layout` changed or not, emits one `GuiFrame` through the
//! frame sink. `main` wires that sink to `AppHandle::emit`, the `mbv://frame`
//! event, and the commands push into the channel; the tests mock both, so no
//! Tauri runtime or tokio runs here (design D12).

use std::fmt;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mbv::{
    App, CLIENT_IDLE_CADENCE, ClientCore, ClientLifecycle, ClientTickHost, GuiInput,
    HomeEventDrain, PlayerEventDrain, TickOutcome, declare_gui_client, load_config,
};
use mbv_config::ensure_runtime_dir;
use mbv_surfaces::breakpoint::{BreakpointClass, BreakpointThresholds, classify};
use mbv_surfaces::gesture::{PointerGesture, PointerGestureState};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_surfaces::{FocusStack, HitMap};
use mbv_ui_model::queue_rows::queue_media_rows;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, KeyEvent, Msg, PointerEvent, Position, ServiceRequest};

use crate::artwork::ArtworkFrames;
use crate::attach::{AttachCampaign, LibraryOwnerSteps};
use crate::bridge::frame::{FrameSources, GuiFrame, GuiWindowFrame};
use crate::bridge::layout::{LayoutReport, ListId, QueueTargetId, RegionTarget, hit_map};
use crate::leaves::{PlaybackLeaf, QueueLeaf, StatusBarLeaf, TransportAvailability, gesture_point};

/// The viewport height before the first `report_layout` measures the window:
/// the frame carries every row the queue holds. The front end reports the
/// real capacity when it mounts (design D5), so the placeholder never
/// survives the first report.
const INITIAL_VIEWPORT_ROWS: usize = usize::MAX;

/// The GUI Client's breakpoint thresholds, in window logical points (design
/// D10). The implementer picks the values (the design's open question); they
/// change no spec. `mini_below` mirrors the TUI's 80-column mini threshold at
/// roughly nine points per column, and `wide_from` is the width a desktop
/// window needs before the later hero surfaces sit side by side.
const GUI_BREAKPOINTS: BreakpointThresholds<f32> = BreakpointThresholds {
    mini_below: 720.0,
    wide_from: 1200.0,
};

/// One message the core thread's channel carries (design D3): the front
/// end's translated input, a layout report, the wake value the installed
/// `ClientWaker` sends, or the window's close.
#[derive(Debug)]
pub enum CoreInput {
    /// A key chord, translated on the Rust side (design D6) and routed
    /// through `App::route_gui_input` with the focused surface.
    Key(KeyEvent),
    /// A pointer event, translated on the Rust side (design D4) and resolved
    /// against the slice hit map.
    Pointer(PointerEvent),
    /// A layout report from the front end. The frame it triggers emits
    /// whether or not a tick changed state (design D3); the report applies
    /// to the Surface state's capacity and hit map first.
    Layout(LayoutReport),
    /// The wake value the installed `ClientWaker` sends (design D3), so
    /// Owner events and timers wake the core as they wake the TUI.
    Wake,
    /// The window closed (design D9). The `main` window's `Destroyed` event
    /// delivers it (design D9's close delivery): the App's waker keeps the
    /// channel connected for the core thread's whole life, so a sender drop
    /// cannot carry the close, and the run ends on this input instead. The
    /// attached run answers it as a quit: the quit edge runs (a no-op on the
    /// window that is already gone), the run ends, and the run's exit
    /// boundary tears the shared state down before the `App`'s drop releases
    /// the Client.
    Close,
}

/// The sink every emitted frame goes through (design D3). Production wires
/// `AppHandle::emit` as the `mbv://frame` event; the tests mock it with a
/// channel.
pub type FrameSink = Box<dyn Fn(GuiWindowFrame) + Send + 'static>;

/// The edge that closes the window (design D9): the mismatch state's Quit and
/// a routed quit key's close, wired by `main` to the window's close. The tests
/// capture it with a channel.
pub type QuitSink = Box<dyn Fn() + Send + 'static>;

/// Start the core thread (design D3): it loads the configuration, attaches
/// to the Owner, and runs the tick loop for the life of the window. The
/// thread ends when the window closes — the `main` window's `Destroyed`
/// event delivers a [`CoreInput::Close`] through the returned sender, which
/// the Tauri commands push through; they never touch the `App` (design D3).
/// The App's waker keeps the channel connected for the thread's whole life,
/// so a sender drop cannot carry the close.
///
/// The caller joins the returned handle after the window's event loop ends:
/// the run's exit boundary runs the shared Client exit teardown, and the
/// join holds the process until it completes, so a quit or a window close
/// persists its state before the process exits.
#[must_use]
pub fn spawn(sink: FrameSink, quit: QuitSink) -> (Sender<CoreInput>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    // The waker's clone stays inside the `App`; the returned sender goes to
    // the commands' managed state.
    let waker_tx = tx.clone();
    let handle = std::thread::spawn(move || {
        if let Err(error) = ensure_runtime_dir() {
            tracing::error!(
                name: "gui_core.runtime_dir_failed",
                target: "gui_core",
                error = %error,
                "runtime directory unavailable; the core thread ends"
            );
            return;
        }
        let config = match load_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(
                    name: "gui_core.config_failed",
                    target: "gui_core",
                    error = %error,
                    "configuration failed to load; the core thread ends"
                );
                return;
            }
        };
        // Declare the GUI Client before the first Owner connection (design
        // D9): the campaign's connections send a plain Hello.
        declare_gui_client();
        let steps = LibraryOwnerSteps::new(&config);
        let outcome = {
            let mut campaign = AttachCampaign::new(&rx, &sink, &quit, &steps);
            campaign.run()
        };
        match outcome {
            Ok(app) => {
                let mut core = CoreThread::new(app, rx, sink, quit);
                core.install_waker(waker_tx);
                core.run();
            }
            Err(end) => tracing::info!(
                name: "gui_core.attach_ended",
                target: "gui_core",
                end = ?end,
                "the attach ended without an App"
            ),
        }
    });
    (tx, handle)
}

/// The core thread's state (design D3): the attached `App`, the first
/// slice's Surface state, and the channel every input and wake feeds.
pub struct CoreThread {
    app: App,
    focus: FocusStack,
    queue: QueueSurface,
    /// The Queue playback Surface state: the transport band's focus
    /// presentation and the seek throttle.
    playback: QueuePlaybackSurface,
    status_bar: StatusBarSurface,
    /// The recognized pointer gestures over the window's logical points (the
    /// GUI's `SamePlace` slop rule, design D4). One recognizer serves the
    /// whole window: the first slice's surfaces paint disjoint regions, and
    /// the TUI's per-parent recognizers all see the same event stream, so
    /// their click and drag states are the same facts.
    gestures: PointerGestureState<Position>,
    /// The painted regions of the last `LayoutReport` (design D4), in paint
    /// order: the semantic targets a pointer gesture resolves against across
    /// the whole slice. An empty map resolves nothing until the first report
    /// arrives.
    regions: HitMap<RegionTarget>,
    /// The whole Queue rows the viewport fits. Row 2.4 sets it from the
    /// `LayoutReport`; until then the frame carries every row.
    viewport_rows: usize,
    /// The window's breakpoint class. Row 2.4 classifies it from the reported
    /// width (design D10); the first slice presents neither Library nor hero,
    /// so `Wide` changes nothing visible until then.
    breakpoint: BreakpointClass,
    rx: Receiver<CoreInput>,
    sink: FrameSink,
    /// The edge that closes the window (design D9): a routed quit key closes
    /// the window through it, as the mismatch state's Quit does.
    quit: QuitSink,
    /// The inputs one step's wait collected, for the tick's input phase.
    pending: Vec<CoreInput>,
    /// A `report_layout` arrived this tick: the frame emits changed or not
    /// (design D3).
    frame_requested: bool,
    /// The frame that finds it armed carries the theme (design D7): the
    /// first frame, and every frame a `report_layout` triggers — the front
    /// end sends the report when it mounts, so a reload re-arms it.
    theme_pending: bool,
    /// The frames' artwork memo (design D10): the value a frame arms, and
    /// the encoded artwork a re-armed frame re-carries encode-free.
    artwork: ArtworkFrames,
    /// The wait between ticks. Production keeps `CLIENT_IDLE_CADENCE`; the
    /// step test zeroes it.
    idle_cadence: Duration,
}

impl CoreThread {
    /// A core over an attached `App` (design D3): it owns the first slice's
    /// Surface state and the channel every input and wake feeds.
    #[must_use]
    pub fn new(app: App, rx: Receiver<CoreInput>, sink: FrameSink, quit: QuitSink) -> Self {
        let mut focus = FocusStack::new();
        // The first slice mounts the Queue panel, the Queue playback band,
        // and the Status bar (design D4), and the Queue side holds focus.
        focus.mount(ComponentId::Queue);
        focus.mount(ComponentId::QueuePlaybackPanel);
        focus.mount(ComponentId::StatusBarPanel);
        focus.set_panel_focus(PanelFocus::Queue);
        let mut app = app;
        // The minimal Emby client fill (design D9's ruled exception): the
        // armed startup request becomes an authentication-only worker — no
        // library bootstrap — whose completion installs the client the
        // session list, cast discovery, artwork, and item-metadata fetches
        // read. Its drain is the tick's `drain_startup_workers` phase.
        app.start_emby_client_fill();
        Self {
            app,
            focus,
            queue: QueueSurface::new(),
            playback: QueuePlaybackSurface::new(),
            status_bar: StatusBarSurface::new(),
            gestures: PointerGestureState::new(),
            regions: HitMap::new(),
            viewport_rows: INITIAL_VIEWPORT_ROWS,
            breakpoint: BreakpointClass::Wide,
            rx,
            sink,
            quit,
            pending: Vec::new(),
            // The frame the attach hands over emits on the first step, changed
            // or not (design D9): the window must not keep the attach state it
            // showed before the attach succeeded.
            frame_requested: true,
            theme_pending: true,
            artwork: ArtworkFrames::default(),
            idle_cadence: CLIENT_IDLE_CADENCE,
        }
    }

    /// Install the wake the App's relays invoke (design D3): every wake feeds
    /// a [`CoreInput::Wake`] into the core's own channel, so Owner events and
    /// timers wake the core as they wake the TUI.
    pub fn install_waker(&self, tx: Sender<CoreInput>) {
        ClientCore::from_app(&self.app).install_waker(move || {
            let _ = tx.send(CoreInput::Wake);
        });
    }

    /// Adopt the wait cadence a test pins; production keeps
    /// `CLIENT_IDLE_CADENCE`. A zeroed cadence makes a step's wait
    /// non-blocking.
    pub fn set_idle_cadence_for_test(&mut self, cadence: Duration) {
        self.idle_cadence = cadence;
    }

    /// Apply one `LayoutReport` to the Surface state's capacity and the hit
    /// map (design D4, task row 2.4): the reported whole rows become the
    /// Queue viewport's height — the paging stride's source and the resting
    /// scroll's clamp — and the reported regions rebuild the combined slice
    /// hit map, the last painted regions a press resolves against. The
    /// reported width classifies into the shared breakpoint classes through
    /// the GUI's own thresholds (design D10).
    fn apply_layout(&mut self, report: &LayoutReport) {
        for list in &report.lists {
            match list.list {
                ListId::Queue => {
                    self.viewport_rows = list.whole_rows;
                    self.queue.set_viewport_height(list.whole_rows);
                }
            }
        }
        self.regions = hit_map(&report.regions);
        self.breakpoint = classify(report.width, GUI_BREAKPOINTS);
    }

    /// Route one translated key chord through the shared router with the
    /// focused surface as the leaf (design D4, task row 2.4). The first
    /// slice's only focusable surface is the Queue; with no focused surface
    /// no leaf can consume anything. Returns whether the `App` asked to
    /// quit, which the GUI Client answers by closing its window.
    fn route_key(&mut self, key: KeyEvent) -> bool {
        if self.focus.focused().as_ref() != Some(&ComponentId::Queue) {
            return self.app.route_gui_input(GuiInput::Key(key), None);
        }
        let Self {
            app,
            queue,
            regions,
            ..
        } = self;
        let mut leaf = QueueLeaf::new(queue, regions);
        app.route_gui_input(GuiInput::Key(key), Some(&mut leaf))
    }

    /// Route one translated pointer event (design D4, task row 2.4): disarm
    /// prefix mode first, recognize the gesture over the window's logical
    /// points, resolve the gesture's point against the combined hit map of
    /// the last `LayoutReport`, apply the focus change the matching TUI click
    /// applies, and hand the surface that owns the region to
    /// [`App::route_gui_pointer`]. A gesture whose point resolves to no
    /// reported region — and a `DragEnd`, which carries no point — resolves
    /// to nothing (`client-interaction-core`: pointer targets resolve from a
    /// semantic hit map); the disarm precedes all of it, so every pointer
    /// event cancels an armed prefix even when nothing resolves.
    fn route_pointer(&mut self, event: PointerEvent) {
        // Any pointer event disarms prefix mode first — the shared contract
        // `App::route_gui_input`'s pointer arm carries, the same silent
        // disarm every terminal mouse event applies in the TUI. The call
        // runs before the gesture and region resolution, so a hover move, a
        // drag end, or a press on no reported region still cancels an armed
        // prefix; the pointer arm itself delivers nothing else.
        self.app.route_gui_input(GuiInput::Pointer(event), None);
        let now = Instant::now();
        let Some(gesture) =
            self.gestures
                .recognize(event.kind, event.modifiers, event.position, now)
        else {
            return;
        };
        let Some(point) = gesture_point(&gesture) else {
            return;
        };
        match self.regions.resolve(point) {
            Some(RegionTarget::Queue(target)) => {
                // The TUI click's focus change (design D4): a click, double
                // click, or right click on a queue row runs the TUI single
                // click's effects, one of which focuses the Queue panel. The
                // dispatched `QueueRowClick` and `QueueRowActivate` apply the
                // App-side half; the core mirrors the panel focus on its own
                // stack — whose derived focus the frame carries and the next
                // keys' routing reads — because the right click's
                // `RowContextMenu` dispatch is the documented no-op. The pill
                // and the wheel take no focus, exactly as the TUI's matching
                // gestures do not.
                if matches!(
                    gesture,
                    PointerGesture::Click { .. }
                        | PointerGesture::DoubleClick(_)
                        | PointerGesture::RightClick(_)
                ) && matches!(target, QueueTargetId::Slot(_))
                {
                    self.focus.set_panel_focus(PanelFocus::Queue);
                }
                let Self {
                    app,
                    queue,
                    regions,
                    ..
                } = self;
                let mut leaf = QueueLeaf::new(queue, regions);
                app.route_gui_pointer(gesture, Some(&mut leaf));
            }
            Some(RegionTarget::QueuePlayback(_)) => {
                // The TUI's playback-panel click applies no focus change, so
                // the core applies none: the press works while the Queue
                // panel holds focus (design D4).
                let availability = self.app.queue_playback_projection().availability;
                let Self {
                    app,
                    playback,
                    regions,
                    ..
                } = self;
                let mut leaf = PlaybackLeaf::new(
                    playback,
                    regions,
                    TransportAvailability {
                        stop: availability.stop,
                        previous: availability.previous,
                        next: availability.next,
                    },
                );
                app.route_gui_pointer(gesture, Some(&mut leaf));
            }
            Some(RegionTarget::StatusBar(_)) => {
                // The TUI's status-row click applies no focus change either.
                let Self { app, regions, .. } = self;
                let mut leaf = StatusBarLeaf::new(regions);
                app.route_gui_pointer(gesture, Some(&mut leaf));
            }
            // A pre-attach button region is the attach campaign's target: the
            // attached slice resolves nothing for it, even when a stale
            // report carried one — and so does a point in no reported region.
            Some(RegionTarget::Attach(_)) | None => {}
        }
    }

    /// Run the loop (design D3): wait for input, wake, or the idle cadence,
    /// then tick. The loop ends when the window closes — its close arrives
    /// as a [`CoreInput::Close`], and a routed quit key answers the same
    /// way — or when the channel itself ends.
    ///
    /// The run's exit boundary runs the shared Client exit teardown once,
    /// after the loop, whichever edge ended it — a routed quit, the window's
    /// close, or a channel end: the deferred settings save flushes, the
    /// auto-reconnect record persists, and the daemon-lifetime policy applies
    /// (daemon-lifecycle "Quitting a client follows Stay Alive and offers no
    /// detach"). The join in `main` holds the process until it completes, so
    /// the teardown survives the window's close.
    pub fn run(&mut self) {
        while self.step() {}
        self.app.client_exit_teardown();
    }

    /// One step: wait for the next input, run one tick, and emit the frame a
    /// changed state or a `report_layout` asks for. Returns whether the loop
    /// continues.
    pub fn step(&mut self) -> bool {
        match self.rx.recv_timeout(self.idle_cadence) {
            Ok(input) => self.pending.push(input),
            // The idle cadence keeps the maintenance phases running with no
            // input queued.
            Err(RecvTimeoutError::Timeout) => {}
            // The channel itself ended: the same consequence as the window's
            // close, and unreachable while the App's waker holds its sender.
            Err(RecvTimeoutError::Disconnected) => return false,
        }
        let outcome = ClientCore::tick(self, Instant::now());
        if outcome.quit_requested {
            // The routed quit closes the window through the same edge the
            // mismatch state's Quit uses (design D9).
            (self.quit)();
            return false;
        }
        // A restart re-reads the landed Owner snapshot next iteration; the
        // frame waits for it, as the TUI's run loop does.
        if !outcome.restart_requested && (outcome.input_received || self.frame_requested) {
            self.frame_requested = false;
            self.emit_frame();
        }
        true
    }

    /// Adopt the Queue projection into the Queue Surface state (design D5),
    /// the consumption the TUI's sync pass runs once per pass, before the
    /// frame that reads the Surface state builds.
    fn sync_queue_surface(&mut self) {
        let update = self.app.queue_surface_projection();
        if let Some(slots) = update.slots {
            self.queue.list_mut().set_content(queue_media_rows(
                &slots,
                update.playback,
                update.pending_slot,
            ));
        } else if let Some((target, row)) = update.patch {
            self.queue.list_mut().patch_row(&target, row);
        }
        self.queue.set_cursor(&update.cursor);
        self.queue.set_link(update.link);
    }

    /// Build and emit the frame the window paints (design D4): the frame
    /// carries the theme when armed — the first frame, and every
    /// `report_layout`-triggered frame (design D7) — and the visual slot's
    /// artwork when the slot's art changed for this window (design D10).
    fn emit_frame(&mut self) {
        self.sync_queue_surface();
        // The active item's metadata fetch (design D4a) issues from the
        // frame path, deduped on its own loading set; a landed completion
        // rides the next frame.
        self.app.fetch_playback_metadata();
        let artwork = self.app.queue_card_artwork();
        let artwork = self.artwork.frame_value(&artwork);
        let theme_changed = std::mem::take(&mut self.theme_pending);
        let frame = GuiFrame::build(&FrameSources {
            app: &self.app,
            focus: &self.focus,
            queue: &self.queue,
            viewport_rows: self.viewport_rows,
            breakpoint: self.breakpoint,
            theme_changed,
            artwork,
        });
        (self.sink)(GuiWindowFrame::Slice(Box::new(frame)));
    }
}

impl fmt::Debug for CoreThread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreThread")
            .field("app", &self.app)
            .field("focus", &self.focus)
            .field("queue", &self.queue)
            .field("playback", &self.playback)
            .field("status_bar", &self.status_bar)
            .field("regions", &self.regions.len())
            .field("viewport_rows", &self.viewport_rows)
            .field("breakpoint", &self.breakpoint)
            .field("pending", &self.pending.len())
            .field("frame_requested", &self.frame_requested)
            .field("theme_pending", &self.theme_pending)
            .field("idle_cadence", &self.idle_cadence)
            .finish_non_exhaustive()
    }
}

impl ClientTickHost for CoreThread {
    fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    fn drain_startup_workers(&mut self) -> bool {
        // The GUI presents no Home panel; a completion's rebuilt Home content
        // drops here — the App-side handlers already applied the semantics.
        self.app.drain_startup_workers().events
    }

    fn reproject_audiobookshelf_content(&mut self) {
        // The first slice presents no Audiobookshelf surface; the App-side
        // drain already applied the event's semantics.
    }

    fn drain_suspended_home_events(&mut self) -> bool {
        let mut had_events = false;
        loop {
            match self.app.drain_home_event() {
                HomeEventDrain::Empty => return had_events,
                // The GUI Client never joins the Pin swap —
                // `declare_gui_client` sends a plain Hello — so the
                // lifecycle arms consume their event with no action.
                HomeEventDrain::Consumed
                | HomeEventDrain::Client(
                    ClientLifecycle::SwapPrepare | ClientLifecycle::SwapQuit,
                ) => had_events = true,
            }
        }
    }

    fn drain_player_events(&mut self, input_received: &mut bool) -> bool {
        match self.app.drain_player_event() {
            PlayerEventDrain::Empty => false,
            // The GUI Client never joins the Pin swap (see the home drain).
            PlayerEventDrain::Client(ClientLifecycle::SwapPrepare | ClientLifecycle::SwapQuit) => {
                *input_received = true;
                false
            }
            PlayerEventDrain::Handled { restart } => {
                // The snapshot adoption ran App-side; the next frame's Queue
                // sync re-projects the adopted state (design D5).
                *input_received = true;
                restart
            }
        }
    }

    fn drain_lib_events(&mut self) -> bool {
        // The App-owned half applies; the GUI keeps no library content state,
        // so no surface re-projection joins the drain.
        let mut had_events = false;
        while self.app.drain_lib_event() {
            had_events = true;
        }
        had_events
    }

    fn drain_search_results(&mut self) -> bool {
        // The GUI mounts no search surface, so nothing sends on the search
        // channel for this Client.
        false
    }

    fn tick_search_clock(&mut self, _now: Instant) -> Option<Msg> {
        // No search control holds a debounce deadline in the first slice.
        None
    }

    fn handle_service_request(&mut self, _request: ServiceRequest) {
        // Unreachable while `tick_search_clock` emits nothing: the search
        // debounce is the only Service request the core routes.
    }

    fn tick_inline_search_clock(&mut self, _now: Instant) -> bool {
        // The first slice presents no Inline Search control.
        false
    }

    fn reproject_feed_content(&mut self) {
        // The first slice presents no feed surface.
    }

    fn drain_feed_add_results(&mut self) -> bool {
        // The GUI opens no feed-add flow, so nothing sends on its channel.
        false
    }

    fn drain_card_image_completions(&mut self) -> bool {
        // The visual slot's artwork paints now (design D10): the slot's
        // fetches resolve through this drain, so a completion's decoded
        // image lands in the cache and the next frame carries it.
        self.app.drain_card_image_completions()
    }

    fn drain_resize_responses(&mut self) -> bool {
        // The GUI issues no image resize request (the TUI's resize drag owns
        // them), so the response channel stays empty.
        false
    }

    fn drain_transport_events(&mut self) -> bool {
        self.app.drain_transport_events()
    }

    fn drain_ws_events(&mut self) -> bool {
        let mut had_events = false;
        while self.app.drain_ws_event() {
            had_events = true;
        }
        had_events
    }

    fn drain_audiobookshelf_socket_events(&mut self) -> bool {
        let mut had_events = false;
        while self.app.drain_audiobookshelf_socket_event() {
            had_events = true;
        }
        had_events
    }

    fn drain_idle_feed(&mut self) -> bool {
        // The GUI never initializes the idle feed; the App-side drain is a
        // no-op without one.
        self.app.drain_idle_feed()
    }

    fn run_periodic_maintenance(&mut self) {
        self.app.run_periodic_maintenance();
    }

    fn tick_input(&mut self, outcome: &mut TickOutcome) {
        // Take the collected inputs out first: the routing borrows the
        // whole `self`, which the drain's field borrow would conflict with.
        for input in std::mem::take(&mut self.pending) {
            match input {
                CoreInput::Wake => outcome.input_received = true,
                CoreInput::Key(key) => {
                    // The chord routes through `App::route_gui_input` with
                    // the focused surface as the leaf (design D4); a routed
                    // quit ends the run.
                    outcome.input_received = true;
                    if self.route_key(key) {
                        outcome.quit_requested = true;
                    }
                }
                CoreInput::Pointer(event) => {
                    // The gesture resolves against the slice hit map and
                    // routes through `App::route_gui_pointer` with the
                    // surface that owns the region (design D4).
                    outcome.input_received = true;
                    self.route_pointer(event);
                }
                CoreInput::Layout(report) => {
                    // A `report_layout` always emits the current frame,
                    // changed or not (design D3), and re-arms the theme: the
                    // front end sends the report when it mounts, so the
                    // frame after a reload carries the theme again (design
                    // D7). The report applies to the capacity and the hit
                    // map first.
                    self.apply_layout(&report);
                    self.frame_requested = true;
                    self.theme_pending = true;
                    // The reload forgets the front end's artwork too: the
                    // next frame re-carries it, encode-free (design D10).
                    self.artwork.rearm();
                }
                CoreInput::Close => {
                    // The window closed (design D9): the run ends as a quit
                    // does, so the same edge runs — a no-op on the window
                    // that is already gone — and the run's exit boundary
                    // tears the shared state down before the App's drop
                    // releases the Client.
                    outcome.quit_requested = true;
                    break;
                }
            }
        }
    }
}
