use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use mbv_render::render_save_playlist_content;
use mbv_ui_model::feed::SavePlaylistStage;
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{LeafKeyResult, Msg, SavePlaylistIntent, ShellRequest};

fn intent_msg(intent: SavePlaylistIntent) -> Msg {
    Msg::Shell(Box::new(ShellRequest::SavePlaylistIntent(intent)))
}

#[derive(Debug)]
pub struct SavePlaylistComponent {
    input: String,
    rename: bool,
    rename_id: Option<String>,
    dim_backdrop_active: bool,
    /// The painted modal rect (last frame) — the outside-click boundary.
    frame: Rect,
    /// Painted button pills (last frame): save, then cancel.
    hit_buttons: HitRegions<SavePlaylistIntent>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl SavePlaylistComponent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: String::new(),
            rename: false,
            rename_id: None,
            dim_backdrop_active: false,
            frame: Rect::default(),
            hit_buttons: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    pub fn set_dialog(&mut self, input: String, stage: SavePlaylistStage) {
        self.input = input;
        self.rename_id = match stage {
            SavePlaylistStage::EnterName => None,
            SavePlaylistStage::RenamePlaylist { id } => Some(id),
        };
        self.rename = self.rename_id.is_some();
    }

    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    #[must_use]
    pub fn is_rename(&self) -> bool {
        self.rename
    }

    #[must_use]
    pub fn rename_id(&self) -> Option<&str> {
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
            Key::Esc => Some(intent_msg(SavePlaylistIntent::Dismiss)),
            Key::Enter => Some(intent_msg(SavePlaylistIntent::Submit)),
            _ => None,
        }
    }

    /// Mouse handling: a click on a button pill presses that pill's key
    /// (save/cancel), like the confirm modal; an outside click mirrors Esc
    /// (`SavePlaylistIntent::Dismiss`). Typing, submit, and
    /// right-click/wheel stay keyboard-only.
    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        let gesture = self.mouse_gestures.recognize(mouse)?;
        let MouseGesture::Click { at, .. } = gesture else {
            return None;
        };
        if let Some(&intent) = self.hit_buttons.resolve(at) {
            return Some(intent_msg(intent));
        }
        if self.frame.contains(at) {
            return None;
        }
        Some(intent_msg(SavePlaylistIntent::Dismiss))
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
        // Adopt the painted frame for outside-click dismissal and the pill
        // rects for button clicks (task 5.1).
        self.frame = geometry.frame;
        self.hit_buttons.clear();
        let intents = [SavePlaylistIntent::Submit, SavePlaylistIntent::Dismiss];
        for (rect, intent) in geometry.buttons.into_iter().zip(intents) {
            self.hit_buttons.push(rect, intent);
        }
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))).into_option(),
                None if matches!(
                    key.code,
                    Key::Backspace | Key::Esc | Key::Enter | Key::Char(_)
                ) =>
                {
                    LeafKeyResult::Consumed(None).into_option()
                }
                None => LeafKeyResult::Unhandled.into_option(),
            },
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tuirealm::event::{KeyModifiers, MouseButton};

    fn left_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn painted_modal(rename: bool) -> (SavePlaylistComponent, ratatui::buffer::Buffer) {
        let mut comp = SavePlaylistComponent::new();
        comp.set_dialog(
            "Road Trip".into(),
            if rename {
                SavePlaylistStage::RenamePlaylist { id: "p1".into() }
            } else {
                SavePlaylistStage::EnterName
            },
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
        terminal
            .draw(|frame| comp.view(frame, frame.area()))
            .expect("draw save-playlist modal");
        let buffer = terminal.backend().buffer().clone();
        (comp, buffer)
    }

    /// A pill click presses that pill's key: save submits, cancel dismisses.
    #[test]
    fn click_on_pill_submits_or_dismisses() {
        let (mut comp, _) = painted_modal(false);
        let save = comp.hit_buttons.regions()[0].0;
        let submit = comp.on(&Event::Mouse(left_down(save.x + 1, save.y)));
        assert!(matches!(
            submit,
            Some(Msg::Shell(ref shell_boxed))
            if matches!(shell_boxed.as_ref(), ShellRequest::SavePlaylistIntent(
                SavePlaylistIntent::Submit
            ))
        ));

        comp.mouse_gestures.reset_for_test();
        let cancel = comp.hit_buttons.regions()[1].0;
        let dismiss = comp.on(&Event::Mouse(left_down(cancel.x + 1, cancel.y)));
        assert!(matches!(
            dismiss,
            Some(Msg::Shell(ref shell_boxed))
            if matches!(shell_boxed.as_ref(), ShellRequest::SavePlaylistIntent(
                SavePlaylistIntent::Dismiss
            ))
        ));
    }
}
