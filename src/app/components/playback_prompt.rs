//! Interactive Component for the status-bar playback prompts.
//!
//! The shell mirrors the prompt text and visibility policy. Player effects and
//! prompt lifecycle remain in the shell-owned App handlers.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::typed_key::to_crossterm_key_event;
use super::user_event::UserEvent;

pub struct PlaybackPromptComponent {
    status: String,
    visible: bool,
    area: Rect,
}

impl PlaybackPromptComponent {
    pub fn new() -> Self {
        Self {
            status: String::new(),
            visible: false,
            area: Rect::default(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, status: &str, visible: bool, area: Rect) {
        self.status.clear();
        self.status.push_str(status);
        self.visible = visible;
        self.area = area;
    }

    pub(in crate::app) fn status(&self) -> &str {
        &self.status
    }
}

impl Default for PlaybackPromptComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PlaybackPromptComponent {
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        crate::app::render::render_playback_prompt_content(
            frame,
            self.area,
            &self.status,
            self.visible,
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

impl AppComponent<Msg, UserEvent> for PlaybackPromptComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => Some(Msg::Shell(ShellRequest::PlaybackPromptKey(
                to_crossterm_key_event(key),
            ))),
            // Mouse and other events do not carry prompt actions. UiRoot's
            // permanent observer supplies the redraw signal.
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuirealm::event::{Key, KeyModifiers};

    #[test]
    fn key_forwards_to_shell() {
        let mut comp = PlaybackPromptComponent::new();
        let msg = comp.on(&Event::Keyboard(tuirealm::event::KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::PlaybackPromptKey(key)))
                if key.code == crossterm::event::KeyCode::Enter
        ));
    }

    #[test]
    fn non_action_events_return_none() {
        let mut comp = PlaybackPromptComponent::new();
        assert_eq!(comp.on(&Event::<UserEvent>::None), None);
        assert_eq!(comp.on(&Event::<UserEvent>::Tick), None);
    }
}
