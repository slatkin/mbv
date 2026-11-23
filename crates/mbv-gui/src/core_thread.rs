//! The GUI Client's core thread (design D3 of `add-tauri-gui-client`): one
//! synchronous thread creates the `App`, attaches to the Owner, and runs
//! `ClientCore::tick` in a loop behind the window.
//!
//! The window's handlers never touch the `App`. They push a [`CoreInput`]
//! value into the thread's channel, and the thread waits on that channel with
//! a timeout of `CLIENT_IDLE_CADENCE`, so an input, a `ClientWaker` wake, or
//! the idle cadence each produce one tick. Each tick sends the batch of the
//! sections it changed through the section sink, and sends nothing when no
//! section changed (design D2). `main` wires that sink to the window channel,
//! and the window pushes into the channel; the tests mock both, so no window
//! or tokio runtime runs here (design D12).

use std::fmt;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mbv::{CLIENT_IDLE_CADENCE, ClientCore, GuiCore, GuiInput, load_config};
use mbv_config::ensure_runtime_dir;
use mbv_surfaces::gesture::PointerGestureState;
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_surfaces::{FocusStack, HitMap};
use mbv_ui_model::queue_rows::queue_media_rows;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, KeyEvent, PointerEvent, Position};

use crate::art::ArtRequest;
use crate::attach::{AttachCampaign, LibraryOwnerSteps};
use crate::bridge::keys::KeyTarget;
use crate::bridge::layout::{LayoutReport, LayoutSize, ListId, RegionTarget, hit_map};
use crate::bridge::sections::GuiSectionBatch;
use crate::leaves::QueueLeaf;
use crate::painted_choices::PaintedChoices;
use crate::sections::{SectionLedger, SectionSources, SentRevisions};

mod launch_restore;
mod overlays;
mod pointer;
mod settings;
mod tick_host;

use self::launch_restore::GuiLaunchRestore;

/// The viewport height before the first `report_layout` measures the window:
/// the queue section carries every row the queue holds. The window reports the
/// real capacity when it mounts (design D5), so the placeholder never
/// survives the first report.
const INITIAL_VIEWPORT_ROWS: usize = usize::MAX;

/// One message the core thread's channel carries (design D3): the front
/// end's translated input, a layout report, the wake value the installed
/// `ClientWaker` sends, or the window's close.
#[derive(Debug)]
pub enum CoreInput {
    /// A key chord, translated on the Rust side (design D6), with the panel
    /// the window's pointer resolved for it (design D2 of
    /// `gui-pointer-target-interaction`). The core sets its Panel focus from
    /// the target, then routes the key through `GuiCore::route_gui_input` with the
    /// focused surface as its leaf.
    Key { key: KeyEvent, target: KeyTarget },
    /// A pointer event, translated on the Rust side (design D4) and resolved
    /// against the slice hit map.
    Pointer(PointerEvent),
    /// A layout report from the window. The report applies to the Surface
    /// state's capacity and hit map; it marks no section and forces no batch
    /// (design D4).
    Layout(LayoutReport),
    /// One Library panel request (add-gui-library-panel design D2): the
    /// window content's `library` command. The core applies it through
    /// [`GuiCore::apply_gui_library_command`], after the Panel-focus adapter sets
    /// Library focus (design D3 of `gui-pointer-target-interaction`).
    Library(mbv::GuiLibraryCommand),
    /// One Settings screen request (`add-gui-settings-screen` design D5): the
    /// window content's `settings` command. The core applies it through
    /// [`GuiCore::apply_gui_settings_command`].
    Settings(mbv::GuiSettingsCommand),
    /// One Playlists screen request: the window content's `playlists`
    /// command, applied through [`GuiCore::apply_gui_playlists_command`].
    Playlists(mbv::GuiPlaylistsCommand),
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
/// the window channel's `WindowEvent::Batch`; the tests mock it with a
/// channel.
pub type SectionSink = Box<dyn Fn(GuiSectionBatch) + Send + 'static>;

/// The edge that closes the window (design D9): the mismatch state's Quit and
/// a routed quit key's close, wired by `main` to the window's close. The tests
/// capture it with a channel.
pub type QuitSink = Box<dyn Fn() + Send + 'static>;

/// Start the core thread (design D3): it loads the configuration, attaches
/// to the Owner, and runs the tick loop for the life of the window. The
/// thread ends when the window closes: `main` sends a [`CoreInput::Close`]
/// through the returned sender. The window's `CoreLink` pushes through the
/// same sender and never touches the `App` (design D3).
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
        // The GUI Client's `ClientCapabilities` claim the local link's
        // `named-queues` (gui-named-queues design D7) before its first Owner
        // connection, and its attach claims no Pin-swap participation.
        let steps = LibraryOwnerSteps::new(&config);
        let outcome = {
            let mut campaign = AttachCampaign::new(&rx, &sink, &quit, &steps);
            let outcome = campaign.run();
            (outcome, campaign.sent_revisions())
        };
        match outcome {
            (Ok(app), sent) => {
                let mut core = CoreThread::new(GuiCore::new(app), rx, sink, quit);
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

/// The core thread's state (design D3): the [`GuiCore`] wrapping the attached
/// `App`, the first slice's Surface state, and the channel every input and
/// wake feeds. The GUI Client owns its `App` by composition (design D4 of
/// `decouple-client-core`), so every core read and write goes through the
/// wrapper.
pub struct CoreThread {
    core: GuiCore,
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
    /// the queue row hover against the rebuilt regions at this point, so a
    /// report the hover itself causes does not clear the hover.
    pointer_at: Option<Position>,
    /// The whole Queue rows the viewport fits. Row 2.4 sets it from the
    /// `LayoutReport`; until then the queue section carries every row.
    viewport_rows: usize,
    /// The panel container's painted size from the last `LayoutReport`
    /// (design D10). The window picks the panel state from the panel
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
}

impl CoreThread {
    /// A core over an attached `App` (design D3): it owns the first slice's
    /// Surface state and the channel every input and wake feeds.
    #[must_use]
    pub fn new(
        mut core: GuiCore,
        rx: Receiver<CoreInput>,
        sink: SectionSink,
        quit: QuitSink,
    ) -> Self {
        // The GUI Client's App rules come first (add-gui-library-panel design
        // D4, D9): the GUI arranges its own panels from its measured size and
        // keeps its own launch location, so the TUI's Mini check, its hidden
        // empty queue column, and its launch state never apply here. The GUI's
        // own launch location replaces the TUI's (design D6a), on top of the
        // capabilities `attach_owner` recorded for this Client.
        core.app_mut().skip_launch_restore();
        let mut focus = FocusStack::new();
        // The first slice mounts the Queue panel, the Queue playback band,
        // and the Status bar (design D4). The Library panel mounts beside the
        // Queue (add-gui-library-panel task row 2.1). Panel focus is not
        // window state (`gui-pointer-target-interaction` design D3): the
        // core sets it per key, and the Library command path sets it before
        // each Library command.
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
        let cached_emby_client = core.app().has_emby_client();
        core.app_mut().start_client_services();
        let _home = core.app_mut().load_home_at_startup();
        if cached_emby_client {
            core.app_mut().start_playback_target_discovery();
        }
        // The GUI's own launch location (design D9), read once before the
        // first step: the TUI's launch state never moves this Client's tab,
        // and this file is the only location it restores. A start with no
        // saved location — or one that fails to load — restores no saved
        // collection sets. Every start opens on Home, whatever the file holds.
        let launch_restore = GuiLaunchRestore::Destination(
            mbv_config::load_gui_launch_state().unwrap_or_else(mbv_config::first_launch),
        );
        let mut thread = Self {
            core,
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
        };
        // The saved location applies over the core's startup state (design
        // D9): a launch the attach brought a catalog for restores here, and a
        // launch whose catalog lands through the startup worker restores on
        // that drain.
        thread.restore_launch_location();
        // A Client's test seeds and reads the state beside the `App` (row 8.1
        // of `decouple-client-core`), and the restore above wrote this core's
        // own, so the state it left goes back out at construction: the test
        // reads the restored sets with no step needing to run.
        thread.publish_seeded_gui_state();
        thread
    }

    /// Move Panel focus on both authorities together (add-gui-library-panel
    /// design D4): the App's stored Panel focus, which its key gates read,
    /// through its public GUI entry, then the core's `FocusStack`.
    fn take_panel_focus(&mut self, focus: PanelFocus) {
        self.core.set_gui_panel_focus(focus);
        self.focus.set_panel_focus(focus);
    }

    /// Set Panel focus on both authorities, skipped when the two already hold
    /// it (design D3 of `gui-pointer-target-interaction`): the GUI keeps no
    /// Panel focus, so the core sets it per key from the key's [`KeyTarget`],
    /// and before each Library command the window asked for.
    fn apply_panel_focus(&mut self, focus: PanelFocus) {
        if self.focus.panel_focus() == focus && self.core.gui_panel_focus() == focus {
            return;
        }
        self.take_panel_focus(focus);
    }

    /// Install the wake the App's relays invoke (design D3): every wake feeds
    /// a [`CoreInput::Wake`] into the core's own channel, so Owner events and
    /// timers wake the core as they wake the TUI.
    pub fn install_waker(&self, tx: Sender<CoreInput>) {
        ClientCore::from_app(self.core.app()).install_waker(move || {
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
    pub fn gui_panel_toggles_for_test(&self) -> mbv::PanelTogglesProjection {
        self.core.panel_toggles_projection()
    }

    pub fn gui_settings_section_for_test(&self) -> Option<mbv::GuiSettingsSection> {
        self.core.gui_settings().map(|state| state.section)
    }

    /// One Library flat list's active sort, for a Client's test that asserts
    /// the sort a command or a launch restore left the destination in.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_list_sort_for_test(&self, target: mbv::GuiSortTarget) -> mbv::GuiListSort {
        self.core.gui_list_sort(target)
    }

    /// The Library panel's destination tabs, for a Client's test that asserts
    /// which destinations a launch or a catalog change presented and which
    /// one is current.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_library_tabs_for_test(&self) -> mbv::GuiLibraryTabs {
        self.core.gui_library_tabs()
    }

    /// The active destination's kind, for a Client's test that asserts the
    /// destination a command or a drain opened.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_active_destination_for_test(&self) -> Option<mbv::GuiDestinationKind> {
        self.core.gui_active_destination()
    }

    /// Whether the Playlists screen is open, for a Client's test that asserts
    /// a key or a drained request opened or closed it.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_playlists_open_for_test(&self) -> bool {
        self.core.gui_playlists_open_for_test()
    }

    /// The Feeds section's content the GUI's core projects: the secrets-free
    /// subscription list and the add's progress, for a test that asserts what
    /// a command or a drained result left behind.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_settings_feeds_for_test(&self) -> mbv::GuiSettingsFeedsProjection {
        self.core.gui_settings_feeds()
    }

    /// Whether a playback-target refresh is waiting for a tick that has an
    /// Emby client, for a test whose stub holds none.
    #[doc(hidden)]
    #[must_use]
    pub fn playback_target_refresh_requested_for_test(&self) -> bool {
        self.core.app().playback_target_refresh_pending()
    }

    /// Apply one `LayoutReport` to the Surface state's capacity and the hit
    /// map (design D4, task row 2.4): the reported whole rows become the
    /// Queue viewport's height — the paging stride's source and the resting
    /// scroll's clamp — and the reported regions rebuild the combined slice
    /// hit map, the last painted regions a press resolves against. The
    /// reported container size is stored as painted (design D10): the
    /// panel-state decision it feeds stays in the window's container
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
        // instead of clearing: the hover fill's repaint re-reports the rows,
        // so the hover itself causes a report, and a clear here would flicker
        // it on every move (add-tauri-gui-client row 6.5).
        let row_hover = self.pointer_at.and_then(|at| self.queue_row_at(at));
        self.queue.set_hover(row_hover);
    }

    /// Route one translated key chord through the shared router with the
    /// focused surface as the leaf (design D4, task row 2.4). The first
    /// slice's only focusable surface is the Queue; with no focused surface
    /// no leaf can consume anything. Returns whether the `App` asked to
    /// quit, which the GUI Client answers by closing its window.
    ///
    /// The Panel-focus adapter runs first (design D3 of
    /// `gui-pointer-target-interaction`): the key's [`KeyTarget`] sets the
    /// stack's Panel focus — `Queue` for a Queue target, `Library` for none —
    /// so the Queue leaf is built only while the stack's focus is the Queue.
    /// `ComponentId::Library` stays mounted, so a key with no Queue target
    /// meets only the leafless global and transport chords.
    fn route_key(&mut self, key: KeyEvent, target: KeyTarget) -> bool {
        let focus = match target {
            KeyTarget::Queue => PanelFocus::Queue,
            KeyTarget::None => PanelFocus::Library,
        };
        self.apply_panel_focus(focus);
        if self.focus.focused().as_ref() != Some(&ComponentId::Queue) {
            return self.core.route_gui_input(GuiInput::Key(key), None);
        }
        let Self {
            core,
            queue,
            regions,
            ..
        } = self;
        let mut leaf = QueueLeaf::new(queue, regions);
        core.route_gui_input(GuiInput::Key(key), Some(&mut leaf))
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
        self.core.app_mut().client_exit_teardown();
    }

    /// Take the GUI Client's own state a Client's test seeded beside its
    /// `App`, at the start of one step (row 8.1 of `decouple-client-core`):
    /// the `App` seams write a state that is not this core's, so the step takes
    /// that one before it reads anything. Production builds no sidecar, so the
    /// call does nothing there.
    fn take_seeded_gui_state(&mut self) {
        self.core.take_seeded_gui_state_for_test();
    }

    /// Publish this core's own state back beside the `App` a Client's test
    /// holds, at the end of one step (row 8.1 of `decouple-client-core`), so
    /// the test's `*_for_test` reads answer with the state the step left.
    /// Production builds no sidecar, so the call does nothing there.
    fn publish_seeded_gui_state(&self) {
        self.core.publish_seeded_gui_state_for_test();
    }

    /// One step: wait for the next input, run one tick, and send the batch of
    /// the sections the tick changed (design D2). Returns whether the loop
    /// continues.
    pub fn step(&mut self) -> bool {
        // A Client's test seeds through the `App` seams, which write the state
        // beside the `App` (row 8.1 of `decouple-client-core`), so the step
        // takes it before it reads anything: the sections it builds and the
        // batch it sends already carry the seed.
        self.take_seeded_gui_state();
        match self.rx.recv_timeout(self.idle_cadence) {
            Ok(input) => {
                self.pending.push(input);
                // Take every input already queued beside it, so a wake that
                // arrives while a command is pending never delays that
                // command to a later step.
                while let Ok(input) = self.rx.try_recv() {
                    self.pending.push(input);
                }
            }
            // The idle cadence keeps the maintenance phases running with no
            // input queued.
            Err(RecvTimeoutError::Timeout) => {}
            // The channel itself ended: the same consequence as the window's
            // close, and unreachable while the App's waker holds its sender.
            // The step hands its state back before it returns either way, so
            // the sidecar slot never stands empty.
            Err(RecvTimeoutError::Disconnected) => {
                self.publish_seeded_gui_state();
                return false;
            }
        }
        let outcome = ClientCore::tick(self, Instant::now()); // The run-loop quit signal the TUI's `'outer` loop breaks on
        // (`shell/run.rs`): an announced Owner shutdown sets it from the
        // App's drain half, and the GUI Client takes the same orderly exit
        // — the quit edge closes the window, and the run's exit boundary in
        // `run` saves the launch location and tears the shared state down.
        if outcome.quit_requested || self.core.app().quit_requested() {
            tracing::info!(
                name: "gui_core.run_quit",
                target: "gui_core",
                input_quit = outcome.quit_requested,
                reason = ?self.core.app().exit_message(),
                "the GUI run ends through the quit edge"
            );
            // The routed quit closes the window through the same edge the
            // mismatch state's Quit uses (design D9). The step hands its state
            // back before it returns either way, so the sidecar slot never
            // stands empty.
            (self.quit)();
            self.publish_seeded_gui_state();
            return false;
        }
        // A restart re-reads the landed Owner snapshot next iteration; the
        // sections wait for it, as the TUI's run loop does.
        if !outcome.restart_requested {
            self.send_sections();
        }
        // The step done, its state goes back beside the `App` (row 8.1 of
        // `decouple-client-core`), so a Client's test reads what the step left.
        self.publish_seeded_gui_state();
        true
    }

    /// Adopt the Queue projection into the Queue Surface state (design D5),
    /// the consumption the TUI's sync pass runs once per pass, before the
    /// section that reads the Surface state builds.
    fn sync_queue_surface(&mut self) {
        let update = self.core.app_mut().queue_surface_projection();
        // The shown entry changed: drop the marks and the viewport before the
        // start-slot cursor lands (design D7 of `gui-named-queues`).
        if update.presentation_reset {
            self.queue.reset_presentation();
        }
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
    /// changed (design D2). The Queue Surface sync runs first, because the
    /// ledger's sections read what it adopts; the sink gets a batch only when
    /// the ledger has one.
    fn send_sections(&mut self) {
        self.sync_queue_surface();
        let batch = self.ledger.collect(SectionSources {
            core: &mut self.core,
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
            .field("core", &self.core)
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
