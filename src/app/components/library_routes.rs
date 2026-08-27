//! Interactive Component for the nested Settings Library-routes popup.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_library_routes_content, LibraryRoutesRenderModel};
use crate::app::types_context_menu::{LibraryRoutePopup, LibraryRouteStage};

pub struct LibraryRoutesComponent {
    stage: Option<LibraryRouteStage>,
    cursor: usize,
    dim_backdrop_active: bool,
}

impl LibraryRoutesComponent {
    pub fn new() -> Self {
        Self {
            stage: None,
            cursor: 0,
            dim_backdrop_active: false,
        }
    }

    /// Mirror the shell snapshot while retaining the local cursor within the
    /// current picker stage.
    pub(in crate::app) fn set_content(&mut self, popup: &LibraryRoutePopup) {
        let same_stage = self
            .stage
            .as_ref()
            .is_some_and(|stage| same_stage_kind(stage, &popup.stage));
        if !same_stage {
            self.stage = Some(popup.stage.clone());
            self.cursor = popup.cursor;
        }
        self.cursor = self
            .cursor
            .min(stage_len(self.stage.as_ref()).saturating_sub(1));
    }

    pub(in crate::app) fn snapshot(&self) -> Option<(LibraryRouteStage, usize)> {
        self.stage.clone().map(|stage| (stage, self.cursor))
    }

    /// Read the current picker stage (task 5.3c): the shell drives stage
    /// transitions once the component owns the interaction state.
    pub(in crate::app) fn stage(&self) -> Option<&LibraryRouteStage> {
        self.stage.as_ref()
    }

    /// Read the current picker cursor (task 5.3c).
    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            Key::Down => {
                let len = stage_len(self.stage.as_ref());
                if len > 0 {
                    self.cursor = (self.cursor + 1).min(len - 1);
                }
                None
            }
            Key::Enter => Some(Msg::Shell(ShellRequest::LibraryRoutesEnter)),
            Key::Esc => Some(Msg::Shell(ShellRequest::LibraryRoutesEsc)),
            _ => None,
        }
    }
}

fn same_stage_kind(left: &LibraryRouteStage, right: &LibraryRouteStage) -> bool {
    matches!(
        (left, right),
        (
            LibraryRouteStage::PickLibrary { .. },
            LibraryRouteStage::PickLibrary { .. }
        ) | (
            LibraryRouteStage::PickDevice { .. },
            LibraryRouteStage::PickDevice { .. }
        )
    )
}

fn stage_len(stage: Option<&LibraryRouteStage>) -> usize {
    match stage {
        Some(LibraryRouteStage::PickLibrary { items }) => items.len(),
        Some(LibraryRouteStage::PickDevice { devices, .. }) => devices.len() + 1,
        None => 0,
    }
}

impl Default for LibraryRoutesComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LibraryRoutesComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        let Some(stage) = self.stage.as_ref() else {
            return;
        };
        render_library_routes_content(
            f,
            &mut self.dim_backdrop_active,
            LibraryRoutesRenderModel {
                stage,
                cursor: self.cursor,
            },
        );
    }

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

impl AppComponent<Msg, UserEvent> for LibraryRoutesComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::event::{KeyEvent, KeyModifiers};

    fn popup() -> LibraryRoutePopup {
        LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary {
                items: vec![
                    ("movies".into(), "Movies".into(), None),
                    ("music".into(), "Music".into(), None),
                ],
            },
            cursor: 0,
        }
    }

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn settings_popup_library_routes_keeps_local_cursor() {
        let mut component = LibraryRoutesComponent::new();
        component.set_content(&popup());
        component.on(&key(Key::Down));
        assert_eq!(component.cursor, 1);
    }

    #[test]
    fn settings_popup_library_routes_cross_boundary_keys_are_typed() {
        let mut component = LibraryRoutesComponent::new();
        component.set_content(&popup());
        assert_eq!(
            component.on(&key(Key::Enter)),
            Some(Msg::Shell(ShellRequest::LibraryRoutesEnter))
        );
        assert_eq!(
            component.on(&key(Key::Esc)),
            Some(Msg::Shell(ShellRequest::LibraryRoutesEsc))
        );
    }

    #[test]
    fn settings_popup_library_routes_renders_without_app_state() {
        let mut component = LibraryRoutesComponent::new();
        component.set_content(&popup());
        let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("Library Routes"));
        assert!(output.contains("Movies"));
    }
}
