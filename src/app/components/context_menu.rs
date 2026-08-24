//! Interactive Component for the Context menu overlay (design D3–D9).
//!
//! Owns the menu's display state (entries, cursor) set by the shell via
//! downcast before each render. The shell owns context-menu actions
//! (`execute_context_action`); the component forwards keys as
//! `Msg::Shell(ShellRequest::ContextMenuKey(key))` so the shell can run the
//! existing `handle_key_context_menu` unchanged. Mouse events are forwarded
//! to the legacy `App::handle_mouse` path (which reads
//! `layout.context_menu_rect` for click-inside/outside behavior).
//!
//! Placement is computed by `App::render_context_menu` (which needs
//! `AppLayout` geometry) and passed to the component via downcast as
//! `menu_rect`. The component's `view()` paints at that rect.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_context_menu_content;

/// The Interactive Component for the Context menu.
///
/// Owns `entries` (as `(label, is_selectable)` pairs), `cursor`, and
/// `menu_rect` (set by the shell via downcast before each render).
pub struct ContextMenuComponent {
    entries: Vec<(&'static str, bool)>,
    cursor: usize,
    menu_rect: Rect,
}

impl ContextMenuComponent {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            menu_rect: Rect::default(),
        }
    }

    /// Set the menu's display content from `App::context_menu`. Called by the
    /// shell via `get_component_mut`+downcast before each render.
    pub(in crate::app) fn set_content(
        &mut self,
        entries: &[crate::app::ContextMenuEntry],
        cursor: usize,
        menu_rect: Rect,
    ) {
        self.entries.clear();
        self.entries
            .extend(entries.iter().map(|e| (e.label, e.action.is_some())));
        self.cursor = cursor;
        self.menu_rect = menu_rect;
    }
}

impl Default for ContextMenuComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ContextMenuComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        if self.entries.is_empty() {
            return;
        }
        render_context_menu_content(f, self.menu_rect, &self.entries, self.cursor);
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

impl AppComponent<Msg, UserEvent> for ContextMenuComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            // Forward every key to the shell's existing context-menu handler.
            Event::Keyboard(key) => {
                let crossterm_key = to_crossterm_key_event(key);
                Some(Msg::Shell(ShellRequest::ContextMenuKey(crossterm_key)))
            }
            // Mouse events: forward to the legacy `App::handle_mouse` path
            // (which reads `layout.context_menu_rect` for click-inside/
            // outside behavior).
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
    fn key_forwards_context_menu_key_to_shell() {
        let mut comp = ContextMenuComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(Key::Down, KeyModifiers::NONE)));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::ContextMenuKey(key)))
                if key.code == crossterm::event::KeyCode::Down
        ));
    }

    #[test]
    fn unbound_key_forwards_to_shell() {
        let mut comp = ContextMenuComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(
            Key::Char('x'),
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::ContextMenuKey(_)))
        ));
    }

    #[test]
    fn non_keyboard_event_returns_noop() {
        let mut comp = ContextMenuComponent::new();
        let msg = comp.on(&Event::<UserEvent>::None);
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn mouse_event_forwards_to_legacy_handler() {
        let mut comp = ContextMenuComponent::new();
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
}
