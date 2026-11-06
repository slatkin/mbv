//! The GUI Client's core thread (design D3 of `add-tauri-gui-client`): one
//! synchronous thread creates the `App`, attaches to the Owner, and runs
//! `ClientCore::tick` in a loop behind the window.
//!
//! Tauri command handlers never touch the `App`. They push a [`CoreInput`]
//! value into the thread's channel, and the thread waits on that channel with
//! a timeout of `CLIENT_IDLE_CADENCE`, so an input, a `ClientWaker` wake, or
//! the idle cadence each produce one tick. Every tick that changed state, and
//! every `report_layout` changed or not, emits one `GuiFrame` through the
//! frame sink. Row 2.5 wires the sink to `AppHandle::emit`; the tests mock
//! it, so no Tauri runtime or tokio runs here (design D12).

use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use mbv::{
    App, CLIENT_IDLE_CADENCE, ClientCore, ClientLifecycle, ClientTickHost, Config, GuiInput,
    HomeEventDrain, OwnerStartOptions, OwnerState, PlayerEventDrain, TickOutcome, attach_owner,
    declare_gui_client, find_owner, load_config, start_owner,
};
use mbv_config::ensure_runtime_dir;
use mbv_remote_player::RemotePlayerError;
use mbv_surfaces::breakpoint::{BreakpointClass, BreakpointThresholds, classify};
use mbv_surfaces::gesture::{PointerGesture, PointerGestureState};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_surfaces::{FocusStack, HitMap, HitRegion};
use mbv_ui_model::queue_rows::queue_media_rows;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, KeyEvent, Msg, PointerEvent, Position, ServiceRequest};

use crate::bridge::frame::{FrameSources, GuiFrame};
use crate::bridge::layout::{LayoutReport, ListId, QueueTargetId, RegionTarget};
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
/// end's translated input, a layout report, or the wake value the installed
/// `ClientWaker` sends into the same channel.
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
}

/// The sink every emitted frame goes through (design D3). Production wires
/// `AppHandle::emit` (row 2.5); the tests mock it with a channel.
pub type FrameSink = Box<dyn Fn(GuiFrame) + Send + 'static>;

/// Why the happy-path Owner attach failed (design D9). Row 2.6 owns the
/// window state each outcome shows and the texts it names; until that row
/// lands the core thread logs the error and ends.
#[derive(Debug)]
enum AttachError {
    /// The single-instance probe itself failed.
    Probe(std::io::Error),
    /// A live Owner holds the lock but does not accept connections.
    Refused,
    /// The Owner process could not start.
    Start(RemotePlayerError),
    /// The attach to a running Owner failed.
    Attach(RemotePlayerError),
}

impl fmt::Display for AttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe(error) => write!(f, "single-instance check failed: {error}"),
            Self::Refused => write!(f, "the Owner process is not accepting connections"),
            Self::Start(error) => write!(f, "failed to start Owner process: {error}"),
            Self::Attach(error) => write!(f, "failed to attach to Owner process: {error}"),
        }
    }
}

impl std::error::Error for AttachError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Probe(error) => Some(error),
            Self::Start(error) | Self::Attach(error) => Some(error),
            Self::Refused => None,
        }
    }
}

/// Start the core thread (design D3): it loads the configuration, attaches
/// to the Owner, and runs the tick loop for the life of the process. The
/// thread is detached; a routed quit key ends the run, and row 2.5's
/// command senders end it when the front end drops.
pub fn spawn() {
    let (tx, rx) = mpsc::channel();
    // Row 2.5 replaces this sink with `AppHandle::emit`; the placeholder
    // keeps the thread startable before the commands exist.
    let sink: FrameSink = Box::new(|_| {});
    // Dropping the handle detaches the thread; nothing joins it.
    std::thread::spawn(move || {
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
        let app = match attach(&config) {
            Ok(app) => app,
            Err(error) => {
                tracing::error!(
                    name: "gui_core.attach_failed",
                    target: "gui_core",
                    error = %error,
                    "Owner attach failed; the core thread ends"
                );
                return;
            }
        };
        let mut core = CoreThread::new(app, rx, sink);
        core.install_waker(tx);
        core.run();
    });
}

/// The happy-path Owner attach (design D9): declare the GUI Client, find the
/// Owner, start one from the `mbv` program when none runs, and attach. Row
/// 2.6 owns the outcome classification — the window states, the wait for a
/// shutting-down Owner, and the R/Q handling — so an error here ends the
/// core thread until that row lands.
fn attach(config: &Config) -> Result<App, AttachError> {
    declare_gui_client();
    let program = owner_program();
    match find_owner().map_err(AttachError::Probe)? {
        OwnerState::Start => {
            let options = OwnerStartOptions {
                owner_exe: Some(&program),
                log_level: None,
            };
            start_owner(&options).map_err(AttachError::Start)?;
        }
        // A live Owner accepts connections; attach alongside it.
        OwnerState::Attach => {}
        OwnerState::Refuse => return Err(AttachError::Refused),
    }
    attach_owner(config, Some(&program)).map_err(AttachError::Attach)
}

/// The `mbv` program that becomes the Owner (design D9): the sibling of the
/// running `mbv-gui` executable when one is installed beside it, else the
/// `mbv` name on `PATH`. The Owner is never the GUI program.
fn owner_program() -> PathBuf {
    let name = format!("mbv{}", std::env::consts::EXE_SUFFIX);
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(&name);
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from(name)
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
    /// The inputs one step's wait collected, for the tick's input phase.
    pending: Vec<CoreInput>,
    /// A `report_layout` arrived this tick: the frame emits changed or not
    /// (design D3).
    frame_requested: bool,
    /// The frame that finds it armed carries the theme (design D7): the
    /// first frame, and every frame a `report_layout` triggers — the front
    /// end sends the report when it mounts, so a reload re-arms it.
    theme_pending: bool,
    /// The wait between ticks. Production keeps `CLIENT_IDLE_CADENCE`; the
    /// step test zeroes it.
    idle_cadence: Duration,
}

impl CoreThread {
    /// A core over an attached `App` (design D3): it owns the first slice's
    /// Surface state and the channel every input and wake feeds.
    #[must_use]
    pub fn new(app: App, rx: Receiver<CoreInput>, sink: FrameSink) -> Self {
        let mut focus = FocusStack::new();
        // The first slice mounts the Queue panel, the Queue playback band,
        // and the Status bar (design D4), and the Queue side holds focus.
        focus.mount(ComponentId::Queue);
        focus.mount(ComponentId::QueuePlaybackPanel);
        focus.mount(ComponentId::StatusBarPanel);
        focus.set_panel_focus(PanelFocus::Queue);
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
            pending: Vec::new(),
            frame_requested: false,
            theme_pending: true,
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
        let mut regions = HitMap::new();
        for region in &report.regions {
            regions.push(
                HitRegion::new(
                    region.rect.x,
                    region.rect.y,
                    region.rect.width,
                    region.rect.height,
                ),
                region.target,
            );
        }
        self.regions = regions;
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
            // A point in no reported region resolves to nothing.
            None => {}
        }
    }

    /// Run the loop (design D3): wait for input, wake, or the idle cadence,
    /// then tick. The loop ends when a routed key quit or the channel
    /// disconnects: row 2.5 adds the front end's command senders, and their
    /// drop ends the run.
    pub fn run(&mut self) {
        while self.step() {}
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
            // The front end and its commands are gone; the run is over.
            Err(RecvTimeoutError::Disconnected) => return false,
        }
        let outcome = ClientCore::tick(self, Instant::now());
        if outcome.quit_requested {
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
    /// `report_layout`-triggered frame (design D7).
    fn emit_frame(&mut self) {
        self.sync_queue_surface();
        let theme_changed = std::mem::take(&mut self.theme_pending);
        let frame = GuiFrame::build(&FrameSources {
            app: &self.app,
            focus: &self.focus,
            queue: &self.queue,
            viewport_rows: self.viewport_rows,
            breakpoint: self.breakpoint,
            theme_changed,
        });
        (self.sink)(frame);
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
        // Only a sync pass that paints the visual slot issues card fetches;
        // the first slice paints none, so the completion channel stays empty.
        false
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
                }
            }
        }
    }
}
