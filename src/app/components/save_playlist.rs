use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{Msg, SavePlaylistIntent, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_save_playlist_content;
use crate::app::SavePlaylistStage;

pub struct SavePlaylistComponent {
    input: String,
    rename: bool,
    rename_id: Option<String>,
    dim_backdrop_active: bool,
    /// The painted modal rect (last frame) — the outside-click boundary.
    frame: Rect,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl SavePlaylistComponent {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            rename: false,
            rename_id: None,
            dim_backdrop_active: false,
            frame: Rect::default(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, input: String, rename: bool) {
        self.input = input;
        self.rename = rename;
        self.rename_id = None;
    }

    pub(in crate::app) fn set_dialog(&mut self, input: String, stage: SavePlaylistStage) {
        self.input = input;
        self.rename_id = match stage {
            SavePlaylistStage::EnterName => None,
            SavePlaylistStage::RenamePlaylist { id } => Some(id),
        };
        self.rename = self.rename_id.is_some();
    }

    pub(in crate::app) fn input(&self) -> &str {
        &self.input
    }

    pub(in crate::app) fn is_rename(&self) -> bool {
        self.rename
    }

    pub(in crate::app) fn rename_id(&self) -> Option<&str> {
        self.rename_id.as_deref()
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Backspace => {
                self.input.pop();
                None
            }
            Key::Char(c)
                if key.modifiers == tuirealm::event::KeyModifiers::NONE
                    || key.modifiers == tuirealm::event::KeyModifiers::SHIFT =>
            {
                self.input.push(c);
                None
            }
            Key::Esc => Some(Msg::Shell(ShellRequest::SavePlaylistIntent(
                SavePlaylistIntent::Dismiss,
            ))),
            Key::Enter => Some(Msg::Shell(ShellRequest::SavePlaylistIntent(
                SavePlaylistIntent::Submit,
            ))),
            _ => None,
        }
    }

    /// Mouse handling (task 5.1): the modal is a single always-focused
    /// name input with no painted buttons and no focus/select keyboard
    /// path, so the only click with a keyboard equivalent is an outside
    /// click mirroring Esc (`SavePlaylistIntent::Dismiss`). Typing,
    /// submit, and right-click/wheel stay keyboard-only.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click(at) if !self.frame.contains(at) => Some(Msg::Shell(
                ShellRequest::SavePlaylistIntent(SavePlaylistIntent::Dismiss),
            )),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_frame(&self) -> Rect {
        self.frame
    }

    /// Test seam: forget the last click so the next event is neither
    /// throttled nor promoted to a double-click.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }
}

impl Default for SavePlaylistComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SavePlaylistComponent {
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        let geometry = render_save_playlist_content(
            frame,
            &mut self.dim_backdrop_active,
            &self.input,
            self.rename,
        );
        // Adopt the painted frame for outside-click dismissal (task 5.1).
        self.frame = geometry.frame;
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
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
