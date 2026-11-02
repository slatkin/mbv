use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use mbv_render::render_twitch_url_content;
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{LeafKeyResult, Msg, ShellRequest, TwitchUrlIntent};

fn intent_msg(intent: TwitchUrlIntent) -> Msg {
    Msg::Shell(Box::new(ShellRequest::TwitchUrlIntent(intent)))
}

/// The Twitch URL prompt (twitch-url-playback D7, task 5.3): a modal text
/// prompt on the model of [`SavePlaylistComponent`](super::save_playlist).
/// It owns the typed input, the shell-pushed refusal text, the painted
/// button pills, and the private mouse gesture. It emits
/// [`TwitchUrlIntent`] only; the host check lives in
/// `TwitchItem::from_url` (the source-of-truth type), never here.
#[derive(Debug)]
pub struct TwitchUrlComponent {
    input: String,
    refusal: Option<String>,
    dim_backdrop_active: bool,
    /// The painted modal rect (last frame) — the outside-click boundary.
    frame: Rect,
    /// Painted button pills (last frame): submit, then cancel.
    hit_buttons: HitRegions<TwitchUrlIntent>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl TwitchUrlComponent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: String::new(),
            refusal: None,
            dim_backdrop_active: false,
            frame: Rect::default(),
            hit_buttons: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Record the shell's refusal for the last submitted URL. The prompt
    /// stays mounted with the input kept (spec: *The prompt accepts only
    /// Twitch hosts*).
    pub fn push_refusal(&mut self, refusal: String) {
        self.refusal = Some(refusal);
    }

    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
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
            Key::Esc => Some(intent_msg(TwitchUrlIntent::Cancel)),
            Key::Enter => Some(intent_msg(TwitchUrlIntent::Submit(self.input.clone()))),
            _ => None,
        }
    }

    /// Mouse handling: a click on a button pill presses that pill's intent
    /// (submit/cancel), like the save-playlist modal; an outside click
    /// mirrors Esc (`TwitchUrlIntent::Cancel`). Typing, submit, and
    /// right-click/wheel stay keyboard-only.
    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        let gesture = self.mouse_gestures.recognize(mouse)?;
        let MouseGesture::Click { at, .. } = gesture else {
            return None;
        };
        if let Some(intent) = self.hit_buttons.resolve(at).cloned() {
            return Some(intent_msg(intent));
        }
        if self.frame.contains(at) {
            return None;
        }
        Some(intent_msg(TwitchUrlIntent::Cancel))
    }
}

impl Default for TwitchUrlComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TwitchUrlComponent {
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        let geometry = render_twitch_url_content(
            frame,
            &mut self.dim_backdrop_active,
            &self.input,
            self.refusal.as_deref(),
        );
        // Adopt the painted frame for outside-click dismissal and the pill
        // rects for button clicks (twitch-url-playback task 5.3).
        self.frame = geometry.frame;
        self.hit_buttons.clear();
        let intents = [
            TwitchUrlIntent::Submit(String::new()),
            TwitchUrlIntent::Cancel,
        ];
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

impl AppComponent<Msg, UserEvent> for TwitchUrlComponent {
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
    use tuirealm::event::KeyModifiers;

    fn type_url(comp: &mut TwitchUrlComponent, url: &str) {
        for c in url.chars() {
            let key = KeyEvent::new(Key::Char(c), KeyModifiers::NONE);
            comp.on(&Event::Keyboard(key));
        }
    }

    /// Enter submits the component-owned input as the intent's payload; the
    /// component performs no host check (twitch-url-playback task 5.3, the
    /// shell owns the check through `TwitchItem::from_url`).
    #[test]
    fn enter_submits_the_typed_url() {
        let mut comp = TwitchUrlComponent::new();
        type_url(&mut comp, "https://www.twitch.tv/somechannel");
        let msg = comp.on(&Event::Keyboard(KeyEvent::new(
            Key::Enter,
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ref shell_boxed))
            if matches!(shell_boxed.as_ref(), ShellRequest::TwitchUrlIntent(
                TwitchUrlIntent::Submit(url)
            ) if url == "https://www.twitch.tv/somechannel")
        ));
    }

    /// Escape cancels: the intent is `Cancel`, whatever the input holds.
    #[test]
    fn escape_cancels() {
        let mut comp = TwitchUrlComponent::new();
        type_url(&mut comp, "https://www.twitch.tv/somechannel");
        let msg = comp.on(&Event::Keyboard(KeyEvent::new(
            Key::Esc,
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ref shell_boxed))
            if matches!(shell_boxed.as_ref(), ShellRequest::TwitchUrlIntent(
                TwitchUrlIntent::Cancel
            ))
        ));
    }
}
