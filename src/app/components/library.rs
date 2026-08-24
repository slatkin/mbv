use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::{ComponentId, Msg, UserEvent};
use crate::app::{PanelFocus, PanelMode, TabSelection};

pub struct LibraryComponent {
    destination: TabSelection,
    panel_focus: PanelFocus,
    panel_mode: PanelMode,
    active_child: Option<ComponentId>,
}

impl LibraryComponent {
    pub fn new() -> Self {
        Self {
            destination: TabSelection::Home,
            panel_focus: PanelFocus::Library,
            panel_mode: PanelMode::Both,
            active_child: None,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        destination: TabSelection,
        panel_focus: PanelFocus,
        panel_mode: PanelMode,
        active_child: Option<ComponentId>,
    ) {
        self.destination = destination;
        self.panel_focus = panel_focus;
        self.panel_mode = panel_mode;
        self.active_child = active_child;
    }

    pub(in crate::app) fn destination(&self) -> TabSelection {
        self.destination
    }

    pub(in crate::app) fn panel_focus(&self) -> PanelFocus {
        self.panel_focus
    }

    pub(in crate::app) fn panel_mode(&self) -> PanelMode {
        self.panel_mode
    }

    pub(in crate::app) fn active_child(&self) -> Option<&ComponentId> {
        self.active_child.as_ref()
    }
}

impl Default for LibraryComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LibraryComponent {
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

impl AppComponent<Msg, UserEvent> for LibraryComponent {
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{BrowserKey, BrowserKind};
    use mbv_core::config::ServiceKind;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::Component;

    fn child_id() -> ComponentId {
        ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "movies".into(),
            kind: BrowserKind::Movies,
        })
    }

    #[test]
    fn library_parent_mirrors_route_and_panel_state() {
        let child = child_id();
        let mut component = LibraryComponent::new();
        component.set_content(
            TabSelection::EmbyLibrary(2),
            PanelFocus::Queue,
            PanelMode::LibraryOnly,
            Some(child.clone()),
        );

        assert_eq!(component.destination(), TabSelection::EmbyLibrary(2));
        assert_eq!(component.panel_focus(), PanelFocus::Queue);
        assert_eq!(component.panel_mode(), PanelMode::LibraryOnly);
        assert_eq!(component.active_child(), Some(&child));
    }

    #[test]
    fn library_parent_has_no_app_bound_rendering() {
        let mut component = LibraryComponent::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
    }
}
