//! Interactive Component for the Confirm modal overlay (design D3–D9).
//!
//! Owns the modal's display content (message, buttons) set by the shell via
//! downcast before each render. The component owns key interpretation and
//! emits semantic confirmation intents; the shell owns the `ConfirmAction` and
//! effect dispatch. Non-key events return `None` because the permanent `UiRoot`
//! observer owns the redraw signal (design D12).

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use mbv_render::render_confirm_modal_content;
use mbv_ui_model::confirm::{ConfirmAction, ConfirmButton, ConfirmModal};
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{ConfirmIntent, LeafKeyResult, Msg, ShellRequest};

/// The Interactive Component for the Confirm modal.
///
/// Owns display content (message, buttons) set by the shell via
/// `get_component_mut`+downcast before each render. The
/// `dim_backdrop_active` field is a scratch flag for `render_modal_frame`
/// (design D9: the visual backdrop is painted by `dim_backdrop`; the flag
/// is written-to but not read by the modal, and `App::render` resets
/// `App::dim_backdrop_active` from `any_dim_modal_open()` before each frame's
/// image lookups, so no shell↔component sync is needed).
#[derive(Debug)]
pub struct ConfirmComponent {
    message: String,
    buttons: Vec<ConfirmButton>,
    on_confirm: Option<ConfirmAction>,
    dim_backdrop_active: bool,
    /// The painted modal rect (last frame) — the outside-click boundary.
    frame: Rect,
    /// Painted button pills (last frame) -> index into `buttons`.
    hit_buttons: HitRegions<usize>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl ConfirmComponent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            message: String::new(),
            buttons: Vec::new(),
            on_confirm: None,
            dim_backdrop_active: false,
            frame: Rect::default(),
            hit_buttons: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Set the modal's display content from a shell request. Called by
    /// the shell via `get_component_mut`+downcast before each render.
    pub fn set_content(&mut self, message: &str, buttons: &[ConfirmButton]) {
        message.clone_into(&mut self.message);
        self.buttons = buttons.to_vec();
    }

    pub fn set_modal(&mut self, modal: &ConfirmModal) {
        self.set_content(&modal.message, &modal.buttons);
        self.on_confirm = Some(modal.on_confirm.clone());
    }

    #[must_use]
    pub fn confirm_action(&self) -> Option<ConfirmAction> {
        self.on_confirm.clone()
    }
}

impl Default for ConfirmComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ConfirmComponent {
    fn view(&mut self, frame: &mut Frame, _area: ratatui::layout::Rect) {
        let geometry = render_confirm_modal_content(
            frame,
            &mut self.dim_backdrop_active,
            &self.message,
            &self.buttons,
        );
        // Adopt the rects the painter just produced (task 5.1, design.md D6).
        self.frame = geometry.frame;
        self.hit_buttons.clear();
        for (index, rect) in geometry.buttons.into_iter().enumerate() {
            self.hit_buttons.push(rect, index);
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

impl ConfirmComponent {
    fn key_result(&self, key: &tuirealm::event::KeyEvent) -> LeafKeyResult {
        let Some(action) = self.on_confirm.as_ref() else {
            return LeafKeyResult::Unhandled;
        };
        let Some(intent) = confirm_intent_for_key(action, key.code) else {
            return LeafKeyResult::Unhandled;
        };
        LeafKeyResult::Consumed(Some(Box::new(confirm_msg(intent))))
    }

    /// Mouse handling (issue #855): a click on a button pill presses that
    /// button's first key through the same `confirm_intent_for_key` mapping the
    /// keyboard uses; a click outside the modal rect cancels (Esc), since the
    /// dialog is blocking. A click inside but off the pills does nothing.
    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        self.on_confirm.as_ref()?;
        let gesture = self.mouse_gestures.recognize(mouse)?;
        let MouseGesture::Click { at, .. } = gesture else {
            return None;
        };
        if let Some(&index) = self.hit_buttons.resolve(at) {
            return self.button_intent(index);
        }
        if self.frame.contains(at) {
            return None;
        }
        Some(confirm_msg(ConfirmIntent::Cancel))
    }

    /// The intent a click on `buttons[index]` emits: press the pill's first
    /// key.
    fn button_intent(&self, index: usize) -> Option<Msg> {
        let action = self.on_confirm.as_ref()?;
        let key = button_keys(&self.buttons.get(index)?.keys).next()?;
        let intent = confirm_intent_for_key(action, key)?;
        Some(confirm_msg(intent))
    }
}

fn confirm_msg(intent: ConfirmIntent) -> Msg {
    Msg::Shell(Box::new(ShellRequest::ConfirmIntent(intent)))
}

/// The keys a pill's `keys` hint names, e.g. `"y/Enter"` -> `y`, `Enter`.
fn button_keys(hint: &str) -> impl Iterator<Item = Key> + '_ {
    hint.split('/').filter_map(key_from_hint)
}

fn key_from_hint(token: &str) -> Option<Key> {
    match token {
        "Enter" => Some(Key::Enter),
        "Esc" => Some(Key::Esc),
        other => {
            let mut chars = other.chars();
            let ch = chars.next()?;
            chars.next().is_none().then_some(Key::Char(ch))
        }
    }
}

impl AppComponent<Msg, UserEvent> for ConfirmComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.key_result(key).into_option(),
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
            _ => None,
        }
    }
}

/// The intent a key produces for `action`. Only Enter and Esc answer a
/// confirmation; every other key (including the old `y`/`n`/`s`/`c`) is a
/// no-op, so the modal stays open. The three-way dirty-playlist prompt adds
/// `d` for its distinct Discard answer.
fn confirm_intent_for_key(action: &ConfirmAction, key: Key) -> Option<ConfirmIntent> {
    match key {
        Key::Enter => Some(match action {
            ConfirmAction::DiscardOrSaveDirtyPlaylist => ConfirmIntent::Save,
            _ => ConfirmIntent::Accept,
        }),
        Key::Esc => Some(ConfirmIntent::Cancel),
        Key::Char('d' | 'D') if matches!(action, ConfirmAction::DiscardOrSaveDirtyPlaylist) => {
            Some(ConfirmIntent::Discard)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use rstest::rstest;
    use tuirealm::event::{Key, KeyModifiers, MouseButton};

    fn make_key(code: Key, modifiers: KeyModifiers) -> tuirealm::event::KeyEvent {
        tuirealm::event::KeyEvent { code, modifiers }
    }

    fn left_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn painted_modal(
        buttons: Vec<ConfirmButton>,
        on_confirm: ConfirmAction,
    ) -> (ConfirmComponent, ratatui::buffer::Buffer) {
        let mut comp = ConfirmComponent::new();
        comp.set_modal(&ConfirmModal {
            message: "Clear the queue?".into(),
            buttons,
            on_confirm,
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
        terminal
            .draw(|frame| comp.view(frame, frame.area()))
            .expect("draw confirm modal");
        let buffer = terminal.backend().buffer().clone();
        (comp, buffer)
    }

    #[test]
    fn key_emits_accept_intent() {
        let mut comp = ConfirmComponent::new();
        comp.set_modal(&ConfirmModal {
            message: String::new(),
            buttons: Vec::new(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        let msg = comp.on(&Event::Keyboard(make_key(Key::Enter, KeyModifiers::NONE)));
        assert!(matches!(
           msg,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Accept
           ))));
    }

    #[rstest]
    #[case::clear_queue_unknown_key_is_a_noop(Key::Char('x'), ConfirmAction::ClearQueue, None)]
    #[case::clear_queue_enter_accepts(
        Key::Enter,
        ConfirmAction::ClearQueue,
        Some(ConfirmIntent::Accept)
    )]
    #[case::dirty_playlist_enter_saves(
        Key::Enter,
        ConfirmAction::DiscardOrSaveDirtyPlaylist,
        Some(ConfirmIntent::Save)
    )]
    #[case::dirty_playlist_discard(
        Key::Char('d'),
        ConfirmAction::DiscardOrSaveDirtyPlaylist,
        Some(ConfirmIntent::Discard)
    )]
    fn action_specific_key_intents(
        #[case] key: Key,
        #[case] action: ConfirmAction,
        #[case] expected: Option<ConfirmIntent>,
    ) {
        assert_eq!(confirm_intent_for_key(&action, key), expected);
    }

    #[test]
    fn esc_emits_cancel_intent() {
        let mut comp = ConfirmComponent::new();
        comp.set_modal(&ConfirmModal {
            message: String::new(),
            buttons: Vec::new(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        let msg = comp.on(&Event::Keyboard(make_key(Key::Esc, KeyModifiers::NONE)));
        assert!(matches!(
           msg,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Cancel
           ))));
    }

    /// Issue #855: a pill click presses that pill's first key.
    #[test]
    fn click_on_pill_emits_that_pills_first_key_intent() {
        let (mut comp, _) = painted_modal(
            vec![
                ConfirmButton::affirmative("Enter", "Confirm"),
                ConfirmButton::cancel("Esc", "Cancel"),
            ],
            ConfirmAction::ClearQueue,
        );
        let ok = comp.hit_buttons.regions()[0].0;
        let accept = comp.on(&Event::Mouse(left_down(ok.x + 1, ok.y)));
        assert!(matches!(
           accept,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Accept
           ))));

        comp.mouse_gestures.reset_for_test();
        let cancel = comp.hit_buttons.regions()[1].0;
        let msg = comp.on(&Event::Mouse(left_down(cancel.x + 1, cancel.y)));
        assert!(matches!(
           msg,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Cancel
           ))));
    }

    /// Issue #855: clicking the dim backdrop cancels; clicking the modal body
    /// off the pills does nothing.
    #[test]
    fn click_outside_cancels_and_inside_off_pill_does_nothing() {
        let (mut comp, _) = painted_modal(
            vec![ConfirmButton::affirmative("Enter", "Confirm")],
            ConfirmAction::ClearQueue,
        );
        let frame = comp.frame;
        let inside = comp.on(&Event::Mouse(left_down(frame.x + 2, frame.y + 2)));
        assert!(inside.is_none());

        comp.mouse_gestures.reset_for_test();
        let outside = comp.on(&Event::Mouse(left_down(frame.x.saturating_sub(1), frame.y)));
        assert!(matches!(
           outside,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Cancel
           ))));
    }
}
