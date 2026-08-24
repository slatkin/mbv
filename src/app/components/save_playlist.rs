use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::to_crossterm_key_event;
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_save_playlist_content;

pub struct SavePlaylistComponent {
    input: String,
    rename: bool,
    dim_backdrop_active: bool,
}

impl SavePlaylistComponent {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            rename: false,
            dim_backdrop_active: false,
        }
    }

    pub(in crate::app) fn set_content(&mut self, input: String, rename: bool) {
        self.input = input;
        self.rename = rename;
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Backspace => {
                self.input.pop();
            }
            Key::Char(c)
                if key.modifiers == tuirealm::event::KeyModifiers::NONE
                    || key.modifiers == tuirealm::event::KeyModifiers::SHIFT =>
            {
                self.input.push(c);
            }
            _ => {}
        }
        Some(Msg::Shell(ShellRequest::SavePlaylistKey(
            to_crossterm_key_event(key),
        )))
    }
}

impl Default for SavePlaylistComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SavePlaylistComponent {
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        render_save_playlist_content(
            frame,
            &mut self.dim_backdrop_active,
            &self.input,
            self.rename,
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

impl AppComponent<Msg, UserEvent> for SavePlaylistComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            _ => None,
        }
    }
}
