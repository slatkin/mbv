//! The Owner attach campaign (design D9 of `add-tauri-gui-client`): the core
//! thread's pre-attach phase. It finds the Owner, starts one from the `mbv`
//! program when none runs, and attaches — and for every outcome that cannot
//! attach immediately, it shows the window state that
//! `startup::run_local_instance` gives the TUI Client. There is no `App`
//! before the attach, so the campaign resolves the mismatch state's own keys
//! and button presses itself; the front end interprets nothing. The campaign
//! ends when the attach succeeds, the user quits the mismatch state, or the
//! window closes — the close arrives as a [`CoreInput::Close`], because the
//! App's waker keeps the channel connected for the core thread's whole life.

use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvError, RecvTimeoutError};
use std::time::{Duration, Instant};

use mbv::{
    App, Config, OwnerStartOptions, OwnerState, attach_owner, exclusive_owner_refusal_text,
    find_owner, start_owner,
};
use mbv_config::owner_lock_path;
use mbv_daemon::{SignalOwnerError, signal_owner};
use mbv_remote_player::RemotePlayerError;
use mbv_surfaces::HitMap;
use mbv_surfaces::gesture::{PointerGesture, PointerGestureState};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PointerEvent, Position};

use crate::bridge::frame::GuiAttachState;
use crate::bridge::frame::theme::GuiTheme;
use crate::bridge::layout::{AttachTargetId, RegionTarget, hit_map};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};
use crate::core_thread::{CoreInput, QuitSink, SectionSink};
use crate::leaves::gesture_point;

/// The wait between the shutting-down Owner's polls, as the TUI's run waits
/// (`startup::run_local_instance`).
const OWNER_WAIT_POLL: Duration = Duration::from_millis(100);

/// How long the wait for a shutting-down Owner lasts before the window shows
/// the still-shutting-down state, as the TUI's run waits.
const OWNER_WAIT_LIMIT: Duration = Duration::from_secs(10);

/// The still-shutting-down state's text, as the TUI's run answers a wait that
/// outlives its deadline.
const STILL_SHUTTING_DOWN: &str =
    "the local owner is still shutting down; use `mbv -q` to stop it.";

/// One poll of a shutting-down Owner (design D9). Production checks the
/// campaign's deadline and then probes; the tests inject the outcome. Every
/// outcome resolves one campaign step, as the TUI's run returns the probe's
/// state and its loop acts on it: the wind-down's repeated attach attempts
/// come from the campaign loop re-entering the wait, not from the wait
/// polling internally.
#[derive(Debug)]
pub enum OwnerWait {
    /// The Owner exited: the lock is free, so the campaign starts an Owner
    /// and attaches.
    Exited,
    /// The probe sees a live Owner that accepts connections — another Client
    /// started a healthy Owner between polls, or the wind-down's socket still
    /// accepts. The campaign attaches to it and lets the handshake decide;
    /// it starts no second Owner, as the TUI's run attaches to the probe's
    /// state verbatim.
    Attachable,
    /// The probe sees a live Owner that refuses connections: the campaign
    /// parks the window on the refusal text, as the TUI's run does.
    Refused,
    /// The wait's deadline passed while the Owner still ran.
    TimedOut,
    /// The probe itself failed.
    Probe(io::Error),
}

/// The Owner steps the campaign runs (design D9). Production calls the `mbv`
/// library's public steps; the tests inject every outcome, so no live Owner,
/// socket, or process runs (design D13).
pub trait OwnerSteps {
    /// `find_owner`: the single-instance probe.
    fn find(&self) -> io::Result<OwnerState>;
    /// `start_owner` from the `mbv` program the campaign resolved.
    fn start(&self, program: &Path) -> Result<(), RemotePlayerError>;
    /// `attach_owner` as a Client of the running Owner.
    fn attach(&self, program: &Path) -> Result<App, RemotePlayerError>;
    /// One poll of a shutting-down Owner.
    fn wait_tick(&self) -> OwnerWait;
    /// `signal_owner` on the owner lock path: the mismatch restart's stop.
    fn signal(&self) -> Result<(), SignalOwnerError>;
    /// The refusal text for an Owner that holds the lock but accepts no
    /// connections (`refusal_text`).
    fn refusal_text(&self) -> String;
}

/// How the campaign ended without an `App` (design D9). The window's close
/// arrives as a [`CoreInput::Close`]; a channel that ends without one means
/// the same, and the mismatch Quit closes the window through the quit edge
/// before the campaign ends.
#[derive(Debug)]
pub enum CampaignEnd {
    /// The user quit the mismatch state (key Q or the Quit button): the quit
    /// edge closed the window.
    Quit,
    /// The window closed: the close arrived as a [`CoreInput::Close`], or the
    /// channel ended.
    Closed,
}

/// The mismatch prompt's choice (design D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptOutcome {
    Restart,
    Quit,
}

/// The campaign (design D9): the pre-attach states the window shows and the
/// Owner steps that resolve them. It runs on the core thread until the attach
/// succeeds, the user quits, or the window's channel disconnects.
pub struct AttachCampaign<'a> {
    rx: &'a Receiver<CoreInput>,
    sink: &'a SectionSink,
    quit: &'a QuitSink,
    steps: &'a dyn OwnerSteps,
    /// The wait between the shutting-down Owner's polls. Production keeps
    /// `OWNER_WAIT_POLL`; the tests zero it, so a poll step never blocks.
    poll_interval: Duration,
    /// The recognized pointer gestures over the window's logical points, the
    /// same recognizer the attached slice routes with (design D4). It lives
    /// across the campaign so a double-click pair is one gesture.
    gestures: PointerGestureState<Position>,
    /// The painted regions of the last report the mismatch state received
    /// (design D4): the Restart and Quit buttons a press resolves against.
    regions: HitMap<RegionTarget>,
    /// The frame the next attach emission carries the theme in (design D7):
    /// the first emission, and the first after a report re-arms it.
    theme_pending: bool,
    /// The revision the next attach section carries: it steps on each
    /// emission, so the window replaces its attach state each time.
    attach_revision: u64,
    /// The revision the next theme section carries.
    theme_revision: u64,
}

impl<'a> AttachCampaign<'a> {
    /// A campaign over the window's channel, frame sink, and quit edge, with
    /// the Owner steps that resolve its states (design D9).
    #[must_use]
    pub fn new(
        rx: &'a Receiver<CoreInput>,
        sink: &'a SectionSink,
        quit: &'a QuitSink,
        steps: &'a dyn OwnerSteps,
    ) -> Self {
        Self {
            rx,
            sink,
            quit,
            steps,
            poll_interval: OWNER_WAIT_POLL,
            gestures: PointerGestureState::new(),
            regions: HitMap::new(),
            theme_pending: true,
            attach_revision: 0,
            theme_revision: 0,
        }
    }

    /// Adopts the poll wait a test pins; production keeps `OWNER_WAIT_POLL`.
    /// A zeroed wait makes a poll's channel read non-blocking.
    pub fn set_poll_interval_for_test(&mut self, interval: Duration) {
        self.poll_interval = interval;
    }

    /// Runs the campaign to an attached `App` (design D9). The loop is
    /// `startup::run_local_instance`'s: probe, start an Owner when none runs,
    /// attach, and follow the attach error up. Every terminal outcome parks
    /// the window in its state until the window closes.
    pub fn run(&mut self) -> Result<App, CampaignEnd> {
        let program = owner_program();
        let mut state = self.initial_state()?;
        let mut restart_requested = false;
        loop {
            match state {
                OwnerState::Start => {
                    if let Err(error) = self.steps.start(&program) {
                        return self.park(&GuiAttachState::Failed(format!(
                            "failed to start Owner process: {error}"
                        )));
                    }
                }
                // A live Owner accepts connections; attach alongside it.
                OwnerState::Attach => {
                    tracing::info!(
                        name: "gui_core.owner_process.detected",
                        target: "gui_core",
                        "Owner process detected; attaching"
                    );
                }
                OwnerState::Refuse => {
                    return self.park(&GuiAttachState::Refusal(self.steps.refusal_text()));
                }
            }
            match self.steps.attach(&program) {
                Ok(app) => return Ok(app),
                Err(error) => {
                    state = self.follow_up(&error, &mut restart_requested)?;
                }
            }
        }
    }

    /// The first single-instance probe (design D9). A probe failure parks the
    /// window on the TUI's single-instance text, as `find_owner_or_fatal` does.
    fn initial_state(&mut self) -> Result<OwnerState, CampaignEnd> {
        match self.steps.find() {
            Ok(state) => Ok(state),
            Err(error) => self.park(&GuiAttachState::Failed(format!(
                "single-instance check failed: {error}"
            ))),
        }
    }

    /// The response an attach error gets (design D9), the window form of
    /// `startup::run_local_instance`'s follow-up: wait for a shutting-down
    /// Owner, prompt on the first mismatch (a mismatch after a requested
    /// restart waits again, as the TUI's does), refuse a second exclusive
    /// Client with the shared text, and park on anything else with the exact
    /// attach-failed text. Returns the state the loop re-runs with.
    fn follow_up(
        &mut self,
        error: &RemotePlayerError,
        restart_requested: &mut bool,
    ) -> Result<OwnerState, CampaignEnd> {
        if error.is_owner_shutting_down() {
            return self.wait_for_owner_exit();
        }
        if error.is_owner_mismatch() {
            if *restart_requested {
                return self.wait_for_owner_exit();
            }
            return self.mismatch_prompt(error, restart_requested);
        }
        if error.is_exclusive_owner() {
            return self.park(&GuiAttachState::ExclusiveRefusal(
                exclusive_owner_refusal_text(error),
            ));
        }
        self.park(&GuiAttachState::Failed(format!(
            "failed to attach to Owner process: {error}"
        )))
    }

    /// Shows the waiting state and runs one paced poll of the shutting-down
    /// Owner (design D9). The paced channel read comes first, as the TUI's
    /// wait sleeps before it probes: a report from the window re-shows the
    /// state, a close ends the wait, and the poll's outcome resolves one
    /// campaign step. The campaign loop re-enters the wait on the wind-down's
    /// next shutting-down failure, so the wind-down's repeated attach
    /// attempts pace one probe per entry, exactly as the TUI's loop re-runs
    /// its state.
    fn wait_for_owner_exit(&mut self) -> Result<OwnerState, CampaignEnd> {
        let waiting = GuiAttachState::Waiting;
        self.show(&waiting);
        match self.rx.recv_timeout(self.poll_interval) {
            Ok(CoreInput::Layout(..)) => self.reshow(&waiting),
            Ok(CoreInput::Close) | Err(RecvTimeoutError::Disconnected) => {
                return Err(CampaignEnd::Closed);
            }
            // The pre-attach window has no App, so a Library or Settings
            // command has nothing to apply: it is served and dropped as a key
            // is.
            Ok(
                CoreInput::Key(_)
                | CoreInput::Pointer(_)
                | CoreInput::Wake
                | CoreInput::Library(_)
                | CoreInput::Settings(_),
            )
            | Err(RecvTimeoutError::Timeout) => {}
        }
        match self.steps.wait_tick() {
            OwnerWait::Exited => Ok(OwnerState::Start),
            OwnerWait::Attachable => Ok(OwnerState::Attach),
            OwnerWait::Refused => self.park(&GuiAttachState::Refusal(self.steps.refusal_text())),
            OwnerWait::TimedOut => {
                self.park(&GuiAttachState::Failed(STILL_SHUTTING_DOWN.to_owned()))
            }
            OwnerWait::Probe(error) => self.park(&GuiAttachState::Failed(format!(
                "single-instance check failed: {error}"
            ))),
        }
    }

    /// Shows the mismatch state and waits for the user's Restart or Quit —
    /// key R and key Q, or a press on the reported button regions (design
    /// D9). Any other chord resolves to nothing: the window is not the TUI's
    /// read-line prompt, where any other line quits.
    fn mismatch_prompt(
        &mut self,
        error: &RemotePlayerError,
        restart_requested: &mut bool,
    ) -> Result<OwnerState, CampaignEnd> {
        let mismatch = GuiAttachState::Mismatch(format!(
            "{error}\nRestarting stops playback and closes any other mbv terminals."
        ));
        self.show(&mismatch);
        loop {
            match self.rx.recv() {
                Ok(CoreInput::Layout(report)) => {
                    self.regions = hit_map(&report.regions);
                    self.reshow(&mismatch);
                }
                Ok(CoreInput::Key(key)) => {
                    if let Some(outcome) = mismatch_key(key) {
                        return self.prompt_outcome(outcome, restart_requested);
                    }
                }
                Ok(CoreInput::Pointer(event)) => {
                    if let Some(outcome) = self.mismatch_pointer(event) {
                        return self.prompt_outcome(outcome, restart_requested);
                    }
                }
                // No App exists before the attach, so no Library or Settings
                // command can act; a wake serves the tick as it does after the
                // attach.
                Ok(CoreInput::Wake | CoreInput::Library(_) | CoreInput::Settings(_)) => {}
                Ok(CoreInput::Close) | Err(RecvError) => {
                    return Err(CampaignEnd::Closed);
                }
            }
        }
    }

    /// Runs the prompt's choice (design D9): Restart signals the Owner on the
    /// owner lock path, marks the restart requested, and waits for the Owner
    /// to exit; Quit closes the window through the quit edge.
    fn prompt_outcome(
        &mut self,
        outcome: PromptOutcome,
        restart_requested: &mut bool,
    ) -> Result<OwnerState, CampaignEnd> {
        match outcome {
            PromptOutcome::Restart => {
                if let Err(signal_error) = self.steps.signal() {
                    return self.park(&GuiAttachState::Failed(format!(
                        "failed to stop Owner process: {signal_error}"
                    )));
                }
                *restart_requested = true;
                self.wait_for_owner_exit()
            }
            PromptOutcome::Quit => {
                (self.quit)();
                Err(CampaignEnd::Quit)
            }
        }
    }

    /// The prompt choice a pointer gesture makes (design D9): a press on a
    /// reported Restart or Quit button region, resolved against the hit map
    /// of the last report the mismatch state received. A press on no reported
    /// region resolves to nothing (`client-interaction-core`: pointer targets
    /// resolve from a semantic hit map).
    fn mismatch_pointer(&mut self, event: PointerEvent) -> Option<PromptOutcome> {
        let gesture =
            self.gestures
                .recognize(event.kind, event.modifiers, event.position, Instant::now())?;
        let point = gesture_point(&gesture)?;
        // A left press acts, the second press of a double-click pair
        // included, exactly as the TUI's transport glyphs do; a right click
        // and the wheel resolve to nothing.
        if !matches!(
            gesture,
            PointerGesture::Click { .. } | PointerGesture::DoubleClick(_)
        ) {
            return None;
        }
        match self.regions.resolve(point) {
            Some(RegionTarget::Attach(AttachTargetId::Restart)) => Some(PromptOutcome::Restart),
            Some(RegionTarget::Attach(AttachTargetId::Quit)) => Some(PromptOutcome::Quit),
            _ => None,
        }
    }

    /// Parks the window in a state that answers nothing (design D9): the run
    /// ends only when the window closes — the close arrives as a
    /// [`CoreInput::Close`], and a channel that ends without one means the
    /// same. Between reports the loop serves the channel, so a reload
    /// re-shows the state with its theme; keys and pointer events resolve to
    /// nothing. The `Ok` type is the caller's, and no call site can reach it:
    /// the loop only ends through the close.
    fn park<T>(&mut self, state: &GuiAttachState) -> Result<T, CampaignEnd> {
        tracing::warn!(
            name: "gui_core.attach_parked",
            target: "gui_core",
            "the attach parked the window in a dead-end state"
        );
        self.show(state);
        loop {
            match self.rx.recv() {
                Ok(CoreInput::Layout(..)) => {
                    // The dead end paints no buttons, so the report carries no
                    // region the campaign resolves.
                    self.reshow(state);
                }
                // The pre-attach window has no App, so a Library or Settings
                // command has nothing to apply: it is served and dropped as a
                // key is.
                Ok(
                    CoreInput::Key(_)
                    | CoreInput::Pointer(_)
                    | CoreInput::Wake
                    | CoreInput::Library(_)
                    | CoreInput::Settings(_),
                ) => {}
                Ok(CoreInput::Close) | Err(RecvError) => {
                    return Err(CampaignEnd::Closed);
                }
            }
        }
    }

    /// Re-emits the state the window shows, as a `report_layout` asks
    /// (design D3's emission rule carried to the pre-attach phase), with the
    /// re-armed theme (design D7).
    fn reshow(&mut self, state: &GuiAttachState) {
        self.theme_pending = true;
        self.show(state);
    }

    /// Emits the attach frame for `state` (design D9). The frame carries the
    /// theme when armed (design D7): the first emission, and the first after
    /// a report re-arms it.
    fn show(&mut self, state: &GuiAttachState) {
        let theme_changed = std::mem::take(&mut self.theme_pending);
        self.attach_revision = self.attach_revision.wrapping_add(1);
        let theme = theme_changed.then(|| {
            self.theme_revision = self.theme_revision.wrapping_add(1);
            GuiSection {
                revision: self.theme_revision,
                value: GuiTheme::current(),
            }
        });
        (self.sink)(GuiSectionBatch {
            attach: Some(GuiSection {
                revision: self.attach_revision,
                value: Some(state.clone()),
            }),
            theme,
            ..GuiSectionBatch::default()
        });
    }
}

impl std::fmt::Debug for AttachCampaign<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttachCampaign")
            .field("poll_interval", &self.poll_interval)
            .field("regions", &self.regions.len())
            .field("theme_pending", &self.theme_pending)
            .finish_non_exhaustive()
    }
}

/// The prompt choice a key chord makes (design D9): key R restarts and key Q
/// quits, with no modifier beyond Shift held — the window form of the TUI
/// prompt's `r`/`R` line. Any other chord resolves to nothing.
fn mismatch_key(key: KeyEvent) -> Option<PromptOutcome> {
    let plain = key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT;
    match (key.code, plain) {
        (Key::Char('r' | 'R'), true) => Some(PromptOutcome::Restart),
        (Key::Char('q' | 'Q'), true) => Some(PromptOutcome::Quit),
        _ => None,
    }
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

/// The campaign's production steps (design D9): the `mbv` library's public
/// Owner steps, with the TUI run's one wait deadline for a shutting-down
/// Owner.
pub(crate) struct LibraryOwnerSteps<'a> {
    config: &'a Config,
    /// The wait's deadline, set at the first poll and never reset — the TUI
    /// run's one deadline per launch (`startup::run_local_instance`'s
    /// `shutdown_deadline`).
    wait_deadline: Cell<Option<Instant>>,
}

impl<'a> LibraryOwnerSteps<'a> {
    /// The production steps over the campaign's configuration.
    pub(crate) fn new(config: &'a Config) -> Self {
        Self {
            config,
            wait_deadline: Cell::new(None),
        }
    }
}

impl OwnerSteps for LibraryOwnerSteps<'_> {
    fn find(&self) -> io::Result<OwnerState> {
        find_owner()
    }

    fn start(&self, program: &Path) -> Result<(), RemotePlayerError> {
        start_owner(&OwnerStartOptions {
            owner_exe: Some(program),
            log_level: None,
        })
    }

    fn attach(&self, program: &Path) -> Result<App, RemotePlayerError> {
        attach_owner(self.config, Some(program))
    }

    fn wait_tick(&self) -> OwnerWait {
        // The deadline is the TUI run's: set at the first poll, never reset,
        // and a past-deadline poll ends the wait (design D9).
        let deadline = if let Some(deadline) = self.wait_deadline.get() {
            deadline
        } else {
            let deadline = Instant::now() + OWNER_WAIT_LIMIT;
            self.wait_deadline.set(Some(deadline));
            deadline
        };
        if Instant::now() >= deadline {
            return OwnerWait::TimedOut;
        }
        match find_owner() {
            // The Owner exited: the lock is free, so the campaign starts an
            // Owner and attaches.
            Ok(OwnerState::Start) => OwnerWait::Exited,
            // A live Owner accepts connections — another Client's healthy
            // Owner, or the wind-down's socket. The campaign attaches to it
            // and lets the handshake decide; it starts no second Owner, as
            // the TUI's run attaches to the probe's state verbatim.
            Ok(OwnerState::Attach) => OwnerWait::Attachable,
            Ok(OwnerState::Refuse) => OwnerWait::Refused,
            Err(error) => OwnerWait::Probe(error),
        }
    }

    fn signal(&self) -> Result<(), SignalOwnerError> {
        let lock_path = owner_lock_path();
        signal_owner(&lock_path).map(|_| ())
    }

    fn refusal_text(&self) -> String {
        mbv::refusal_text()
    }
}
