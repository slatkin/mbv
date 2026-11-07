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

use mbv::{App, CLIENT_IDLE_CADENCE, ClientCore, GuiInput, declare_gui_client, load_config};
use mbv_config::ensure_runtime_dir;
use mbv_queue::QueueSlotId;
use mbv_surfaces::gesture::{PointerGesture, PointerGestureState};
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_surfaces::{FocusStack, HitMap, HitRegion};
use mbv_ui_model::queue_rows::queue_media_rows;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, KeyEvent, PointerEvent, PointerKind, Position};

use crate::artwork::ArtworkFrames;
use crate::attach::{AttachCampaign, LibraryOwnerSteps};
use crate::bridge::frame::playback::GuiScrubberTile;
use crate::bridge::frame::{FrameSources, GuiFrame, GuiWindowFrame, scrubber_ticks};
use crate::bridge::layout::{
    LayoutReport, LayoutSize, ListId, QueuePlaybackTargetId, QueueTargetId, RegionTarget, hit_map,
};
use crate::leaves::{PlaybackLeaf, QueueLeaf, StatusBarLeaf, TransportAvailability, gesture_point};
use crate::painted_choices::PaintedChoices;

mod tick_host;

/// The viewport height before the first `report_layout` measures the window:
/// the frame carries every row the queue holds. The front end reports the
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
    /// The panel container's painted size from the last `LayoutReport`
    /// (design D10). The front end's container queries pick the panel state
    /// from it, so the core stores it without classifying it: the reported
    /// capacity and regions already measure the layout that size painted.
    container_size: LayoutSize,
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
    /// The frames' trickplay memo (task 6.7): the hovered sheet's encoded
    /// image, re-carried only when the hovered sheet changes.
    trickplay: crate::trickplay::TrickplayFrames,
    /// The open overlay's choice lists as the last emitted frame painted them
    /// (add-tauri-gui-client row 6.5): a chooser or popover row's press
    /// resolves its painted index against these, so a list change between the
    /// paint and the press selects the painted choice or nothing.
    painted: PaintedChoices,
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
            container_size: LayoutSize::default(),
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
            trickplay: crate::trickplay::TrickplayFrames::default(),
            painted: PaintedChoices::default(),
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
        // over a painted bar re-arms it. The queue row's hover follows the
        // same rule (add-tauri-gui-client row 6.5).
        self.playback.set_scrubber_hover(None);
        self.queue.set_hover(None);
    }

    /// The Queue row the pointer rests on: the row's own region or the chrome
    /// painted over it (its grip and actions button), so moving onto the
    /// chrome keeps the row hovered.
    fn queue_row_at(&self, at: Position) -> Option<QueueSlotId> {
        match self.regions.resolve(at) {
            Some(RegionTarget::Queue(
                QueueTargetId::Slot(raw)
                | QueueTargetId::RowGrip(raw)
                | QueueTargetId::RowActions(raw),
            )) => Some(QueueSlotId::from_raw(*raw)),
            _ => None,
        }
    }

    /// The last painted seekbar region holding `at`, the resolution
    /// precedence the hit map gives the last push; `None` when the pointer
    /// is away from the bar.
    fn seekbar_region_at(&self, at: Position) -> Option<HitRegion> {
        self.regions
            .iter()
            .filter(|(region, target)| {
                matches!(
                    target,
                    RegionTarget::QueuePlayback(QueuePlaybackTargetId::Seekbar)
                ) && region.contains(at)
            })
            .last()
            .map(|(region, _)| *region)
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
        // A cancelled gesture (the webview lost the pointer, the pointer left
        // the window with a button held, or the window lost focus) drops the
        // drag state without a move (add-tauri-gui-client row 6.5, finding
        // P1-2): the release that would have ended the drag never arrives.
        if event.kind == PointerKind::Cancel {
            self.gestures.cancel();
            self.queue.cancel_row_drag();
            self.queue.set_hover(None);
            self.playback.set_scrubber_hover(None);
            return;
        }
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
        let hover = self
            .seekbar_region_at(event.position)
            .and_then(|bar| self.playback.seek_target(bar, event.position.x));
        self.playback.set_scrubber_hover(hover);
        // The canvas queue's row hover (add-tauri-gui-client row 6.5): the row
        // the pointer rests on, its chrome regions included. A frame paints
        // the hovered row's grip and actions button.
        let row_hover = self.queue_row_at(event.position);
        self.queue.set_hover(row_hover);
        // An open overlay owns the keyboard and the window's presses: a press
        // outside its own targets dismisses it and does not reach what is
        // painted beneath (the canvas's outside press; the TUI's blocking
        // context menu dismisses the same way).
        if matches!(event.kind, PointerKind::Press | PointerKind::RightPress)
            && self.app.gui_overlay_is_open()
            && !self
                .regions
                .resolve(event.position)
                .is_some_and(RegionTarget::owns_gui_overlay)
        {
            self.app.dismiss_open_gui_overlay();
            return;
        }
        let now = Instant::now();
        let Some(gesture) =
            self.gestures
                .recognize(event.kind, event.modifiers, event.position, now)
        else {
            return;
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
            return;
        }
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
                // emitted frame's choice lists, not this read.
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
            Some(RegionTarget::Attach(_)) | None => {}
        }
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
    /// `report_layout`-triggered frame (design D7) — the visual slot's
    /// artwork when the slot's art changed for this window (design D10), and
    /// the scrubber hover's trickplay tile (task 6.7).
    fn emit_frame(&mut self) {
        self.sync_queue_surface();
        // The active item's metadata fetch (design D4a) issues from the
        // frame path, deduped on its own loading set; a landed completion
        // rides the next frame.
        self.app.fetch_playback_metadata();
        // One playback projection read feeds the frame's playback section,
        // its overlay, and the hover's tick resolution, so they cannot
        // disagree inside one frame.
        let playback = self.app.queue_playback_projection();
        let hover_tile = self.hover_tile(&playback);
        let artwork = self.app.queue_card_artwork();
        let artwork = self.artwork.frame_value(&artwork);
        let theme_changed = std::mem::take(&mut self.theme_pending);
        let frame = GuiFrame::build(&FrameSources {
            app: &self.app,
            focus: &self.focus,
            queue: &self.queue,
            playback: &self.playback,
            playback_projection: &playback,
            hover_tile,
            viewport_rows: self.viewport_rows,
            theme_changed,
            artwork,
        });
        // The frame the window is about to paint becomes the choice lists a
        // pointer press resolves against (add-tauri-gui-client row 6.5).
        self.painted.adopt(frame.overlay.as_ref());
        (self.sink)(GuiWindowFrame::Slice(Box::new(frame)));
    }

    /// The trickplay tile the hover preview paints (task 6.7): the hovered
    /// position resolved against the active item's grid, with the sheet's
    /// fetch issued from here as the metadata fetch is. `None` while the
    /// pointer is away from the bar, while no sheet is cached yet, and for an
    /// item with no trickplay source.
    fn hover_tile(&mut self, playback: &mbv::QueuePlaybackProjection) -> Option<GuiScrubberTile> {
        let fraction = self.playback.scrubber_hover()?;
        let ticks = scrubber_ticks(playback, fraction);
        let preview = self.app.trickplay_preview_tile(ticks);
        self.trickplay.frame_value(preview.as_ref())
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
            .field("frame_requested", &self.frame_requested)
            .field("theme_pending", &self.theme_pending)
            .field("painted", &self.painted)
            .finish_non_exhaustive()
    }
}
