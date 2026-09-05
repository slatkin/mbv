//! Interactive Component for the Daemon-lost modal overlay (design D3–D9).
//!
//! Owns the modal's display content (last_playing_title, daemon_log_path,
//! restart_error) set by the shell via downcast before each render. The shell
//! owns restart/quit dispatch in the shell; the component interprets keys as
//! semantic restart/quit intents. Mouse and other events are swallowed by the
//! blocking modal; UiRoot's permanent observer supplies the redraw signal
//! (design D12).

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{DaemonLostIntent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_daemon_lost_modal_content;

/// The Interactive Component for the Daemon-lost modal.
///
/// Owns display content set by the shell via `get_component_mut`+downcast
/// before each render. The `dim_backdrop_active` field is a scratch flag for
/// `render_modal_frame` (same pattern as `ConfirmComponent`).
pub struct DaemonLostComponent {
    last_playing_title: Option<String>,
    daemon_log_path: String,
    restart_error: Option<String>,
    dim_backdrop_active: bool,
}

impl DaemonLostComponent {
    pub fn new() -> Self {
        Self {
            last_playing_title: None,
            daemon_log_path: String::new(),
            restart_error: None,
            dim_backdrop_active: false,
        }
    }

    /// Set the modal's display content from a shell request. Called
    /// by the shell via `get_component_mut`+downcast before each render.
    pub(in crate::app) fn set_content(
        &mut self,
        last_playing_title: Option<&str>,
        daemon_log_path: &str,
        restart_error: Option<&str>,
    ) {
        self.last_playing_title = last_playing_title.map(|s| s.to_string());
        self.daemon_log_path.clear();
        self.daemon_log_path.push_str(daemon_log_path);
        self.restart_error = restart_error.map(|s| s.to_string());
    }

    pub(in crate::app) fn set_restart_error(&mut self, message: String) {
        self.restart_error = Some(message);
    }
}

impl Default for DaemonLostComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DaemonLostComponent {
    fn view(&mut self, f: &mut ratatui::Frame, _area: ratatui::layout::Rect) {
        render_daemon_lost_modal_content(
            f,
            &mut self.dim_backdrop_active,
            self.last_playing_title.as_deref(),
            &self.daemon_log_path,
            self.restart_error.as_deref(),
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

impl AppComponent<Msg, UserEvent> for DaemonLostComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let Event::Keyboard(key) = ev else {
            return None;
        };
        let intent = match key.code {
            Key::Char('r') | Key::Char('R') => DaemonLostIntent::RestartWithTray,
            Key::Char('s') | Key::Char('S') => DaemonLostIntent::RestartWithoutTray,
            Key::Char('q') | Key::Char('Q') => DaemonLostIntent::Quit,
            _ => return None,
        };
        Some(Msg::Shell(ShellRequest::DaemonLostIntent(intent)))
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
    fn restart_key_emits_restart_intent() {
        let mut comp = DaemonLostComponent::new();
        let msg = comp.on(&Event::Keyboard(make_key(
            Key::Char('r'),
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::DaemonLostIntent(
                DaemonLostIntent::RestartWithTray
            )))
        ));
    }

    #[test]
    fn unbound_key_is_swallowed_locally() {
        let mut comp = DaemonLostComponent::new();
        assert_eq!(
            comp.on(&Event::Keyboard(make_key(
                Key::Char('x'),
                KeyModifiers::NONE,
            ))),
            None
        );
    }

    #[test]
    fn non_keyboard_events_return_none() {
        let mut comp = DaemonLostComponent::new();
        assert_eq!(comp.on(&Event::<UserEvent>::None), None);
        assert_eq!(
            comp.on(&Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 10,
                row: 5,
                modifiers: KeyModifiers::NONE,
            })),
            None
        );
    }

    #[test]
    fn set_content_updates_fields() {
        let mut comp = DaemonLostComponent::new();
        comp.set_content(Some("Birthday Clip"), "/tmp/mbvd.log", Some("refused"));
        assert_eq!(comp.last_playing_title, Some("Birthday Clip".into()));
        assert_eq!(comp.daemon_log_path, "/tmp/mbvd.log");
        assert_eq!(comp.restart_error, Some("refused".into()));
    }

    #[test]
    fn set_content_with_none_values() {
        let mut comp = DaemonLostComponent::new();
        comp.set_content(None, "/var/log/mbvd.log", None);
        assert_eq!(comp.last_playing_title, None);
        assert_eq!(comp.daemon_log_path, "/var/log/mbvd.log");
        assert_eq!(comp.restart_error, None);
    }
}
