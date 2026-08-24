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

/// Root routing owns overlay z-order; TuiRealm owns focus and its LIFO stack.
pub(in crate::app) struct UiRootComponent {
    overlay_order: Vec<ComponentId>,
    legacy_input: LegacyInput,
}

impl UiRootComponent {
    pub(in crate::app) fn new() -> Self {
        Self {
            overlay_order: Vec::new(),
            legacy_input: LegacyInput,
        }
    }

    pub(in crate::app) fn overlay_ids() -> &'static [ComponentId] {
        OVERLAY_IDS
    }

    pub(in crate::app) fn sync_overlay_order(&mut self, mounted: &[ComponentId]) {
        self.overlay_order.retain(|id| mounted.contains(id));
        for id in mounted {
            if !self.overlay_order.contains(id) {
                self.overlay_order.push(id.clone());
            }
        }
    }

    pub(in crate::app) fn overlay_order(&self) -> &[ComponentId] {
        &self.overlay_order
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{ModalId, OverlayId};

    #[test]
    fn root_ui_keeps_open_overlay_order_without_owning_focus() {
        let settings = ComponentId::Overlay(OverlayId::Settings);
        let help = ComponentId::Overlay(OverlayId::Help);
        let mut root = UiRootComponent::new();

        root.sync_overlay_order(&[settings.clone(), help.clone()]);
        root.sync_overlay_order(&[
            settings.clone(),
            help.clone(),
            ComponentId::Modal(ModalId::Confirm),
        ]);
        root.sync_overlay_order(&[help.clone(), ComponentId::Modal(ModalId::Confirm)]);

        assert_eq!(
            root.overlay_order(),
            &[help, ComponentId::Modal(ModalId::Confirm)]
        );
    }
}
