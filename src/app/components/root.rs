use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::{ComponentId, LegacyInput, Msg, UserEvent};

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
pub(in crate::app) struct UiRootComponent {
    legacy_input: LegacyInput,
}

impl UiRootComponent {
    pub(in crate::app) fn new() -> Self {
        Self {
            legacy_input: LegacyInput,
        }
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
        self.legacy_input.on(event)
    }
}
