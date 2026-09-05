use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use tuirealm::subscription::{EventClause, Sub, SubClause};

use super::{ComponentId, Msg, TerminalObserverEvent, UserEvent};

const OVERLAY_IDS: &[ComponentId] = &[
    ComponentId::Overlay(super::OverlayId::Settings),
    ComponentId::Overlay(super::OverlayId::Playlists),
    ComponentId::Modal(super::ModalId::SavePlaylist),
    ComponentId::Overlay(super::OverlayId::Help),
    ComponentId::Modal(super::ModalId::Confirm),
    ComponentId::Modal(super::ModalId::DaemonLost),
    ComponentId::Modal(super::ModalId::RemoteReanchor),
    ComponentId::Overlay(super::OverlayId::ContextMenu),
    ComponentId::Overlay(super::OverlayId::SelectionModal),
    ComponentId::Popup(super::PopupId::Multiselect),
    ComponentId::Popup(super::PopupId::LibraryRoutes),
    ComponentId::Popup(super::PopupId::FeedManage),
    ComponentId::Overlay(super::OverlayId::Search),
    ComponentId::Overlay(super::OverlayId::Sessions),
];

/// Root routing owns overlay z-order from a fixed canonical mount order;
/// TuiRealm owns focus and its LIFO stack.
pub(in crate::app) struct UiRootComponent;

impl UiRootComponent {
    pub(in crate::app) fn new() -> Self {
        Self
    }

    /// Subscribe the root to every terminal event so the shell can distinguish
    /// a processed event from an empty component message. The root remains a
    /// permanent observer even while another component owns focus.
    pub(in crate::app) fn subscriptions() -> Vec<Sub<ComponentId, UserEvent>> {
        vec![Sub::new(EventClause::Any, SubClause::Always)]
    }

    pub(in crate::app) fn overlay_ids() -> &'static [ComponentId] {
        OVERLAY_IDS
    }
}

impl Component for UiRootComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for UiRootComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        let observed = match event {
            Event::Keyboard(key) => TerminalObserverEvent::Key(*key),
            Event::WindowResize(width, height) => TerminalObserverEvent::Resize {
                width: *width,
                height: *height,
            },
            Event::FocusGained => TerminalObserverEvent::FocusGained,
            Event::FocusLost => TerminalObserverEvent::FocusLost,
            // Mouse events are otherwise delivered to components through
            // `mouse_sub()` subscriptions (ADR 0024); the observer only needs
            // them as a redraw signal, same as the other non-chord events. A
            // left-click press is the one exception: it is also the only
            // signal shell-painted chrome with no mounted component of its
            // own (the tab bar, task 6.5) ever sees, so it is carried through
            // as `MouseClick` for the shell to resolve against painted
            // geometry.
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                TerminalObserverEvent::MouseClick {
                    column: mouse.column,
                    row: mouse.row,
                }
            }
            Event::Mouse(_) | Event::None | Event::Paste(_) | Event::Tick | Event::User(_) => {
                TerminalObserverEvent::NoOp
            }
        };
        Some(Msg::TerminalEvent(observed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::FeedsComponent;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn root_observer_marks_none_returning_local_key_as_processed() {
        let event = Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        });
        let mut feeds = FeedsComponent::new();
        assert!(feeds.on(&event).is_none());

        let mut root = UiRootComponent::new();
        assert!(matches!(
            root.on(&event),
            Some(Msg::TerminalEvent(TerminalObserverEvent::Key(_)))
        ));
    }
}
