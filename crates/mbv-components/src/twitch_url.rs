use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::MouseGestureState;
use super::mouse::hit::HitRegions;
use mbv_render::render_twitch_url_content;
use mbv_surfaces::gesture::PointerGesture;
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{LeafKeyResult, Msg, ShellRequest, TwitchUrlIntent};

fn intent_msg(intent: TwitchUrlIntent) -> Msg {
    Msg::Shell(Box::new(ShellRequest::TwitchUrlIntent(intent)))
}

/// A painted pill's identity. Hit regions resolve clicks to this marker,
/// never to a payload-carrying intent: the submit payload is the
/// component's live input at click time, so a click always submits what the
/// user typed (unit-C review P1: the pill used to store an empty
/// `Submit` payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TwitchUrlPill {
    Submit,
    Cancel,
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
    hit_buttons: HitRegions<TwitchUrlPill>,
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

    /// The shell-pushed refusal for the last submitted URL, if any.
    #[must_use]
    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
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
        let PointerGesture::Click { at, .. } = gesture else {
            return None;
        };
        match self.hit_buttons.resolve(at) {
            Some(TwitchUrlPill::Submit) => {
                return Some(intent_msg(TwitchUrlIntent::Submit(self.input.clone())));
            }
            Some(TwitchUrlPill::Cancel) => return Some(intent_msg(TwitchUrlIntent::Cancel)),
            None => {}
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
        let pills = [TwitchUrlPill::Submit, TwitchUrlPill::Cancel];
        for (rect, pill) in geometry.buttons.into_iter().zip(pills) {
            self.hit_buttons.push(rect, pill);
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tuirealm::event::{KeyModifiers, MouseButton};

    fn type_url(comp: &mut TwitchUrlComponent, url: &str) {
        for c in url.chars() {
            let key = KeyEvent::new(Key::Char(c), KeyModifiers::NONE);
            comp.on(&Event::Keyboard(key));
        }
    }

    fn left_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
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

    /// Regression for the unit-C review P1: the submit pill used to store an
    /// empty `Submit` payload at paint time, so a click always submitted an
    /// empty URL. The click now resolves to the pill marker and emits the
    /// component's live input at click time.
    #[test]
    fn click_submit_pill_submits_the_typed_input() {
        let mut comp = TwitchUrlComponent::new();
        type_url(&mut comp, "https://www.twitch.tv/somechannel");
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
        terminal
            .draw(|frame| comp.view(frame, frame.area()))
            .expect("draw twitch-url prompt");
        let submit = comp.hit_buttons.regions()[0].0;
        let msg = comp.on(&Event::Mouse(left_down(submit.x + 1, submit.y)));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ref shell_boxed))
            if matches!(shell_boxed.as_ref(), ShellRequest::TwitchUrlIntent(
                TwitchUrlIntent::Submit(url)
            ) if url == "https://www.twitch.tv/somechannel")
        ));
    }
}
