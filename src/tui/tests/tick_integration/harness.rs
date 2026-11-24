use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use tuirealm::application::PollStrategy;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};

use crate::tui::{Model, fold_mouse_messages};
use mbv_client_core::App;
use mbv_client_core::input::router::RouterOutcome;
use mbv_ui_msg::{ComponentId, Msg, UserEvent};

const INJECT_PORT_INTERVAL: Duration = Duration::from_millis(1);
const INJECT_PORT_MAX_POLL: usize = 8;
// D5: bounded wait for the listener worker to deliver one injected event;
// never sleep here, so wiring regressions fail instead of hanging CI.
const TICK_TIMEOUT: Duration = Duration::from_millis(500);

struct InjectPort {
    rx: Receiver<Event<UserEvent>>,
}

impl Poll<UserEvent> for InjectPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        Ok(self.rx.try_recv().ok())
    }
}

pub(in crate::tui) struct TickHarness {
    tx: Sender<Event<UserEvent>>,
    model: Model,
}

#[derive(Debug)]
pub(in crate::tui) struct StepOutcome {
    pub(in crate::tui) raw_messages: Vec<Msg>,
    pub(in crate::tui) messages: Vec<Msg>,
    pub(in crate::tui) pre_fold_focus: Option<ComponentId>,
    pub(in crate::tui) router: RouterOutcome,
    /// Whether the shell's deferred candidate fired on this step (the leaf
    /// did not consume and the command was dispatched).
    pub(in crate::tui) deferred_fired: bool,
}

impl TickHarness {
    pub(in crate::tui) fn new(app: App) -> Self {
        let (tx, port) = Self::port();
        let model = Model::new_with_listener(app, port);
        Self { tx, model }
    }

    /// A harness over a shell the caller built itself (`Model::new_with_listener`
    /// with [`TickHarness::port`]): state a test installs on the `Model` —
    /// the image protocol store, say — survives into the harness.
    pub(in crate::tui) fn from_model(model: Model) -> Self {
        let (tx, _) = Self::port();
        Self { tx, model }
    }

    /// The listener configuration and its event sender: build a `Model` over
    /// it, then hand the model to [`TickHarness::from_model`].
    pub(in crate::tui) fn port() -> (Sender<Event<UserEvent>>, EventListenerCfg<UserEvent>) {
        let (tx, rx) = mpsc::channel();
        let port = InjectPort { rx };
        let listener_cfg = EventListenerCfg::default().add_port(
            Box::new(port),
            INJECT_PORT_INTERVAL,
            INJECT_PORT_MAX_POLL,
        );
        (tx, listener_cfg)
    }

    pub(in crate::tui) fn model(&self) -> &Model {
        &self.model
    }

    pub(in crate::tui) fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    pub(in crate::tui) fn inject(&self, event: Event<UserEvent>) {
        self.tx.send(event).expect("inject event");
    }

    pub(in crate::tui) fn step(&mut self) -> StepOutcome {
        // The run loop drains one Player event before Application::tick();
        // process it here too so integration tests exercise the same event →
        // mounted-surface sync path. `drain_player_event` replays the events
        // the queue-op answer pump deferred before the live receiver (row 5.1).
        self.model.drain_suspended_home_events();
        self.model.app.drain_player_event();
        // Mirror the run loop's pending-transfer settlement, which runs in the
        // same tick phase that adopts owner snapshots (row 7.1, design D2).
        self.model.app.settle_pending_queue_transfer_for_test();
        self.model.sync_mounted_surfaces();
        if let Some(Msg::Service(request)) = self.model.tick_search_clock(Instant::now()) {
            self.model.handle_service_request(request);
        }
        // Mirror the run loop's Inline Search debounce sweep (shell-supplied
        // wall clock into the embedded control).
        self.model.tick_inline_search_clock(Instant::now());
        self.model.drain_transport_events();
        let pre_fold_focus = self.model.application.focus().cloned();
        let raw_messages = self
            .model
            .application
            .tick(PollStrategy::Once(TICK_TIMEOUT))
            .expect("tick injected event");
        // Mirror the run loop: the right-press record runs on the raw tick
        // messages, before any is drained, so the focused panel's own menu
        // request is pointer-anchored in the same tick (design D6).
        self.model.record_right_press(&raw_messages);
        // Mirror the run loop: the mouse fold runs before the keyboard router
        // fold (ADR 0024). A keyboard tick passes the mouse fold untouched.
        let folded = fold_mouse_messages(raw_messages.clone());
        let router = self.model.router_outcome(&folded);
        let (messages, diagnostic) =
            crate::tui::arbitrate_key(folded, pre_fold_focus.as_ref(), &router);
        let deferred_fired = self
            .model
            .apply_deferred_candidate(&router, diagnostic.leaf_disposition == "consumed");
        StepOutcome {
            raw_messages,
            messages,
            pre_fold_focus,
            router,
            deferred_fired,
        }
    }
}

#[test]
fn injected_key_reaches_application_tick() {
    let mut harness = TickHarness::new(mbv_client_core::tests::make_app_stub());
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::NONE,
    }));

    let outcome = harness.step();

    assert!(
        !outcome.raw_messages.is_empty(),
        "injected key must produce at least one tick message"
    );
}
