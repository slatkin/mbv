//! Interactive Component for the Sessions sidebar.
//!
//! The shell supplies an owned snapshot of the already-resolved Emby/Cast
//! targets. This component owns only the sidebar's cursor, viewport, and hit
//! geometry; connecting, detaching, and refreshing targets remain shell work.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::panel_targets::PanelTarget;

/// The Interactive Component for the Sessions sidebar.
pub struct SessionsComponent {
    targets: Vec<PanelTarget>,
    loading: bool,
    cursor: usize,
    scroll: usize,
    connected_session_id: Option<String>,
    tracking: bool,
    cast_attachment_id: Option<String>,
    can_disconnect: bool,
    requested_panel_area: Option<Rect>,
    painted_panel_area: Option<Rect>,
    row_targets: Vec<(Rect, usize)>,
}

impl SessionsComponent {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            loading: false,
            cursor: 0,
            scroll: 0,
            connected_session_id: None,
            tracking: false,
            cast_attachment_id: None,
            can_disconnect: false,
            requested_panel_area: None,
            painted_panel_area: None,
            row_targets: Vec::new(),
        }
    }

    /// Replace the shell-owned target snapshot while preserving the local
    /// cursor when its row still exists.
    pub(in crate::app) fn set_content(
        &mut self,
        targets: &[PanelTarget],
        loading: bool,
        connected_session_id: Option<&str>,
        tracking: bool,
        cast_attachment_id: Option<&str>,
        can_disconnect: bool,
        panel_area: Option<Rect>,
    ) {
        let selected_key = self.targets.get(self.cursor).map(target_key);
        self.targets = targets.to_vec();
        self.loading = loading;
        self.cursor = selected_key
            .and_then(|key| {
                self.targets
                    .iter()
                    .position(|target| target_key(target) == key)
            })
            .unwrap_or_else(|| self.cursor.min(self.targets.len().saturating_sub(1)));
        self.connected_session_id = connected_session_id.map(str::to_owned);
        self.tracking = tracking;
        self.cast_attachment_id = cast_attachment_id.map(str::to_owned);
        self.can_disconnect = can_disconnect;
        self.requested_panel_area = panel_area;
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Char('q') if key.modifiers.is_empty() => Some(Msg::Shell(ShellRequest::Quit)),
            Key::Esc | Key::Function(3) => Some(Msg::Shell(ShellRequest::DismissSessions)),
            Key::Function(2) => Some(Msg::Shell(ShellRequest::OpenSettings)),
            Key::Function(4) => Some(Msg::Shell(ShellRequest::OpenPlaylists)),
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            Key::Down => {
                if !self.targets.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.targets.len() - 1);
                }
                None
            }
            Key::Char('r') => Some(Msg::Shell(ShellRequest::RefreshSessions)),
            Key::Enter => Some(Msg::Shell(ShellRequest::SelectSession(self.cursor))),
            Key::Char('d') if self.can_disconnect || self.cast_attachment_id.is_some() => {
                Some(Msg::Shell(ShellRequest::DetachSessions))
            }
            _ => None,
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if !self.targets.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.targets.len() - 1);
                }
                None
            }
            MouseEventKind::ScrollUp => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = (mouse.column, mouse.row).into();
                if !self
                    .painted_panel_area
                    .is_some_and(|area| area.contains(pos))
                {
                    return Some(Msg::Shell(ShellRequest::DismissSessions));
                }
                if let Some((_, index)) =
                    self.row_targets.iter().find(|(rect, _)| rect.contains(pos))
                {
                    if self.cursor == *index {
                        return Some(Msg::Shell(ShellRequest::SelectSession(*index)));
                    }
                    self.cursor = *index;
                }
                None
            }
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum TargetKey {
    Emby(String),
    Cast(String),
}

fn target_key(target: &PanelTarget) -> TargetKey {
    match target {
        PanelTarget::Emby(session) => TargetKey::Emby(session.id.clone()),
        PanelTarget::Cast(receiver) => TargetKey::Cast(receiver.id.clone()),
    }
}

impl Default for SessionsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SessionsComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        let (panel_area, rows) = crate::app::render::render_sessions_overlay_content(
            f,
            self.requested_panel_area,
            &self.targets,
            self.loading,
            &mut self.cursor,
            &mut self.scroll,
            self.connected_session_id.as_deref(),
            self.tracking,
            self.cast_attachment_id.as_deref(),
            self.can_disconnect,
        );
        self.painted_panel_area = Some(panel_area);
        self.row_targets = rows;
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

impl AppComponent<Msg, UserEvent> for SessionsComponent {
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
    use tuirealm::event::KeyModifiers;

    fn key(code: Key) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn sessions_local_navigation_stays_local() {
        let mut component = SessionsComponent::new();
        component.cursor = 1;
        let message = component.handle_key(&key(Key::Up));
        assert_eq!(component.cursor, 0);
        assert_eq!(message, None);
    }

    #[test]
    fn disconnect_key_detaches_cast_only_attachment() {
        let mut component = SessionsComponent::new();
        component.can_disconnect = false;
        component.cast_attachment_id = Some("cast-1".to_string());
        assert_eq!(
            component.handle_key(&key(Key::Char('d'))),
            Some(Msg::Shell(ShellRequest::DetachSessions))
        );
    }

    #[test]
    fn sessions_cross_boundary_keys_are_typed() {
        let mut component = SessionsComponent::new();
        assert_eq!(
            component.handle_key(&key(Key::Esc)),
            Some(Msg::Shell(ShellRequest::DismissSessions))
        );
        assert_eq!(
            component.handle_key(&key(Key::Char('r'))),
            Some(Msg::Shell(ShellRequest::RefreshSessions))
        );
        assert_eq!(
            component.handle_key(&key(Key::Enter)),
            Some(Msg::Shell(ShellRequest::SelectSession(0)))
        );
    }
}
