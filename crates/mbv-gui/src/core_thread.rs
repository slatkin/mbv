//! The GUI Client's core thread (design D3 of `add-tauri-gui-client`): one
//! synchronous thread creates the `App`, attaches to the Owner, and runs
//! `ClientCore::tick` in a loop behind the window.
//!
//! Tauri command handlers never touch the `App`. They push a [`CoreInput`]
//! value into the thread's channel, and the thread waits on that channel with
//! a timeout of `CLIENT_IDLE_CADENCE`, so an input, a `ClientWaker` wake, or
//! the idle cadence each produce one tick. Each tick sends the batch of the
//! sections it changed through the section sink, and sends nothing when no
//! section changed (design D2). `main` wires that sink to `AppHandle::emit`,
//! and the commands push into the channel; the tests mock both, so no Tauri
//! runtime or tokio runs here (design D12).

use std::fmt;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mbv::{App, CLIENT_IDLE_CADENCE, ClientCore, GuiInput, declare_gui_client, load_config};
use mbv_config::ensure_runtime_dir;
use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::{PointerGesture, PointerGestureState};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_surfaces::{FocusStack, HitMap};
use mbv_ui_model::queue_rows::queue_media_rows;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, KeyEvent, PointerEvent, PointerKind, Position};

use crate::art::ArtRequest;
use crate::attach::{AttachCampaign, LibraryOwnerSteps};
use crate::bridge::layout::{
    ConfirmTargetId, DaemonLostTargetId, LayoutReport, LayoutSize, ListId, QueueTargetId,
    RegionTarget, hit_map,
};
use crate::bridge::sections::GuiSectionBatch;
use crate::leaves::{
    PlaybackLeaf, QueueLeaf, StatusBarLeaf, TransportAvailability, gesture_point, row_slot_at,
    seekbar_region,
};
use crate::painted_choices::PaintedChoices;
use crate::sections::{SectionLedger, SectionSources, SentRevisions};

mod launch_restore;
mod overlays;
mod settings;
mod tick_host;

use self::launch_restore::GuiLaunchRestore;

/// The viewport height before the first `report_layout` measures the window:
/// the queue section carries every row the queue holds. The front end reports the
/// real capacity when it mounts (design D5), so the placeholder never
/// survives the first report.
const INITIAL_VIEWPORT_ROWS: usize = usize::MAX;

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
    /// A layout report from the front end. The report applies to the Surface
    /// state's capacity and hit map; it marks no section and forces no batch
    /// (design D4).
    Layout(LayoutReport),
    /// One Library panel request (add-gui-library-panel design D2): the
    /// window content's `library` command. The core applies it through
    /// [`App::apply_gui_library_command`], and the take-focus variant also
    /// moves the core's `FocusStack` with the App's stored focus (design D4),
    /// so the window paints the focus the App's key gates use.
    Library(mbv::GuiLibraryCommand),
    /// One Settings screen request (`add-gui-settings-screen` design D5): the
    /// window content's `settings` command. The core applies it through
    /// [`App::apply_gui_settings_command`].
    Settings(mbv::GuiSettingsCommand),
    /// The window asks for a full snapshot (design D4): the core releases
    /// every section the window holds, so the next batch carries every present
    /// section at its current revision. It marks no section.
    Sync,
    /// One image load the `mbv-art` scheme raised (design D7 of
    /// `gui-change-driven-sections`): the tick host answers it through the
    /// `App`'s read-only cache queries, with no mark and no `input_received`.
    Art(ArtRequest),
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

/// The sink every sent batch goes through (design D1). Production wires
/// `AppHandle::emit` as the `mbv://sections` event; the tests mock it with a
/// channel.
pub type SectionSink = Box<dyn Fn(GuiSectionBatch) + Send + 'static>;

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
pub fn spawn(sink: SectionSink, quit: QuitSink) -> (Sender<CoreInput>, JoinHandle<()>) {
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
            let outcome = campaign.run();
            (outcome, campaign.sent_revisions())
        };
        match outcome {
            (Ok(app), sent) => {
                let mut core = CoreThread::new(app, rx, sink, quit);
                core.resume_after_attach(sent);
                core.install_waker(waker_tx);
                core.run();
            }
            (Err(end), _) => tracing::info!(
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
    /// Where the pointer last rested, from the last non-cancel pointer event;
    /// `None` after a cancel or before any event. A layout report re-resolves
    /// the queue row hover against the rebuilt regions at this point, so the
    /// report the hover chrome's own regions cause does not clear the hover.
    pointer_at: Option<Position>,
    /// The whole Queue rows the viewport fits. Row 2.4 sets it from the
    /// `LayoutReport`; until then the queue section carries every row.
    viewport_rows: usize,
    /// The panel container's painted size from the last `LayoutReport`
    /// (design D10). The front end picks the panel state from the panel
    /// container's measured size, the same measurement this report carries,
    /// so the core stores it without classifying it: the reported capacity
    /// and regions already measure the layout that size painted.
    container_size: LayoutSize,
    rx: Receiver<CoreInput>,
    sink: SectionSink,
    /// The edge that closes the window (design D9): a routed quit key closes
    /// the window through it, as the mismatch state's Quit does.
    quit: QuitSink,
    /// The inputs one step's wait collected, for the tick's input phase.
    pending: Vec<CoreInput>,
    /// The section ledger (design D2): the last built value of each section,
    /// the revision it carries, and what the window holds.
    ledger: SectionLedger,
    /// The open overlay's choice lists as the last sent batch painted them
    /// (add-tauri-gui-client row 6.5): a chooser or popover row's press
    /// resolves its painted index against these, so a list change between the
    /// paint and the press selects the painted choice or nothing.
    painted: PaintedChoices,
    /// The wait between ticks. Production keeps `CLIENT_IDLE_CADENCE`; the
    /// step test zeroes it.
    idle_cadence: Duration,
    /// The saved GUI launch location and its restore phase (design D9).
    launch_restore: GuiLaunchRestore,
    /// A take-focus ran while the restore was pending (design D9): the user
    /// moved Panel focus in the pre-catalog window, so the first phase skips
    /// the saved Panel-focus re-apply.
    launch_focus_taken: bool,
}

impl CoreThread {
    /// A core over an attached `App` (design D3): it owns the first slice's
    /// Surface state and the channel every input and wake feeds.
    #[must_use]
    pub fn new(mut app: App, rx: Receiver<CoreInput>, sink: SectionSink, quit: QuitSink) -> Self {
        // The GUI Client's App rules come first (add-gui-library-panel design
        // D4, D9): the GUI arranges its own panels from its measured size and
        // keeps its own launch location, so the TUI's Mini check, its hidden
        // empty queue column, and its launch state never apply here.
        app.enable_gui_client();
        let mut focus = FocusStack::new();
        // The first slice mounts the Queue panel, the Queue playback band,
        // and the Status bar (design D4), and the Queue side holds focus. The
        // Library panel mounts beside the Queue (add-gui-library-panel task
        // row 2.1) and takes Panel focus on the take-focus command.
        focus.mount(ComponentId::Queue);
        focus.mount(ComponentId::QueuePlaybackPanel);
        focus.mount(ComponentId::StatusBarPanel);
        focus.mount(ComponentId::Library);
        // The shared Service startup step every Client runs once after the
        // attach (add-gui-library-panel design D11): the Service startup
        // workers, the startup feed fetch, capability registration, and the
        // idle feed. Then the launch Home load, whose content the App
        // stores for the Home projection that lands in row 6.2. A client the
        // attach brought (`owner_link`'s
        // `cached_emby_client`) starts neither startup worker, so the GUI
        // starts the playback-target discovery it feeds here; a startup the
        // drain reports starts it in the drain host instead.
        let cached_emby_client = app.has_emby_client();
        app.start_client_services();
        let _home = app.load_home_at_startup();
        if cached_emby_client {
            app.start_playback_target_discovery();
        }
        // The GUI's own launch location (design D9), read once before the
        // first step: the TUI's launch state never moves this Client's tab or
        // Panel focus, and this file is the only location it restores. A start
        // with no saved location — or one that fails to load — restores the
        // Client's first-choice rules through the same restore path: the first
        // presented destination, Library Panel focus, and no saved collection
        // sets, so the Library panel is reachable from the first launch.
        let launch_restore = GuiLaunchRestore::Destination(
            mbv_config::load_gui_launch_state().unwrap_or_else(mbv_config::first_launch),
        );
        let mut core = Self {
            app,
            focus,
            queue: QueueSurface::new(),
            playback: QueuePlaybackSurface::new(),
            status_bar: StatusBarSurface::new(),
            gestures: PointerGestureState::new(),
            regions: HitMap::new(),
            pointer_at: None,
            viewport_rows: INITIAL_VIEWPORT_ROWS,
            container_size: LayoutSize::default(),
            rx,
            sink,
            quit,
            pending: Vec::new(),
            ledger: SectionLedger::default(),
            painted: PaintedChoices::default(),
            idle_cadence: CLIENT_IDLE_CADENCE,
            launch_restore,
            launch_focus_taken: false,
        };
        // The core sets its `FocusStack` and the App's stored Panel focus
        // together (design D4), so the sections the window paints and the gates
        // the keys meet agree from the first batch. The Queue side holds
        // focus until the take-focus command moves it.
        core.take_panel_focus(PanelFocus::Queue);
        // The saved location applies over that default (design D9): a launch
        // the attach brought a catalog for restores here, and a launch whose
        // catalog lands through the startup worker restores on that drain.
        core.restore_launch_location();
        core
    }

    /// Move Panel focus on both authorities together (add-gui-library-panel
    /// design D4): the App's stored Panel focus, which its key gates read,
    /// through its public GUI entry, then the core's `FocusStack`, which
    /// derives the window's focus.
    fn take_panel_focus(&mut self, focus: PanelFocus) {
        self.app.set_gui_panel_focus(focus);
        self.focus.set_panel_focus(focus);
    }

    /// Install the wake the App's relays invoke (design D3): every wake feeds
    /// a [`CoreInput::Wake`] into the core's own channel, so Owner events and
    /// timers wake the core as they wake the TUI.
    pub fn install_waker(&self, tx: Sender<CoreInput>) {
        ClientCore::from_app(&self.app).install_waker(move || {
            let _ = tx.send(CoreInput::Wake);
        });
    }

    /// Continue the revisions the attach campaign sent (design D4). The
    /// attached core's first batch carries `attach: null` at a revision past
    /// the campaign's, so the window applies it.
    pub fn resume_after_attach(&mut self, sent: SentRevisions) {
        self.ledger.resume(sent);
    }

    /// Adopt the wait cadence a test pins; production keeps
    /// `CLIENT_IDLE_CADENCE`. A zeroed cadence makes a step's wait
    /// non-blocking.
    pub fn set_idle_cadence_for_test(&mut self, cadence: Duration) {
        self.idle_cadence = cadence;
    }

    /// Turn the section audit on or off (design D3): with it on, every tick
    /// builds each clean gated section and panics when the build differs from
    /// the last built value, which names a missed mark. The core-thread tests
    /// run with it on; production never turns it on.
    pub fn set_section_audit_for_test(&mut self, on: bool) {
        self.ledger.set_audit_for_test(on);
    }

    /// Move the playhead anchor's send instant back by `by` (design D5's
    /// drift rule), so a core-thread test's next collect predicts from an
    /// older anchor with no real wait. Production never calls it.
    pub fn backdate_playhead_anchor_for_test(&mut self, by: Duration) {
        self.ledger.backdate_playhead_anchor_for_test(by);
    }

    /// Mark the sections a catalog landing marks (design D3 rule 2), for a test
    /// whose seed lands the catalog between ticks. The startup drain marks every
    /// gated section when it lands a catalog (`DrainHook::DrainStartupWorkers`), and
    /// this call marks the same rows. Production never calls it.
    pub fn mark_startup_landing_for_test(&mut self) {
        self.mark_hook(crate::sections::DrainHook::DrainStartupWorkers);
    }

    /// Mark the sections a library completion marks (design D3 rule 2), for a
    /// test whose seed stands in for completions the library drain applies. The
    /// drain marks these rows when it reports a completion
    /// (`DrainHook::DrainLibEvents`), and this call marks the same rows. Production
    /// never calls it.
    pub fn mark_library_landing_for_test(&mut self) {
        self.mark_hook(crate::sections::DrainHook::DrainLibEvents);
    }

    /// The Settings screen's open section, for a Client's test that asserts a
    /// key opened, closed, or switched it (`add-gui-settings-screen` rows
    /// 3.2, 3.3). The settings section carries it to the window; this is the
    /// read a Client's test has.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_settings_section_for_test(&self) -> Option<mbv::GuiSettingsSection> {
        self.app.gui_settings().map(|state| state.section)
    }

    /// Apply one `LayoutReport` to the Surface state's capacity and the hit
    /// map (design D4, task row 2.4): the reported whole rows become the
    /// Queue viewport's height — the paging stride's source and the resting
    /// scroll's clamp — and the reported regions rebuild the combined slice
    /// hit map, the last painted regions a press resolves against. The
    /// reported container size is stored as painted (design D10): the
    /// panel-state decision it feeds stays in the front end's container
    /// queries, and the capacity and regions the same report carries
    /// already measure the layout that size painted.
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
        self.container_size = report.container_size;
        // The regions the hover resolves against just rebuilt: a hover over
        // a moved or unpainted bar is stale, so it clears and the next move
        // over a painted bar re-arms it.
        self.playback.set_scrubber_hover(None);
        // The queue row's hover re-resolves at the pointer's last place
        // instead of clearing: the hovered row paints its grip and actions
        // button as regions of their own, so the hover itself causes a report,
        // and a clear here would drop the chrome the report just measured and
        // flicker it on every move (add-tauri-gui-client row 6.5).
        let row_hover = self.pointer_at.and_then(|at| self.queue_row_at(at));
        self.queue.set_hover(row_hover);
    }

    /// The Queue row the pointer rests on, the shared region rule the leaf's
    /// click path resolves with too: the row's own region or the chrome
    /// painted over it (its grip and actions button), so moving onto the
    /// chrome keeps the row hovered.
    fn queue_row_at(&self, at: Position) -> Option<QueueSlotId> {
        row_slot_at(&self.regions, at)
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
    fn route_pointer(&mut self, event: PointerEvent) -> bool {
        // Any pointer event disarms prefix mode first — the shared contract
        // `App::route_gui_input`'s pointer arm carries, the same silent
        // disarm every terminal mouse event applies in the TUI. The call
        // runs before the gesture and region resolution, so a hover move, a
        // drag end, or a press on no reported region still cancels an armed
        // prefix; the pointer arm itself delivers nothing else.
        self.app.route_gui_input(GuiInput::Pointer(event), None);
        // A cancelled gesture (the webview lost the pointer, the pointer left
        // the window with a button held, or the window lost focus) drops the
        // drag state without a move (add-tauri-gui-client row 6.5, finding
        // P1-2): the release that would have ended the drag never arrives.
        if event.kind == PointerKind::Cancel {
            self.pointer_at = None;
            self.gestures.cancel();
            self.queue.cancel_row_drag();
            self.queue.set_hover(None);
            self.playback.set_scrubber_hover(None);
            return false;
        }
        self.pointer_at = Some(event.position);
        // A new press while a row drag is armed discards the stale drag
        // first: a release the window never saw cannot make the next click's
        // DragEnd move the old slot.
        if matches!(event.kind, PointerKind::Press | PointerKind::RightPress) {
            self.queue.cancel_row_drag();
        }
        // The scrubber's hover preview (design D4: "a move over the
        // scrubber carries the hover"): the event's position resolves
        // against the painted bar through the same fraction rule a seek
        // uses, and a position away from the bar clears it.
        let hover = seekbar_region(&self.regions, event.position)
            .and_then(|bar| self.playback.seek_target(bar, event.position.x));
        self.playback.set_scrubber_hover(hover);
        // The canvas queue's row hover (add-tauri-gui-client row 6.5): the row
        // the pointer rests on, its chrome regions included. The window paints
        // the hovered row's grip and actions button.
        let row_hover = self.queue_row_at(event.position);
        self.queue.set_hover(row_hover);
        // An open overlay owns the keyboard and the window's presses: a press
        // outside its own targets dismisses it and does not reach what is
        // painted beneath (the canvas's outside press; the TUI's blocking
        // context menu dismisses the same way).
        if self.press_outside_open_overlay(&event) {
            self.app.dismiss_open_gui_overlay();
            return false;
        }
        let now = Instant::now();
        let Some(gesture) =
            self.gestures
                .recognize(event.kind, event.modifiers, event.position, now)
        else {
            return false;
        };
        // The row drag's motion and release address the Queue Surface state
        // directly (add-tauri-gui-client row 6.5, design D5): motion only
        // previews the drop, and the release emits the one move request. A
        // drag the row drag does not own keeps the generic path (the
        // seekbar's scrub is a drag too).
        if matches!(
            gesture,
            PointerGesture::Drag { .. } | PointerGesture::DragEnd
        ) && self.route_row_drag_gesture(gesture)
        {
            return false;
        }
        let Some(point) = gesture_point(&gesture) else {
            return false;
        };
        match self.regions.resolve(point) {
            Some(RegionTarget::Queue(target)) => {
                // The TUI click's focus change (design D4): a click, double
                // click, or right click on a queue row runs the TUI single
                // click's effects, one of which focuses the Queue panel. The
                // dispatched `QueueRowClick` and `QueueRowActivate` apply the
                // App-side half; the core mirrors the panel focus on its own
                // stack — whose derived focus the window paints and the next
                // keys' routing reads — because the right click's
                // `RowContextMenu` dispatch is the documented no-op. The pill
                // and the wheel take no focus, exactly as the TUI's matching
                // gestures do not.
                if matches!(
                    gesture,
                    PointerGesture::Click { .. }
                        | PointerGesture::DoubleClick(_)
                        | PointerGesture::RightClick(_)
                ) && matches!(
                    target,
                    QueueTargetId::Slot(_)
                        | QueueTargetId::RowGrip(_)
                        | QueueTargetId::RowActions(_)
                ) {
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
                // panel holds focus (design D4). The projection's
                // availability gates the transport; the chooser and popover
                // rows resolve their painted indexes against the last
                // sent choice lists, not this read.
                let projection = self.app.queue_playback_projection();
                let availability = projection.availability;
                let Self {
                    app,
                    playback,
                    regions,
                    painted,
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
                    painted,
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
            // A confirm button's press answers the modal through the same
            // execution the button's key hint applies; the outside-press
            // dismissal above already left confirm regions un-dismissed.
            Some(RegionTarget::Confirm(ConfirmTargetId::Button(index))) => {
                self.confirm_button_press(gesture, *index);
            }
            // The daemon-lost dialog's buttons answer through the same
            // execution their key hints apply; the press that lands outside
            // them left the dialog up (it is not dismissible).
            Some(RegionTarget::DaemonLost(DaemonLostTargetId::Button(index))) => {
                return self.daemon_lost_button_press(gesture, *index);
            }
            Some(RegionTarget::Attach(_)) | None => {}
        }
        false
    }

    /// Deliver a gesture the row drag owns to the Queue leaf: motion previews
    /// the drop and the release sends the move request, both through the leaf
    /// entry so a message the drag arm returns is dispatched, never dropped.
    /// Returns whether the drag owned the gesture; `false` leaves it to the
    /// generic region path (a drag that started off a grip is no row drag).
    fn route_row_drag_gesture(&mut self, gesture: PointerGesture<Position>) -> bool {
        if self.queue.row_drag().is_none() {
            return false;
        }
        let Self {
            app,
            queue,
            regions,
            ..
        } = self;
        let mut leaf = QueueLeaf::new(queue, regions);
        app.route_gui_pointer(gesture, Some(&mut leaf));
        true
    }

    /// Run the loop (design D3): wait for input, wake, or the idle cadence,
    /// then tick. The loop ends when the window closes — its close arrives
    /// as a [`CoreInput::Close`], a routed quit key answers the same way,
    /// and an announced exit path the App's drains recorded — the Owner
    /// shutdown's quit signal (`daemon-disconnect-handling`: "Clients exit
    /// cleanly on an announced shutdown", the TUI run loop's `'outer` break
    /// on `QUIT_REQUESTED`) ends it through the quit edge — or when the
    /// channel itself ends.
    ///
    /// The run's exit boundary saves the GUI's launch location first (design
    /// D9: the orderly exit's save, and the only write of that file), then
    /// runs the shared Client exit teardown once, after the loop, whichever
    /// edge ended it — a routed quit, the window's close, the Owner's
    /// announced shutdown, or a channel end: the deferred settings save
    /// flushes, the auto-reconnect record persists, and the daemon-lifetime
    /// policy applies (daemon-lifecycle "Quitting a client follows Stay
    /// Alive and offers no detach"). The join in `main`
    /// holds the process until it completes, so the teardown survives the
    /// window's close.
    pub fn run(&mut self) {
        while self.step() {}
        self.save_launch_location();
        self.app.client_exit_teardown();
    }

    /// One step: wait for the next input, run one tick, and send the batch of
    /// the sections the tick changed (design D2). Returns whether the loop
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
        // The run-loop quit signal the TUI's `'outer` loop breaks on
        // (`shell/run.rs`): an announced Owner shutdown sets it from the
        // App's drain half, and the GUI Client takes the same orderly exit
        // — the quit edge closes the window, and the run's exit boundary in
        // `run` saves the launch location and tears the shared state down.
        if outcome.quit_requested || self.app.quit_requested() {
            tracing::info!(
                name: "gui_core.run_quit",
                target: "gui_core",
                input_quit = outcome.quit_requested,
                reason = ?self.app.exit_message(),
                "the GUI run ends through the quit edge"
            );
            // The routed quit closes the window through the same edge the
            // mismatch state's Quit uses (design D9).
            (self.quit)();
            return false;
        }
        // A restart re-reads the landed Owner snapshot next iteration; the
        // sections wait for it, as the TUI's run loop does.
        if !outcome.restart_requested {
            self.send_sections();
        }
        true
    }

    /// Adopt the Queue projection into the Queue Surface state (design D5),
    /// the consumption the TUI's sync pass runs once per pass, before the
    /// section that reads the Surface state builds.
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

    /// Run the per-tick prologue and send the batch of the sections this tick
    /// changed (design D2). The Queue Surface sync and the active item's
    /// metadata fetch run first, because the ledger's sections read what they
    /// adopt or start; the sink gets a batch only when the ledger has one.
    fn send_sections(&mut self) {
        self.sync_queue_surface();
        self.app.fetch_playback_metadata();
        let batch = self.ledger.collect(SectionSources {
            app: &mut self.app,
            focus: &self.focus,
            queue: &self.queue,
            playback_surface: &self.playback,
            viewport_rows: self.viewport_rows,
            now: Instant::now(),
        });
        let Some(batch) = batch else {
            return;
        };
        // The overlay the window now holds is the choice list a pointer press
        // resolves against (add-tauri-gui-client row 6.5).
        if let Some(overlay) = batch.overlay.as_ref() {
            self.painted.adopt(overlay.value.as_ref());
        }
        (self.sink)(batch);
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
            .field("container_size", &self.container_size)
            .field("pending", &self.pending.len())
            .field("painted", &self.painted)
            .finish_non_exhaustive()
    }
}
