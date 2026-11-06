//! Public-API tests of the Owner attach campaign (`mbv_gui::attach`): each
//! outcome and the window state the campaign shows for it, and the flows
//! D9 names — the restart, the repeated mismatch, the quit edge, and the
//! button regions — over injected Owner steps (design D13: no live Owner, no
//! socket, no process, no wait).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use mbv::{App, OwnerState, exclusive_owner_refusal_text, stub_app_with_queue};
use mbv_daemon::SignalOwnerError;
use mbv_gui::attach::{AttachCampaign, CampaignEnd, OwnerSteps, OwnerWait};
use mbv_gui::bridge::frame::{GuiAttachState, GuiWindowFrame};
use mbv_gui::bridge::layout::{
    AttachTargetId, LayoutRect, LayoutRegion, LayoutReport, RegionTarget,
};
use mbv_gui::core_thread::{CoreInput, FrameSink, QuitSink};
use mbv_remote_player::RemotePlayerError;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PointerEvent, PointerKind, Position};

/// The refusal text the mock's probe refusal carries, as the campaign's
/// `refusal_text` step answers.
const MOCK_REFUSAL: &str = "the mock refusal text";

/// The still-shutting-down text, as the wait's deadline answers.
const STILL_SHUTTING_DOWN: &str =
    "the local owner is still shutting down; use `mbv -q` to stop it.";

/// The attach outcome a case injects, as the campaign's steps report it.
#[derive(Debug, Clone, Copy)]
enum InjectedOutcome {
    /// The single-instance probe reports a live Owner that refuses
    /// connections.
    RefusingOwner,
    /// The attach fails: stay-alive is off and a Client is attached.
    ExclusiveOwner,
    /// The attach fails: the Owner is shutting down.
    OwnerShuttingDown,
    /// The attach fails: the Owner's build differs.
    OwnerBuildMismatch,
    /// The attach fails with an unrelated error.
    OtherError,
}

/// The window-state kind the outcome shows, as the table matches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShownState {
    Refusal,
    ExclusiveRefusal,
    Waiting,
    Mismatch,
    Failed,
}

impl ShownState {
    /// Whether `state` carries this kind, whatever text it carries.
    fn matches(self, state: &GuiAttachState) -> bool {
        matches!(
            (self, state),
            (Self::Refusal, GuiAttachState::Refusal(_))
                | (Self::ExclusiveRefusal, GuiAttachState::ExclusiveRefusal(_))
                | (Self::Waiting, GuiAttachState::Waiting)
                | (Self::Mismatch, GuiAttachState::Mismatch(_))
                | (Self::Failed, GuiAttachState::Failed(_))
        )
    }
}

/// The exclusive-owner refusal, as a live Owner with stay-alive off reports.
fn exclusive_error() -> RemotePlayerError {
    RemotePlayerError::exclusive_owner_for_test(4242)
}

/// The shutting-down refusal, as an Owner winding down reports.
fn shutting_down_error() -> RemotePlayerError {
    RemotePlayerError::owner_shutting_down_for_test()
}

/// The build-mismatch refusal, as an Owner from another build reports.
fn mismatch_error() -> RemotePlayerError {
    RemotePlayerError::owner_build_mismatch_for_test("1.2.3", "build-a")
}

/// An unrelated attach error, as a failed connection reports.
fn other_error() -> RemotePlayerError {
    RemotePlayerError::from(io::Error::new(
        io::ErrorKind::ConnectionRefused,
        "connection refused",
    ))
}

/// The injected Owner steps (design D13): every step returns the outcome its
/// test queued, so no Owner, socket, or process runs.
struct MockSteps {
    find_results: RefCell<VecDeque<io::Result<OwnerState>>>,
    attach_results: RefCell<VecDeque<Result<App, RemotePlayerError>>>,
    wait_ticks: RefCell<VecDeque<OwnerWait>>,
    signals: Cell<usize>,
    refusal: String,
}

impl OwnerSteps for MockSteps {
    fn find(&self) -> io::Result<OwnerState> {
        self.find_results
            .borrow_mut()
            .pop_front()
            .expect("a find outcome is queued")
    }

    fn start(&self, _program: &Path) -> Result<(), RemotePlayerError> {
        Ok(())
    }

    fn attach(&self, _program: &Path) -> Result<App, RemotePlayerError> {
        self.attach_results
            .borrow_mut()
            .pop_front()
            .expect("an attach outcome is queued")
    }

    fn wait_tick(&self) -> OwnerWait {
        self.wait_ticks
            .borrow_mut()
            .pop_front()
            .expect("a wait outcome is queued")
    }

    fn signal(&self) -> Result<(), SignalOwnerError> {
        self.signals.set(self.signals.get() + 1);
        Ok(())
    }

    fn refusal_text(&self) -> String {
        self.refusal.clone()
    }
}

impl MockSteps {
    /// A mock whose probe reports a refusing Owner: the campaign parks before
    /// any attach, so no attach or wait outcome is queued.
    fn refusing_probe() -> Self {
        Self {
            find_results: RefCell::new(VecDeque::from([Ok(OwnerState::Refuse)])),
            attach_results: RefCell::new(VecDeque::new()),
            wait_ticks: RefCell::new(VecDeque::new()),
            signals: Cell::new(0),
            refusal: MOCK_REFUSAL.to_owned(),
        }
    }

    /// A mock whose probe reports a live Owner, whose attach fails with
    /// `errors` in order and then succeeds with a stub App, and whose wait
    /// polls return `ticks` in order.
    fn attach_fails(errors: Vec<RemotePlayerError>, ticks: Vec<OwnerWait>) -> Self {
        let mut attach_results: VecDeque<Result<App, RemotePlayerError>> =
            errors.into_iter().map(Err).collect();
        attach_results.push_back(Ok(stub_app_with_queue(0)));
        Self {
            find_results: RefCell::new(VecDeque::from([Ok(OwnerState::Attach)])),
            attach_results: RefCell::new(attach_results),
            wait_ticks: RefCell::new(VecDeque::from(ticks)),
            signals: Cell::new(0),
            refusal: MOCK_REFUSAL.to_owned(),
        }
    }

    fn restarts(&self) -> usize {
        self.signals.get()
    }
}

/// The mock a table case injects (design D9).
fn mock_for(outcome: InjectedOutcome) -> MockSteps {
    match outcome {
        InjectedOutcome::RefusingOwner => MockSteps::refusing_probe(),
        InjectedOutcome::ExclusiveOwner => MockSteps::attach_fails(vec![exclusive_error()], vec![]),
        InjectedOutcome::OwnerShuttingDown => {
            MockSteps::attach_fails(vec![shutting_down_error()], vec![OwnerWait::Exited])
        }
        InjectedOutcome::OwnerBuildMismatch => {
            MockSteps::attach_fails(vec![mismatch_error()], vec![])
        }
        InjectedOutcome::OtherError => MockSteps::attach_fails(vec![other_error()], vec![]),
    }
}

/// The harness a test drives: the frame capture and the quit capture. The
/// channel's sender and receiver travel separately, so a test drops the
/// sender to end the campaign the way a closing window does.
struct Harness {
    sink: FrameSink,
    quit: QuitSink,
    frames: mpsc::Receiver<GuiWindowFrame>,
    quits: mpsc::Receiver<()>,
}

impl Harness {
    /// A harness, its channel's sender, and its channel's receive end, which
    /// the campaign borrows.
    fn new() -> (Self, mpsc::Sender<CoreInput>, mpsc::Receiver<CoreInput>) {
        let (tx, rx) = mpsc::channel();
        let (frame_tx, frames) = mpsc::channel();
        let (quit_tx, quits) = mpsc::channel();
        let sink: FrameSink = Box::new(move |frame| {
            let _ = frame_tx.send(frame);
        });
        let quit: QuitSink = Box::new(move || {
            let _ = quit_tx.send(());
        });
        (
            Self {
                sink,
                quit,
                frames,
                quits,
            },
            tx,
            rx,
        )
    }

    /// The attach states the campaign emitted, in order.
    fn attach_states(&self) -> Vec<GuiAttachState> {
        self.drain()
            .into_iter()
            .filter_map(|frame| match frame {
                GuiWindowFrame::Attach(attach) => Some(attach.state),
                GuiWindowFrame::Slice(_) => None,
            })
            .collect()
    }

    /// The campaign emitted no slice frame: it never reaches the attached
    /// tick loop.
    fn assert_no_slice_frames(&self) {
        assert!(
            !self
                .drain()
                .iter()
                .any(|frame| matches!(frame, GuiWindowFrame::Slice(_))),
            "the campaign emitted a slice frame"
        );
    }

    /// The quit edge fired.
    fn assert_quit_fired(&self) {
        self.quits.try_recv().expect("the quit edge fired");
    }

    fn drain(&self) -> Vec<GuiWindowFrame> {
        std::iter::from_fn(|| self.frames.try_recv().ok()).collect()
    }
}

/// The campaign over the harness's sink and quit edge, the channel's receive
/// end, and the injected steps. The wait's poll interval is zeroed, so a
/// poll step never blocks.
fn campaign<'a>(
    sink: &'a FrameSink,
    quit: &'a QuitSink,
    rx: &'a mpsc::Receiver<CoreInput>,
    steps: &'a dyn OwnerSteps,
) -> AttachCampaign<'a> {
    let mut campaign = AttachCampaign::new(rx, sink, quit, steps);
    campaign.set_poll_interval_for_test(Duration::ZERO);
    campaign
}

/// A plain letter press with no modifier held, as the translated `key`
/// command carries it.
fn key_press(letter: char) -> CoreInput {
    CoreInput::Key(KeyEvent::new(Key::Char(letter), KeyModifiers::NONE))
}

/// A layout report the front end measured with `regions` painted, as a
/// report on the attach screens carries.
fn layout_with(regions: Vec<LayoutRegion>) -> CoreInput {
    CoreInput::Layout(LayoutReport {
        lists: vec![],
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

/// Each attach outcome and the window state the campaign shows for it
/// (design D9, task row 2.6): one row per outcome.
#[rstest::rstest]
#[case::refuse(InjectedOutcome::RefusingOwner, ShownState::Refusal)]
#[case::exclusive(InjectedOutcome::ExclusiveOwner, ShownState::ExclusiveRefusal)]
#[case::shutting_down(InjectedOutcome::OwnerShuttingDown, ShownState::Waiting)]
#[case::mismatch(InjectedOutcome::OwnerBuildMismatch, ShownState::Mismatch)]
#[case::other(InjectedOutcome::OtherError, ShownState::Failed)]
fn each_attach_outcome_shows_its_window_state(
    #[case] outcome: InjectedOutcome,
    #[case] expected: ShownState,
) {
    let (harness, tx, rx) = Harness::new();
    drop(tx);
    let steps = mock_for(outcome);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    let states = harness.attach_states();
    assert_eq!(states.len(), 1, "the outcome shows one state: {states:?}");
    assert!(expected.matches(&states[0]), "the outcome shows {states:?}");
    harness.assert_no_slice_frames();
}

/// The generic attach error's text is exact (design D9): the window shows
/// `failed to attach to Owner process: {error}` and nothing else.
#[test]
fn the_generic_attach_error_text_is_exact() {
    let expected = format!("failed to attach to Owner process: {}", other_error());
    let (harness, tx, rx) = Harness::new();
    drop(tx);
    let steps = MockSteps::attach_fails(vec![other_error()], vec![]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    assert_eq!(
        harness.attach_states(),
        vec![GuiAttachState::Failed(expected)]
    );
}

/// The refusal text comes from the shared `refusal_text` step, byte for byte.
#[test]
fn the_refusal_text_is_carried_verbatim() {
    let (harness, tx, rx) = Harness::new();
    drop(tx);
    let steps = MockSteps::refusing_probe();
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    assert_eq!(
        harness.attach_states(),
        vec![GuiAttachState::Refusal(MOCK_REFUSAL.to_owned())]
    );
}

/// The exclusive refusal comes from the shared text the TUI Client gets,
/// byte for byte (spec `gui-client`: the same explanation a second TUI
/// Client gets).
#[test]
fn the_exclusive_refusal_uses_the_shared_text() {
    let expected = exclusive_owner_refusal_text(&exclusive_error());
    let (harness, tx, rx) = Harness::new();
    drop(tx);
    let steps = MockSteps::attach_fails(vec![exclusive_error()], vec![]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    assert_eq!(
        harness.attach_states(),
        vec![GuiAttachState::ExclusiveRefusal(expected)]
    );
}

/// The mismatch text names the mismatch and the restart's cost, as the TUI
/// prompt's text does without its key line.
#[test]
fn the_mismatch_text_names_the_mismatch_and_the_restart_cost() {
    let expected = format!(
        "{}\nRestarting stops playback and closes any other mbv terminals.",
        mismatch_error()
    );
    let (harness, tx, rx) = Harness::new();
    drop(tx);
    let steps = MockSteps::attach_fails(vec![mismatch_error()], vec![]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    assert_eq!(
        harness.attach_states(),
        vec![GuiAttachState::Mismatch(expected)]
    );
}

/// The Restart choice signals the Owner, waits for its exit, starts a new
/// Owner, and attaches (design D9): key R runs the whole chain.
#[test]
fn a_requested_restart_signals_waits_and_attaches() {
    let (harness, tx, rx) = Harness::new();
    tx.send(key_press('r')).unwrap();
    let steps = MockSteps::attach_fails(vec![mismatch_error()], vec![OwnerWait::Exited]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(outcome.is_ok(), "the restart attached: {outcome:?}");
    assert_eq!(steps.restarts(), 1);
    let states = harness.attach_states();
    assert_eq!(states.len(), 2);
    assert!(ShownState::Mismatch.matches(&states[0]));
    assert!(ShownState::Waiting.matches(&states[1]));
}

/// A mismatch after a requested restart waits again instead of prompting, as
/// the TUI's run does (design D9): the second mismatch shows only the
/// waiting state.
#[test]
fn a_mismatch_after_a_requested_restart_waits_instead_of_prompting() {
    let (harness, tx, rx) = Harness::new();
    tx.send(key_press('r')).unwrap();
    let steps = MockSteps::attach_fails(
        vec![mismatch_error(), mismatch_error()],
        vec![OwnerWait::Exited, OwnerWait::Exited],
    );
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(outcome.is_ok(), "the second wait attached: {outcome:?}");
    assert_eq!(steps.restarts(), 1);
    let states = harness.attach_states();
    assert_eq!(states.len(), 3);
    assert!(ShownState::Mismatch.matches(&states[0]));
    assert!(ShownState::Waiting.matches(&states[1]));
    assert!(ShownState::Waiting.matches(&states[2]));
}

/// The Quit choice closes the window through the quit edge and ends the
/// campaign (design D9).
#[test]
fn the_quit_key_ends_the_campaign_through_the_quit_edge() {
    let (harness, tx, rx) = Harness::new();
    tx.send(key_press('q')).unwrap();
    let steps = MockSteps::attach_fails(vec![mismatch_error()], vec![]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(matches!(outcome, Err(CampaignEnd::Quit)));
    harness.assert_quit_fired();
    let states = harness.attach_states();
    assert_eq!(states.len(), 1);
    assert!(ShownState::Mismatch.matches(&states[0]));
}

/// The Restart button resolves through the reported regions (design D9): a
/// press on the reported region runs the same restart the key R runs.
#[test]
fn the_restart_button_resolves_through_the_reported_regions() {
    let (harness, tx, rx) = Harness::new();
    tx.send(layout_with(vec![region_at(
        0.0,
        0.0,
        RegionTarget::Attach(AttachTargetId::Restart),
    )]))
    .unwrap();
    tx.send(press_at(50.0, 10.0)).unwrap();
    let steps = MockSteps::attach_fails(vec![mismatch_error()], vec![OwnerWait::Exited]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(outcome.is_ok(), "the button restart attached: {outcome:?}");
    assert_eq!(steps.restarts(), 1);
    let states = harness.attach_states();
    assert_eq!(states.len(), 3);
    assert!(ShownState::Mismatch.matches(&states[0]));
    assert!(ShownState::Mismatch.matches(&states[1]));
    assert!(ShownState::Waiting.matches(&states[2]));
}

/// The Quit button resolves through the reported regions (design D9): a
/// press on the reported region closes the window through the quit edge.
#[test]
fn the_quit_button_resolves_through_the_reported_regions() {
    let (harness, tx, rx) = Harness::new();
    tx.send(layout_with(vec![region_at(
        0.0,
        100.0,
        RegionTarget::Attach(AttachTargetId::Quit),
    )]))
    .unwrap();
    tx.send(press_at(50.0, 110.0)).unwrap();
    let steps = MockSteps::attach_fails(vec![mismatch_error()], vec![]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(matches!(outcome, Err(CampaignEnd::Quit)));
    harness.assert_quit_fired();
}

/// The wait polls a still-running Owner until it exits (design D9): a poll
/// that sees the Owner still running shows the waiting state again and keeps
/// polling.
#[test]
fn the_wait_polls_until_the_owner_exits() {
    let (harness, _tx, rx) = Harness::new();
    let steps = MockSteps::attach_fails(
        vec![shutting_down_error()],
        vec![OwnerWait::StillRunning, OwnerWait::Exited],
    );
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(outcome.is_ok(), "the wait attached: {outcome:?}");
    let states = harness.attach_states();
    assert_eq!(states.len(), 1);
    assert!(ShownState::Waiting.matches(&states[0]));
}

/// A wait whose deadline passes shows the still-shutting-down state the TUI's
/// run answers a wait that outlives its deadline, after the waiting state it
/// shows first.
#[test]
fn a_wait_past_its_deadline_shows_the_still_shutting_down_state() {
    let (harness, tx, rx) = Harness::new();
    drop(tx);
    let steps = MockSteps::attach_fails(vec![shutting_down_error()], vec![OwnerWait::TimedOut]);
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    assert_eq!(
        harness.attach_states(),
        vec![
            GuiAttachState::Waiting,
            GuiAttachState::Failed(STILL_SHUTTING_DOWN.to_owned())
        ]
    );
}

/// A report during a dead-end state re-shows the state (design D3's emission
/// rule carried to the pre-attach phase): a reload of the window content
/// re-shows the state it is parked in.
#[test]
fn a_report_during_a_dead_end_re_shows_the_state() {
    let (harness, tx, rx) = Harness::new();
    tx.send(layout_with(vec![])).unwrap();
    drop(tx);
    let steps = MockSteps::refusing_probe();
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let outcome = campaign.run();
    assert!(matches!(outcome, Err(CampaignEnd::Disconnected)));
    let states = harness.attach_states();
    assert_eq!(states.len(), 2);
    assert!(ShownState::Refusal.matches(&states[0]));
    assert!(ShownState::Refusal.matches(&states[1]));
    harness.assert_no_slice_frames();
}

/// A report re-arms the attach frame's theme (design D7): the first attach
/// frame carries the theme, and the frame a report triggers carries it
/// again, so a reload re-shows the state with its theme.
#[test]
fn a_report_re_arms_the_attach_theme() {
    let (harness, tx, rx) = Harness::new();
    tx.send(layout_with(vec![])).unwrap();
    drop(tx);
    let steps = MockSteps::refusing_probe();
    let mut campaign = campaign(&harness.sink, &harness.quit, &rx, &steps);
    let _outcome = campaign.run();
    let frames = harness.drain();
    assert_eq!(frames.len(), 2);
    let (first, second) = (&frames[0], &frames[1]);
    let (GuiWindowFrame::Attach(first), GuiWindowFrame::Attach(second)) = (first, second) else {
        panic!("the campaign emitted only attach frames");
    };
    assert!(first.theme.is_some());
    assert!(second.theme.is_some());
}
