//! Interactive Component for the Help sidebar overlay (design D3–D9).
//!
//! Owns the help scroll offset and destination context. The shell Model
//! computes the destination (`help_destination`) from current app state and
//! writes it (plus the panel area) into the component via
//! `get_component_mut`+downcast before each render. The component handles
//! keyboard and mouse input in `on()`, renders via the existing render
//! substrate in `view()`, and emits `Msg::Shell(...)` for cross-boundary work
//! (quit, switch panels, dismiss). Local state changes (scroll) return
//! `None`; the permanent root observer remains the redraw signal (design D12).

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
use super::user_event::UserEvent;
use crate::app::render::{
    help_destination, render_help_panel, HelpDestination, HelpRenderGeometry,
};
use crate::app::{PanelFocus, TabSelection};
use mbv_keybinds::Keybinds;

/// The Interactive Component for the Help sidebar.
///
/// Owns `scroll` and `destination` (set by the shell). `panel_area` is stored
/// during `view()` for hit-testing in `on()` (design D8: component-owned
/// geometry).
pub struct HelpComponent {
    scroll: u16,
    destination: HelpDestination,
    /// The compiled keybind configuration the shell renders with (plain
    /// data, projected by the shell like the destination above): help's
    /// Playback rows show the chords the loaded configuration fires on.
    keybinds: Keybinds,
    /// The area requested by the shell for rendering. The painted area is
    /// retained separately because a missing request paints the full frame.
    panel_area: Option<Rect>,
    /// The area painted by the last `view()`, used for mouse hit-testing.
    painted_panel_area: Option<Rect>,
    content_geometry: Option<HelpRenderGeometry>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl HelpComponent {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            destination: HelpDestination::EmbyLibrary,
            keybinds: Keybinds::default(),
            panel_area: None,
            painted_panel_area: None,
            content_geometry: None,
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Set the destination context from app state. Called by the shell via
    /// `get_component_mut`+downcast before each render (design D5).
    pub(in crate::app) fn set_destination(&mut self, panel_focus: PanelFocus, tab: TabSelection) {
        self.destination = help_destination(panel_focus, tab);
    }

    /// Set the compiled keybind configuration from the shell. Called via
    /// `get_component_mut`+downcast before each render, so the rendered
    /// Playback rows follow the loaded configuration (design D7).
    pub(in crate::app) fn set_keybinds(&mut self, keybinds: &Keybinds) {
        self.keybinds = keybinds.clone();
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
            Key::Char('q') if key.modifiers.is_empty() => {
                Some(Msg::Shell(Box::new(ShellRequest::Quit)))
            }
            Key::Esc | Key::Function(1) => Some(Msg::Shell(Box::new(ShellRequest::DismissHelp))),
            Key::Function(2) => Some(Msg::Shell(Box::new(ShellRequest::OpenSettings))),
            Key::Function(3) => Some(Msg::Shell(Box::new(ShellRequest::OpenSessions))),
            Key::Function(4) => Some(Msg::Shell(Box::new(ShellRequest::OpenPlaylists))),
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

    /// Handle a mouse event (task 5.2): recognition via the component's own
    /// `MouseGestureState` (ADR 0024, design.md D3). Behaviour unchanged
    /// from the ad-hoc handler: a click inside the panel is swallowed, a
    /// click outside dismisses (the second click of a double included), and
    /// the focused overlay's wheel adjusts the scroll by one line regardless of pointer.
    #[cfg(test)]
    pub(crate) fn test_scroll(&self) -> u16 {
        self.scroll
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click { at, .. } | MouseGesture::DoubleClick(at) => {
                if self
                    .painted_panel_area
                    .is_some_and(|area| area.contains(at))
                {
                    // Click inside: swallow.
                    None
                } else {
                    // Click outside the panel: dismiss.
                    Some(Msg::Shell(Box::new(ShellRequest::DismissHelp)))
                }
            }
            MouseGesture::Scroll { delta, .. } => {
                let geometry = self.content_geometry.as_ref()?;
                let delta = i16::try_from(delta).unwrap_or(if delta.is_negative() {
                    i16::MIN
                } else {
                    i16::MAX
                });
                self.scroll = self
                    .scroll
                    .saturating_add_signed(delta)
                    .min(geometry.max_scroll);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
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
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        // Use the panel area set by the shell (via `set_panel_area`), not
        // the `area` parameter from TuiRealm (which is the full terminal).
        let geometry = render_help_panel(
            frame,
            self.panel_area,
            &mut self.scroll,
            self.destination,
            &self.keybinds,
        );
        self.painted_panel_area = Some(geometry.panel_area);
        self.content_geometry = Some(geometry);
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

impl AppComponent<Msg, UserEvent> for HelpComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))).into_option(),
                None if matches!(
                    key.code,
                    Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home
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
    use rstest::rstest;
    use tuirealm::event::{Key, KeyModifiers};

    fn make_key(code: Key, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers }
    }

    #[rstest]
    #[case::scroll_down_increments_scroll(5, Key::Down, 6)]
    fn scroll(#[case] initial: u16, #[case] key: Key, #[case] expected: u16) {
        let mut comp = HelpComponent::new();
        comp.scroll = initial;
        comp.handle_key(&make_key(key, KeyModifiers::NONE));
        assert_eq!(comp.scroll, expected);
    }

    #[rstest]
    #[case::quit_emits_shell_quit(
        Key::Char('q'),
        KeyModifiers::NONE,
        Some(Msg::Shell(Box::new(ShellRequest::Quit)))
    )]
    #[case::escape_emits_dismiss_help(
        Key::Esc,
        KeyModifiers::NONE,
        Some(Msg::Shell(Box::new(ShellRequest::DismissHelp)))
    )]
    #[case::unbound_key_is_swallowed(Key::Char('x'), KeyModifiers::NONE, None)]
    fn key_to_msg(
        #[case] key: Key,
        #[case] modifiers: KeyModifiers,
        #[case] expected: Option<Msg>,
    ) {
        let mut comp = HelpComponent::new();
        let msg = comp.handle_key(&make_key(key, modifiers));
        assert_eq!(msg, expected);
    }
}
