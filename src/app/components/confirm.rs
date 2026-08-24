//! Interactive Component for the Confirm modal overlay (design D3–D9).
//!
//! Owns the modal's display content (title, message, hint) set by the shell
//! via downcast before each render. The shell owns the `ConfirmAction` and
//! key-to-action dispatch (`App::handle_key_confirm_modal`); the component
//! forwards every key as `Msg::Shell(ShellRequest::ConfirmKey(key))` so the
//! shell can run the existing handler unchanged. The component owns rendering
//! and the blocking-modal swallow semantics (returns `Some(NoOp)` for
//! non-key/non-mouse events — the redraw signal, design D12).

use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_confirm_modal_content;

/// The Interactive Component for the Confirm modal.
///
/// Owns display content (title, message, hint) set by the shell via
/// `get_component_mut`+downcast before each render. The
/// `dim_backdrop_active` field is a scratch flag for `render_modal_frame`
/// (design D9: the visual backdrop is painted by `dim_backdrop`; the flag
/// is written-to but not read by the modal, and `App::render` resets
/// `App::dim_backdrop_active` from `any_dim_modal_open()` before each frame's
/// image lookups, so no shell↔component sync is needed).
pub struct ConfirmComponent {
    title: String,
    message: String,
    hint: String,
    dim_backdrop_active: bool,
}

impl ConfirmComponent {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            message: String::new(),
            hint: String::new(),
            dim_backdrop_active: false,
        }
    }

    /// Set the modal's display content from `App::confirm_modal`. Called by
    /// the shell via `get_component_mut`+downcast before each render.
    pub(in crate::app) fn set_content(&mut self, title: &str, message: &str, hint: &str) {
        self.title.clear();
        self.title.push_str(title);
        self.message.clear();
        self.message.push_str(message);
        self.hint.clear();
        self.hint.push_str(hint);
    }
}

impl Default for ConfirmComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ConfirmComponent {
    fn view(&mut self, f: &mut Frame, _area: ratatui::layout::Rect) {
        render_confirm_modal_content(
            f,
            &mut self.dim_backdrop_active,
            &self.title,
            &self.message,
            &self.hint,
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

impl AppComponent<Msg, UserEvent> for ConfirmComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            // Forward every key to the shell's existing confirm-modal handler.
            // The shell owns `ConfirmAction` dispatch (which key means yes/
            // cancel/save/discard depends on the pending action, which is
            // shell-owned state the component cannot see).
            Event::Keyboard(key) => {
                let crossterm_key = to_crossterm_key_event(key);
                Some(Msg::Shell(ShellRequest::ConfirmKey(crossterm_key)))
            }
            // Mouse events: forward to the legacy `App::handle_mouse` path
            // (design D12/D13). The legacy confirm modal does not block
            // mouse — clicks on tabs, panels, and playback controls still
            // work while a confirm modal is open. The component is the active
            // component, so without forwarding the mouse would be swallowed.
            Event::Mouse(mouse) => {
                let crossterm_mouse = to_crossterm_mouse_event(mouse);
                Some(Msg::Legacy(LegacyTerminalEvent::Mouse(crossterm_mouse)))
            }
            // Non-key/non-mouse events: no-op redraw signal (design D12).
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
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
    fn key_forwards_confirm_key_to_shell() {
        let mut comp = ConfirmComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(
            Key::Char('y'),
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::ConfirmKey(key)))
                if key.code == crossterm::event::KeyCode::Char('y')
        ));
    }

    #[test]
    fn esc_forwards_confirm_key_to_shell() {
        let mut comp = ConfirmComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(Key::Esc, KeyModifiers::NONE)));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::ConfirmKey(key)))
                if key.code == crossterm::event::KeyCode::Esc
        ));
    }

    #[test]
    fn unbound_key_forwards_to_shell() {
        let mut comp = ConfirmComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(
            Key::Char('x'),
            KeyModifiers::NONE,
        )));
        assert!(matches!(msg, Some(Msg::Shell(ShellRequest::ConfirmKey(_)))));
    }

    #[test]
    fn non_keyboard_event_returns_noop() {
        let mut comp = ConfirmComponent::new();
        let msg = comp.on(&Event::<UserEvent>::None);
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn tick_event_returns_noop() {
        let mut comp = ConfirmComponent::new();
        let msg = comp.on(&Event::<UserEvent>::Tick);
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn mouse_event_forwards_to_legacy_handler() {
        let mut comp = ConfirmComponent::new();
        let msg = comp.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            msg,
            Some(Msg::Legacy(LegacyTerminalEvent::Mouse(_)))
        ));
    }

    #[test]
    fn set_content_updates_title_message_hint() {
        let mut comp = ConfirmComponent::new();
        comp.set_content(" Title ", "Are you sure?", "[y] Yes    [Esc] Cancel");
        assert_eq!(comp.title, " Title ");
        assert_eq!(comp.message, "Are you sure?");
        assert_eq!(comp.hint, "[y] Yes    [Esc] Cancel");
    }
}
