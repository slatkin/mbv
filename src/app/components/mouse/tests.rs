use super::*;
use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::application::{Application, PollStrategy};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEvent};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use crate::app::components::{Msg, TerminalObserverEvent};

/// Records that a mouse event was forwarded to it, and emits a message so
/// the forward is observable out of `Application::tick`.
struct MouseProbe;

impl Component for MouseProbe {
    fn view(&mut self, _f: &mut Frame, _a: Rect) {}
    fn query<'a>(&'a self, _a: Attribute) -> Option<QueryResult<'a>> {
        None
    }
    fn attr(&mut self, _a: Attribute, _v: AttrValue) {}
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _c: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for MouseProbe {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Mouse(_) => Some(Msg::TerminalEvent(TerminalObserverEvent::NoOp)),
            _ => None,
        }
    }
}

struct InjectPort {
    rx: std::sync::mpsc::Receiver<Event<UserEvent>>,
}

impl Poll<UserEvent> for InjectPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        Ok(self.rx.try_recv().ok())
    }
}

fn every_mouse_kind() -> Vec<MouseEventKind> {
    vec![
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Down(MouseButton::Right),
        MouseEventKind::Down(MouseButton::Middle),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Moved,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ]
}

/// The `mouse_sub()` clause must forward *every* `MouseEventKind` at
/// arbitrary coordinates. If a future `tuirealm` starts honouring `kind`
/// in `is_in_range`, the `Moved` clause would stop forwarding the other
/// kinds and this test fails loudly (ADR 0024 pinned-dependency note).
#[test]
fn mouse_sub_forwards_every_kind_at_any_coordinate() {
    for kind in every_mouse_kind() {
        let (tx, rx) = std::sync::mpsc::channel();
        let cfg = EventListenerCfg::default().add_port(
            Box::new(InjectPort { rx }),
            std::time::Duration::from_millis(1),
            8,
        );
        let mut app: Application<ComponentId, Msg, UserEvent> = Application::init(cfg);
        app.mount(
            ComponentId::Playback,
            Box::new(MouseProbe),
            vec![mouse_sub()],
        )
        .expect("mount probe");

        tx.send(Event::Mouse(MouseEvent {
            kind,
            modifiers: KeyModifiers::NONE,
            column: 4321,
            row: 9876,
        }))
        .unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        let mut messages = Vec::new();
        while messages.is_empty() && std::time::Instant::now() < deadline {
            messages = app
                .tick(PollStrategy::Once(std::time::Duration::from_millis(50)))
                .expect("tick");
        }
        assert_eq!(
            messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::NoOp)],
            "mouse_sub() must forward {kind:?} at arbitrary coordinates"
        );
    }
}
