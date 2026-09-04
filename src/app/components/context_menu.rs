//! Interactive Component for the Context menu overlay (design D3–D9).
//!
//! Owns the menu's display state (`entries`, `cursor`, `anchor`) and the
//! painted `menu_rect`, set by the shell via downcast before each render.
//! The shell owns context-menu actions (`execute_context_action`); the
//! component interprets keyboard input as semantic intents and handles its own
//! mouse hit-test (click-inside/outside and hover),
//! emitting `ContextMenuSelect`/`ContextMenuDismiss` (task 5.3c — the
//! component owns its rect and hit test, replacing the old
//! `layout.context_menu_rect` global).

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
#[cfg(test)]
use tuirealm::event::KeyEvent as TuiKeyEvent;
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_context_menu_content;
use crate::app::types_context_menu::{ContextAction, ContextMenuAnchor, ContextMenuEntry};
use crate::app::PanelFocus;

/// The Interactive Component for the Context menu.
///
/// Owns `entries` (with their `ContextAction`s), `cursor`, `anchor`, and the
/// painted `menu_rect` (set by the shell via downcast before each render).
pub struct ContextMenuComponent {
    entries: Vec<ContextMenuEntry>,
    cursor: usize,
    anchor: ContextMenuAnchor,
    menu_rect: Rect,
}

impl ContextMenuComponent {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            anchor: ContextMenuAnchor::SelectedItem(PanelFocus::Library),
            menu_rect: Rect::default(),
        }
    }

    /// Set the menu's display content from the shell's `OverlayRequest`.
    /// Called by the shell via `get_component_mut`+downcast on mount.
    pub(in crate::app) fn set_content(
        &mut self,
        anchor: ContextMenuAnchor,
        entries: Vec<ContextMenuEntry>,
        cursor: usize,
    ) {
        self.anchor = anchor;
        self.entries = entries;
        self.cursor = cursor;
    }

    /// Set the painted rect, computed by the shell from `AppLayout` each frame.
    pub(in crate::app) fn set_rect(&mut self, rect: Rect) {
        self.menu_rect = rect;
    }

    /// The painted rect the shell computed from `AppLayout` (task 5.3c).
    pub(in crate::app) fn menu_rect(&self) -> Rect {
        self.menu_rect
    }

    /// The menu's anchor, used by the shell to recompute `menu_rect`.
    pub(in crate::app) fn anchor(&self) -> ContextMenuAnchor {
        self.anchor
    }

    /// Entries, used by the shell to recompute `menu_rect` size.
    pub(in crate::app) fn entries(&self) -> &[ContextMenuEntry] {
        &self.entries
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selectable action at `idx`, if any (the shell executes it).
    pub(in crate::app) fn action_at(&self, idx: usize) -> Option<ContextAction> {
        self.entries.get(idx).and_then(|entry| entry.action.clone())
    }

    /// Move the cursor to the next/previous selectable entry (wrapping).
    pub(in crate::app) fn move_cursor(&mut self, down: bool) {
        let selectable: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| entry.action.is_some().then_some(i))
            .collect();
        if selectable.is_empty() {
            return;
        }
        let pos = selectable.iter().position(|&i| i == self.cursor);
        self.cursor = match (down, pos) {
            (true, Some(p)) => selectable[(p + 1) % selectable.len()],
            (true, None) => selectable[0],
            (false, Some(p)) => selectable[p.checked_sub(1).unwrap_or(selectable.len() - 1)],
            (false, None) => selectable[selectable.len() - 1],
        };
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if self.entries.is_empty() || self.menu_rect == Rect::default() {
            return None;
        }
        use ratatui::layout::Position;
        let pos = Position::new(mouse.column, mouse.row);
        let inside = self.menu_rect.contains(pos);
        let inner_y = self.menu_rect.y + 1;
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if inside && pos.y >= inner_y {
                    let idx = (pos.y - inner_y) as usize;
                    if let Some(entry) = self.entries.get(idx) {
                        if entry.action.is_some() {
                            return Some(Msg::Shell(ShellRequest::ContextMenuSelect(idx)));
                        }
                    }
                }
                Some(Msg::Shell(ShellRequest::ContextMenuDismiss))
            }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Right) => {
                if inside
                    && pos.y >= inner_y
                    && self.menu_rect.y + 1 + self.entries.len() as u16 > pos.y
                {
                    let idx = (pos.y - inner_y) as usize;
                    if let Some(entry) = self.entries.get(idx) {
                        if entry.action.is_some() && idx != self.cursor {
                            self.cursor = idx;
                            // Force a redraw so the highlight follows the pointer.
                            return None;
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl Default for ContextMenuComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ContextMenuComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        if self.entries.is_empty() || self.menu_rect == Rect::default() {
            return;
        }
        let entries: Vec<(&'static str, bool)> = self
            .entries
            .iter()
            .map(|entry| (entry.label, entry.action.is_some()))
            .collect();
        render_context_menu_content(f, self.menu_rect, &entries, self.cursor);
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
            Event::Keyboard(key) => {
                let intent = match key.code {
                    tuirealm::event::Key::Up => super::msg::ContextMenuIntent::MoveUp,
                    tuirealm::event::Key::Down => super::msg::ContextMenuIntent::MoveDown,
                    tuirealm::event::Key::Enter => super::msg::ContextMenuIntent::Select,
                    tuirealm::event::Key::Esc => super::msg::ContextMenuIntent::Dismiss,
                    _ => return None,
                };
                Some(Msg::Shell(ShellRequest::ContextMenuIntent(intent)))
            }
            // The component owns its mouse hit-test (task 5.3c).
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            // Non-key/non-mouse events are handled by UiRoot's permanent
            // observer, which supplies the redraw signal (design D12).
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuirealm::event::{Key, KeyModifiers, MouseButton, MouseEventKind};

    fn make_key(code: Key, modifiers: KeyModifiers) -> TuiKeyEvent {
        TuiKeyEvent { code, modifiers }
    }

    #[test]
    fn navigation_key_emits_move_intent() {
        let mut comp = ContextMenuComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(Key::Down, KeyModifiers::NONE)));
        assert_eq!(
            msg,
            Some(Msg::Shell(ShellRequest::ContextMenuIntent(
                crate::app::components::msg::ContextMenuIntent::MoveDown
            )))
        );
    }

    #[test]
    fn unbound_key_is_swallowed_locally() {
        let mut comp = ContextMenuComponent::new();
        assert_eq!(
            comp.on(&Event::Keyboard(make_key(
                Key::Char('x'),
                KeyModifiers::NONE,
            ))),
            None
        );
    }

    #[test]
    fn non_keyboard_event_returns_none() {
        let mut comp = ContextMenuComponent::new();
        assert_eq!(comp.on(&Event::<UserEvent>::None), None);
    }

    #[test]
    fn mouse_event_selects_selectable_entry() {
        let mut comp = ContextMenuComponent::new();
        comp.set_content(
            ContextMenuAnchor::SelectedItem(PanelFocus::Library),
            vec![
                ContextMenuEntry {
                    label: "a",
                    action: Some(ContextAction::Play),
                },
                ContextMenuEntry {
                    label: "sep",
                    action: None,
                },
            ],
            0,
        );
        comp.set_rect(Rect {
            x: 10,
            y: 5,
            width: 10,
            height: 4,
        });
        let msg = comp.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 6, // inner row 1 -> entry index 0
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::ContextMenuSelect(0)))
        ));
    }

    fn dismissable_menu() -> ContextMenuComponent {
        let mut comp = ContextMenuComponent::new();
        comp.set_content(
            ContextMenuAnchor::SelectedItem(PanelFocus::Library),
            vec![ContextMenuEntry {
                label: "a",
                action: Some(ContextAction::Play),
            }],
            0,
        );
        comp.set_rect(Rect {
            x: 10,
            y: 5,
            width: 10,
            height: 4,
        });
        comp
    }

    #[test]
    fn mouse_click_outside_the_menu_dismisses_like_esc() {
        let mut comp = dismissable_menu();
        let msg = comp.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0, // far above/left of the painted rect
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::ContextMenuDismiss))
        ));
    }

    #[test]
    fn mouse_wheel_does_not_mutate_the_menu() {
        let mut comp = dismissable_menu();
        let msg = comp.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 12,
            row: 6,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(msg, None, "wheel is not part of the menu's vocabulary");
        assert_eq!(comp.cursor(), 0, "the highlight must not move");
    }
}
