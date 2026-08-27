//! Interactive Component for the Help sidebar overlay (design D3–D9).
//!
//! Owns the help scroll offset and destination context. The shell Model
//! computes the destination (`help_destination`) from current app state and
//! writes it (plus the panel area) into the component via
//! `get_component_mut`+downcast before each render. The component handles
//! keyboard and mouse input in `on()`, renders via the existing render
//! substrate in `view()`, and emits `Msg::Shell(...)` for cross-boundary work
//! (quit, switch panels, dismiss). Local state changes (scroll) return
//! `Msg::Legacy(NoOp)` — the mixed-phase redraw signal (design D12): a
//! non-empty `tick` result marks the frame dirty via `had_events`.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{help_destination, render_help_panel, HelpDestination};
use crate::app::{PanelFocus, TabSelection};

/// The Interactive Component for the Help sidebar.
///
/// Owns `scroll` and `destination` (set by the shell). `panel_area` is stored
/// during `view()` for hit-testing in `on()` (design D8: component-owned
/// geometry).
pub struct HelpComponent {
    scroll: u16,
    destination: HelpDestination,
    /// The area the panel was painted in during `view()`, used for mouse
    /// hit-testing in `on()`. `None` when no panel area was provided (the
    /// help sidebar uses the full terminal with a width constraint).
    panel_area: Option<Rect>,
}

impl HelpComponent {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            destination: HelpDestination::EmbyLibrary,
            panel_area: None,
        }
    }

    /// Set the destination context from app state. Called by the shell via
    /// `get_component_mut`+downcast before each render (design D5).
    pub(in crate::app) fn set_destination(&mut self, panel_focus: PanelFocus, tab: TabSelection) {
        self.destination = help_destination(panel_focus, tab);
    }

    /// Set the panel area for rendering and hit-testing. Called by the shell
    /// via `get_component_mut`+downcast before each render.
    pub(in crate::app) fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    /// Handle a keyboard event. Returns `Msg::Shell(...)` for cross-boundary
    /// work and `None` for local state changes or swallowed keys; the root
    /// terminal observer supplies the redraw signal (design D12).
    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        // Help swallows every key (matching legacy `handle_key_help`'s
        // unconditional `Some(false)` return for unbound keys).
        match key.code {
            Key::Char('q') if key.modifiers.is_empty() => Some(Msg::Shell(ShellRequest::Quit)),
            Key::Esc | Key::Function(1) => Some(Msg::Shell(ShellRequest::DismissHelp)),
            Key::Function(2) => Some(Msg::Shell(ShellRequest::OpenSettings)),
            Key::Function(3) => Some(Msg::Shell(ShellRequest::OpenSessions)),
            Key::Function(4) => Some(Msg::Shell(ShellRequest::OpenPlaylists)),
            Key::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                None
            }
            Key::Down => {
                self.scroll = self.scroll.saturating_add(1);
                None
            }
            Key::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                None
            }
            Key::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                None
            }
            Key::Home => {
                self.scroll = 0;
                None
            }
            // Unbound key: swallow.
            _ => None,
        }
    }

    /// Handle a mouse event. Click-outside dismisses; scroll adjusts by 3
    /// (matching legacy `handle_mouse_panels` help branch).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let (col, row) = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside = self
                    .panel_area
                    .is_some_and(|r| r.contains((col, row).into()));
                if !inside {
                    // Click outside the panel: dismiss.
                    Some(Msg::Shell(ShellRequest::DismissHelp))
                } else {
                    // Click inside: swallow.
                    None
                }
            }
            MouseEventKind::ScrollDown => {
                self.scroll = self.scroll.saturating_add(3);
                None
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(3);
                None
            }
            _ => None,
        }
    }
}

impl Default for HelpComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for HelpComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        // Use the panel area set by the shell (via `set_panel_area`), not
        // the `area` parameter from TuiRealm (which is the full terminal).
        render_help_panel(f, self.panel_area, &mut self.scroll, self.destination);
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

impl AppComponent<Msg, UserEvent> for HelpComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuirealm::event::{Key, KeyModifiers};

    fn make_key(code: Key, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers }
    }

    #[test]
    fn scroll_down_increments_scroll() {
        let mut comp = HelpComponent::new();
        comp.scroll = 5;
        comp.handle_key(&make_key(Key::Down, KeyModifiers::NONE));
        assert_eq!(comp.scroll, 6);
    }

    #[test]
    fn scroll_up_decrements_scroll() {
        let mut comp = HelpComponent::new();
        comp.scroll = 5;
        comp.handle_key(&make_key(Key::Up, KeyModifiers::NONE));
        assert_eq!(comp.scroll, 4);
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut comp = HelpComponent::new();
        comp.scroll = 0;
        comp.handle_key(&make_key(Key::Up, KeyModifiers::NONE));
        assert_eq!(comp.scroll, 0);
    }

    #[test]
    fn page_down_increments_by_ten() {
        let mut comp = HelpComponent::new();
        comp.scroll = 5;
        comp.handle_key(&make_key(Key::PageDown, KeyModifiers::NONE));
        assert_eq!(comp.scroll, 15);
    }

    #[test]
    fn page_up_decrements_by_ten() {
        let mut comp = HelpComponent::new();
        comp.scroll = 5;
        comp.handle_key(&make_key(Key::PageUp, KeyModifiers::NONE));
        assert_eq!(comp.scroll, 0);
    }

    #[test]
    fn home_resets_scroll_to_zero() {
        let mut comp = HelpComponent::new();
        comp.scroll = 42;
        comp.handle_key(&make_key(Key::Home, KeyModifiers::NONE));
        assert_eq!(comp.scroll, 0);
    }

    #[test]
    fn quit_emits_shell_quit() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Char('q'), KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::Quit)));
    }

    #[test]
    fn escape_emits_dismiss_help() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Esc, KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::DismissHelp)));
    }

    #[test]
    fn f1_emits_dismiss_help() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Function(1), KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::DismissHelp)));
    }

    #[test]
    fn f2_emits_open_settings() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Function(2), KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::OpenSettings)));
    }

    #[test]
    fn f3_emits_open_sessions() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Function(3), KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::OpenSessions)));
    }

    #[test]
    fn f4_emits_open_playlists() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Function(4), KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::OpenPlaylists)));
    }

    #[test]
    fn unbound_key_is_swallowed() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Char('x'), KeyModifiers::NONE));
        assert_eq!(msg, None);
    }

    #[test]
    fn ctrl_q_does_not_emit_quit() {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(Key::Char('q'), KeyModifiers::CONTROL));
        // Ctrl+Q is not the plain-q quit binding; it's swallowed.
        assert_eq!(msg, None);
    }

    #[test]
    fn mouse_scroll_down_increments_by_three() {
        let mut comp = HelpComponent::new();
        comp.scroll = 5;
        comp.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(comp.scroll, 8);
    }

    #[test]
    fn mouse_scroll_up_decrements_by_three() {
        let mut comp = HelpComponent::new();
        comp.scroll = 5;
        comp.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(comp.scroll, 2);
    }

    #[test]
    fn mouse_click_outside_panel_dismisses() {
        let mut comp = HelpComponent::new();
        comp.set_panel_area(Some(Rect::new(0, 0, 40, 20)));
        let msg = comp.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 50,
            row: 10,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::DismissHelp)));
    }

    #[test]
    fn mouse_click_inside_panel_is_swallowed() {
        let mut comp = HelpComponent::new();
        comp.set_panel_area(Some(Rect::new(0, 0, 40, 20)));
        let msg = comp.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(msg, None);
    }

    #[test]
    fn set_destination_from_queue_focus_returns_queue() {
        let mut comp = HelpComponent::new();
        comp.set_destination(PanelFocus::Queue, TabSelection::Home);
        assert_eq!(comp.destination, HelpDestination::Queue);
    }

    #[test]
    fn set_destination_from_library_focus_uses_tab() {
        let mut comp = HelpComponent::new();
        comp.set_destination(PanelFocus::Library, TabSelection::Feeds);
        assert_eq!(comp.destination, HelpDestination::Feeds);
    }
}
