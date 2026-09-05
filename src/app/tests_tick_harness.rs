use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use tuirealm::application::PollStrategy;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};

use crate::app::components::{ComponentId, Msg, UserEvent};
use crate::app::router::RouterOutcome;
use crate::app::shell::{apply_router_outcome, fold_mouse_messages, Model};
use crate::app::App;

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

pub(in crate::app) struct TickHarness {
    tx: Sender<Event<UserEvent>>,
    model: Model,
}

#[derive(Debug)]
pub(in crate::app) struct StepOutcome {
    pub(in crate::app) raw_messages: Vec<Msg>,
    pub(in crate::app) messages: Vec<Msg>,
    pub(in crate::app) pre_fold_focus: Option<ComponentId>,
    pub(in crate::app) router: RouterOutcome,
}

impl TickHarness {
    pub(in crate::app) fn new(app: App) -> Self {
        let (tx, rx) = mpsc::channel();
        let port = InjectPort { rx };
        let listener_cfg = EventListenerCfg::default().add_port(
            Box::new(port),
            INJECT_PORT_INTERVAL,
            INJECT_PORT_MAX_POLL,
        );
        let model = Model::new_with_listener(app, listener_cfg);
        Self { tx, model }
    }

    pub(in crate::app) fn model(&self) -> &Model {
        &self.model
    }

    pub(in crate::app) fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    pub(in crate::app) fn inject(&self, event: Event<UserEvent>) {
        self.tx.send(event).expect("inject event");
    }

    pub(in crate::app) fn step(&mut self) -> StepOutcome {
        self.model.sync_mounted_surfaces();
        if let Some(Msg::Service(request)) = self.model.tick_search_clock(Instant::now()) {
            self.model.handle_service_request(request);
        }
        let pre_fold_focus = self.model.application.focus().cloned();
        let raw_messages = self
            .model
            .application
            .tick(PollStrategy::Once(TICK_TIMEOUT))
            .expect("tick injected event");
        // Mirror the run loop: the mouse fold runs before the keyboard router
        // fold (ADR 0024). A keyboard tick passes the mouse fold untouched.
        let folded = fold_mouse_messages(raw_messages.clone());
        let router = self.model.router_outcome(&folded);
        let messages = apply_router_outcome(folded, pre_fold_focus.as_ref(), &router);
        StepOutcome {
            raw_messages,
            messages,
            pre_fold_focus,
            router,
        }
    }
}

#[test]
fn injected_key_reaches_application_tick() {
    let mut harness = TickHarness::new(crate::app::tests::make_app_stub());
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
