//! Interactive Component for the Remote-reanchor popup (design D3–D9).
//!
//! Owns the popup's display content (targets, cursor) set by the shell via
//! downcast before each render. The component owns key interpretation and
//! emits semantic movement/accept/dismiss intents; the shell owns
//! reconciliation. Mouse and other events are swallowed by the blocking popup;
//! UiRoot's permanent observer supplies the redraw signal (design D12).

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, RemoteReanchorIntent, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_remote_reanchor_popup_content;

/// The Interactive Component for the Remote-reanchor popup.
pub struct RemoteReanchorComponent {
    targets: Vec<(usize, String)>,
    cursor: usize,
    dim_backdrop_active: bool,
}

impl RemoteReanchorComponent {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            cursor: 0,
            dim_backdrop_active: false,
        }
    }

    /// Set the popup's display content from a shell request.
    /// Called by the shell via `get_component_mut`+downcast before each render.
    pub(in crate::app) fn set_content(&mut self, targets: &[(usize, String)], cursor: usize) {
        self.targets.clear();
        self.targets.extend(targets.iter().cloned());
        self.cursor = cursor;
    }

    pub(in crate::app) fn move_cursor(&mut self, down: bool) {
        if down {
            self.cursor = self
                .cursor
                .saturating_add(1)
                .min(self.targets.len().saturating_sub(1));
        } else {
            self.cursor = self.cursor.saturating_sub(1);
        }
    }

    pub(in crate::app) fn selected_target(&self) -> Option<usize> {
        self.targets.get(self.cursor).map(|(target, _)| *target)
    }
}

impl Default for RemoteReanchorComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for RemoteReanchorComponent {
    fn view(&mut self, f: &mut ratatui::Frame, _area: ratatui::layout::Rect) {
        render_remote_reanchor_popup_content(
            f,
            &mut self.dim_backdrop_active,
            &self.targets,
            self.cursor,
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

impl AppComponent<Msg, UserEvent> for RemoteReanchorComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let Event::Keyboard(key) = ev else {
            return None;
        };
        let intent = match key.code {
            Key::Up => RemoteReanchorIntent::MoveUp,
            Key::Down => RemoteReanchorIntent::MoveDown,
            Key::Enter => RemoteReanchorIntent::Accept,
            Key::Esc => RemoteReanchorIntent::Dismiss,
            _ => return None,
        };
        Some(Msg::Shell(ShellRequest::RemoteReanchorIntent(intent)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuirealm::event::{Key, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn make_key(code: Key, modifiers: KeyModifiers) -> tuirealm::event::KeyEvent {
        tuirealm::event::KeyEvent { code, modifiers }
    }

    #[test]
    fn navigation_key_emits_move_intent() {
        let mut comp = RemoteReanchorComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(Key::Up, KeyModifiers::NONE)));
        assert_eq!(
            msg,
            Some(Msg::Shell(ShellRequest::RemoteReanchorIntent(
                RemoteReanchorIntent::MoveUp
            )))
        );
    }

    #[test]
    fn unbound_key_is_swallowed_locally() {
        let mut comp = RemoteReanchorComponent::new();
        assert_eq!(
            comp.on(&Event::Keyboard(make_key(
                Key::Char('x'),
                KeyModifiers::NONE,
            ))),
            None
        );
    }

    #[test]
    fn non_keyboard_events_return_none() {
        let mut comp = RemoteReanchorComponent::new();
        assert_eq!(comp.on(&Event::<UserEvent>::None), None);
        assert_eq!(
            comp.on(&Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 10,
                row: 5,
                modifiers: KeyModifiers::NONE,
            })),
            None
        );
    }

    #[test]
    fn set_content_updates_targets_and_cursor() {
        let mut comp = RemoteReanchorComponent::new();
        comp.set_content(&[(0, "a".into()), (1, "b".into())], 1);
        assert_eq!(
            comp.targets,
            vec![(0, "a".to_string()), (1, "b".to_string())]
        );
        assert_eq!(comp.cursor, 1);
    }
}
