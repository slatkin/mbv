//! Interactive Component for the Sessions sidebar.
//!
//! The shell supplies an owned snapshot of the already-resolved Emby/Cast
//! targets. This component owns selection and hit geometry; connecting,
//! detaching, and refreshing targets remain shell work.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::list::{ThreeLineFlatList, ThreeLineItem, ThreeLineRole, ThreeLineSpan, Viewported};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
use super::user_event::UserEvent;
use crate::app::state::panel_targets::{PanelTarget, SessionTargetKey};

/// The Interactive Component for the Sessions sidebar.
pub struct SessionsComponent {
    targets: Vec<PanelTarget>,
    loading: bool,
    list: ThreeLineFlatList<SessionTargetKey>,
    content_dirty: bool,
    projected_width: Option<u16>,
    connected_session_id: Option<String>,
    cast_attachment_id: Option<String>,
    can_disconnect: bool,
    requested_panel_area: Option<Rect>,
    painted_panel_area: Option<Rect>,
    #[cfg(test)]
    painted_content_area: Option<Rect>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl SessionsComponent {
    pub fn new() -> Self {
        let mut list = ThreeLineFlatList::new(0);
        list.set_focused(true);
        Self {
            targets: Vec::new(),
            loading: false,
            list,
            content_dirty: true,
            projected_width: None,
            connected_session_id: None,
            cast_attachment_id: None,
            can_disconnect: false,
            requested_panel_area: None,
            painted_panel_area: None,
            #[cfg(test)]
            painted_content_area: None,
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Replace the shell-owned target snapshot; selection is preserved by key
    /// when the next frame projects its presentation into the embedded list.
    pub(in crate::app) fn set_content(
        &mut self,
        targets: &[PanelTarget],
        loading: bool,
        connected_session_id: Option<&str>,
        cast_attachment_id: Option<&str>,
        can_disconnect: bool,
        panel_area: Option<Rect>,
    ) {
        let geometry_changed = self.requested_panel_area != panel_area
            || self
                .targets
                .iter()
                .map(PanelTarget::key)
                .ne(targets.iter().map(PanelTarget::key));
        self.targets = targets.to_vec();
        self.loading = loading;
        self.connected_session_id = connected_session_id.map(str::to_owned);
        self.cast_attachment_id = cast_attachment_id.map(str::to_owned);
        self.can_disconnect = can_disconnect;
        self.requested_panel_area = panel_area;
        self.content_dirty = true;
        if geometry_changed {
            self.list.invalidate_paint();
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Char('q') if key.modifiers.is_empty() => Some(Msg::Shell(ShellRequest::Quit)),
            Key::Esc | Key::Function(3) => Some(Msg::Shell(ShellRequest::DismissSessions)),
            Key::Function(2) => Some(Msg::Shell(ShellRequest::OpenSettings)),
            Key::Function(4) => Some(Msg::Shell(ShellRequest::OpenPlaylists)),
            Key::Up => {
                self.list.move_selection(-1);
                None
            }
            Key::Down => {
                self.list.move_selection(1);
                None
            }
            Key::Char('r') => Some(Msg::Shell(ShellRequest::RefreshSessions)),
            Key::Enter => self
                .list
                .selected_target()
                .cloned()
                .map(|key| Msg::Shell(ShellRequest::SelectSession(key))),
            Key::Char('d') if self.can_disconnect || self.cast_attachment_id.is_some() => {
                Some(Msg::Shell(ShellRequest::DetachSessions))
            }
            _ => None,
        }
    }

    /// The parent recognizes gestures; the embedded list resolves completed
    /// item geometry and owns selection.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { delta, .. } => {
                self.list.move_selection(if delta < 0 { -1 } else { 1 });
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::Click { at, .. } | MouseGesture::DoubleClick(at) => {
                if !self
                    .painted_panel_area
                    .is_some_and(|area| area.contains(at))
                {
                    return Some(Msg::Shell(ShellRequest::DismissSessions));
                }
                let target = self.list.resolve_point(at).cloned()?;
                if self.list.selected_target() == Some(&target) {
                    Some(Msg::Shell(ShellRequest::SelectSession(target)))
                } else {
                    self.list.select_target(&target);
                    None
                }
            }
            _ => None,
        }
    }

    fn project_targets(&self, text_width: usize) -> Vec<ThreeLineItem<SessionTargetKey>> {
        self.targets
            .iter()
            .map(|target| {
                let key = target.key();
                let connected = match target {
                    PanelTarget::Emby(session) => {
                        self.connected_session_id.as_deref() == Some(session.id.as_str())
                    }
                    PanelTarget::Cast(receiver) => {
                        self.cast_attachment_id.as_deref() == Some(receiver.id.as_str())
                    }
                };
                let badge = if connected { " ✚" } else { "" };
                let lines = match target {
                    PanelTarget::Emby(session) => {
                        let meta = format!(
                            "{} · {}@{}",
                            session.client, session.user_name, session.host
                        );
                        let state_icon = if session.now_playing.is_some() {
                            if session.is_paused {
                                "⏸"
                            } else {
                                "▶"
                            }
                        } else {
                            "■"
                        };
                        let time = if session.now_playing.is_some() {
                            format!(
                                " {}/{}",
                                crate::app::ui_util::fmt_duration_short(session.position_s),
                                crate::app::ui_util::fmt_duration_short(session.runtime_s)
                            )
                        } else {
                            String::new()
                        };
                        let title = session.now_playing.as_deref().unwrap_or("idle");
                        let title_width = text_width
                            .saturating_sub(state_icon.len() + 1)
                            .saturating_sub(time.len());
                        [
                            vec![
                                ThreeLineSpan::new("[EMBY] ", ThreeLineRole::Kind),
                                ThreeLineSpan::new(
                                    crate::app::ui_util::trunc_str(
                                        &session.device_name,
                                        text_width.saturating_sub(7).saturating_sub(badge.len()),
                                    ),
                                    ThreeLineRole::Name,
                                ),
                                ThreeLineSpan::new(badge, ThreeLineRole::Accent),
                            ],
                            vec![ThreeLineSpan::new(meta, ThreeLineRole::Detail)],
                            vec![ThreeLineSpan::new(
                                format!(
                                    "{state_icon} {}{time}",
                                    crate::app::ui_util::trunc_str(title, title_width)
                                ),
                                ThreeLineRole::Status,
                            )],
                        ]
                    }
                    PanelTarget::Cast(receiver) => [
                        vec![
                            ThreeLineSpan::new("[CAST] ", ThreeLineRole::Kind),
                            ThreeLineSpan::new(
                                crate::app::ui_util::trunc_str(
                                    &receiver.friendly_name,
                                    text_width.saturating_sub(7).saturating_sub(badge.len()),
                                ),
                                ThreeLineRole::Name,
                            ),
                            ThreeLineSpan::new(badge, ThreeLineRole::Accent),
                        ],
                        vec![ThreeLineSpan::new(
                            format!("{}:{}", receiver.host, receiver.port),
                            ThreeLineRole::Detail,
                        )],
                        Vec::new(),
                    ],
                };
                ThreeLineItem::new(key, lines)
            })
            .collect()
    }

    /// Test seam: forget the last click so the next event is neither
    /// throttled nor promoted to a double-click.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }

    #[cfg(test)]
    pub(crate) fn selection_and_offset_for_test(&self) -> (Option<SessionTargetKey>, usize) {
        (
            self.list.selected_target().cloned(),
            Viewported::viewport_offset(&self.list),
        )
    }

    #[cfg(test)]
    pub(crate) fn content_area_for_test(&self) -> Option<Rect> {
        self.painted_content_area
    }

    #[cfg(test)]
    pub(crate) fn target_at_for_test(
        &self,
        point: ratatui::layout::Position,
    ) -> Option<SessionTargetKey> {
        self.list.resolve_point(point).cloned()
    }
}

impl Default for SessionsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SessionsComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        let (panel_area, content_area, has_content) =
            crate::app::render::render_sessions_overlay_content(
                f,
                self.requested_panel_area,
                self.targets.len(),
                self.loading,
                self.can_disconnect,
            );
        self.painted_panel_area = Some(panel_area);
        #[cfg(test)]
        {
            self.painted_content_area = Some(content_area);
        }
        if has_content {
            if self.content_dirty || self.projected_width != Some(content_area.width) {
                self.list
                    .set_content(self.project_targets(content_area.width as usize));
                self.projected_width = Some(content_area.width);
                self.content_dirty = false;
            }
            self.list.view(f, content_area);
            crate::app::render::render_sessions_scrollbar(
                f,
                content_area,
                self.list.items().len(),
                self.list.viewport_offset(),
                self.list.gap(),
            );
        } else {
            self.list.invalidate_paint();
        }
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
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(message)).into_option(),
                None if matches!(
                    key.code,
                    Key::Up | Key::Down | Key::Enter | Key::Esc | Key::Char(_)
                ) =>
                {
                    LeafKeyResult::Consumed(None).into_option()
                }
                None => LeafKeyResult::Unhandled.into_option(),
            },
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::Terminal;
    use tuirealm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn key(code: Key) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_session_local_navigation_stays_local() {
        let mut component = SessionsComponent::new();
        component.list.set_content(vec![
            ThreeLineItem::new(
                SessionTargetKey::Emby("a".into()),
                std::array::from_fn(|_| Vec::new()),
            ),
            ThreeLineItem::new(
                SessionTargetKey::Emby("b".into()),
                std::array::from_fn(|_| Vec::new()),
            ),
        ]);
        component.handle_key(&key(Key::Down));
        assert_eq!(
            component.list.selected_target(),
            Some(&SessionTargetKey::Emby("b".into()))
        );
        component.handle_key(&key(Key::Up));
        assert_eq!(
            component.list.selected_target(),
            Some(&SessionTargetKey::Emby("a".into()))
        );
    }

    #[test]
    fn test_session_disconnect_key_detaches_cast_only_attachment() {
        let mut component = SessionsComponent::new();
        component.can_disconnect = false;
        component.cast_attachment_id = Some("cast-1".to_string());
        assert_eq!(
            component.handle_key(&key(Key::Char('d'))),
            Some(Msg::Shell(ShellRequest::DetachSessions))
        );
    }

    #[test]
    fn test_session_cross_boundary_keys_are_typed() {
        let mut component = SessionsComponent::new();
        assert_eq!(
            component.handle_key(&key(Key::Esc)),
            Some(Msg::Shell(ShellRequest::DismissSessions))
        );
        assert_eq!(
            component.handle_key(&key(Key::Char('r'))),
            Some(Msg::Shell(ShellRequest::RefreshSessions))
        );
        component.list.set_content(vec![ThreeLineItem::new(
            SessionTargetKey::Cast("cast-1".into()),
            std::array::from_fn(|_| Vec::new()),
        )]);
        assert_eq!(
            component.handle_key(&key(Key::Enter)),
            Some(Msg::Shell(ShellRequest::SelectSession(
                SessionTargetKey::Cast("cast-1".into())
            )))
        );
    }

    fn painted_component() -> SessionsComponent {
        use crate::app::tests::make_session;
        let mut first = make_session("a", "mbv");
        first.id = "a".to_string();
        let mut second = make_session("b", "mbv");
        second.id = "b".to_string();
        let targets = vec![
            PanelTarget::Emby(Box::new(first)),
            PanelTarget::Emby(Box::new(second)),
        ];
        let mut component = SessionsComponent::new();
        component.set_content(
            &targets,
            false,
            None,
            None,
            false,
            Some(Rect::new(0, 0, 40, 12)),
        );
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        component
    }

    fn left_down(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn cell_x(buffer: &ratatui::buffer::Buffer, row: u16, symbol: &str) -> u16 {
        (0..buffer.area.width)
            .find(|&x| buffer[(x, row)].symbol() == symbol)
            .unwrap_or_else(|| panic!("no {symbol:?} painted on row {row}"))
    }

    #[test]
    fn test_session_buffer_zebra_adjacency_selection_and_hit_geometry() {
        use crate::app::palette;
        use crate::app::tests::make_session;

        let mut first = make_session("first", "Emby");
        first.id = "first".into();
        let mut second = make_session("connected", "Emby");
        second.id = "connected".into();
        let targets = [
            PanelTarget::Emby(Box::new(first)),
            PanelTarget::Emby(Box::new(second)),
        ];
        let area = Rect::new(0, 0, 50, 16);
        let mut component = SessionsComponent::new();
        component.set_content(&targets, false, Some("connected"), None, false, Some(area));
        component.list.set_focused(false);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();

        let content = component.painted_content_area.unwrap();
        let y = content.y;
        let stripe = palette::SESSIONS_STRIPE_BG;
        let surface = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
        let buffer = terminal.backend().buffer();
        // With gap 0 the items are adjacent: item 0 owns y..y+3 and item 1 owns
        // y+3..y+6, alternating stripes with no blank row between them.
        for row in y..y + 3 {
            assert_eq!(buffer[(content.x, row)].bg, surface, "first item unstriped");
        }
        for row in y + 3..y + 6 {
            assert_eq!(buffer[(content.x, row)].bg, stripe, "second item's zebra");
        }
        // Every painted line resolves to its own item; no dead rows between.
        let first_key = SessionTargetKey::Emby("first".into());
        let second_key = SessionTargetKey::Emby("connected".into());
        for row in y..y + 3 {
            assert_eq!(
                component.target_at_for_test(Position {
                    x: content.x,
                    y: row
                }),
                Some(first_key.clone()),
                "line {row} resolves to item 0"
            );
        }
        for row in y + 3..y + 6 {
            assert_eq!(
                component.target_at_for_test(Position {
                    x: content.x,
                    y: row
                }),
                Some(second_key.clone()),
                "line {row} resolves to item 1"
            );
        }
        let badge_x = cell_x(buffer, y + 3, "✚");
        assert_eq!(buffer[(badge_x, y + 3)].fg, palette::ACCENT_ACTIVE);

        component.list.set_focused(true);
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        // The selected bar spans exactly item 0's three lines and stops there.
        for row in y..y + 3 {
            assert_eq!(buffer[(content.x, row)].bg, palette::SELECTED_ROW_BG);
        }
        assert_eq!(
            buffer[(content.x, y + 3)].bg,
            stripe,
            "selection bar does not bleed into the next item"
        );

        component
            .list
            .select_target(&SessionTargetKey::Emby("connected".into()));
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        for row in y + 3..y + 6 {
            assert_eq!(buffer[(content.x, row)].bg, palette::SELECTED_ROW_BG);
        }
        let badge_x = cell_x(buffer, y + 3, "✚");
        assert_eq!(buffer[(badge_x, y + 3)].fg, palette::ACCENT_ACTIVE);
        assert_eq!(
            buffer[(content.x, y + 6)].bg,
            surface,
            "selection bar ends with item 1"
        );
    }

    #[test]
    fn test_session_mouse_click_on_selected_item_connects() {
        let mut component = painted_component();
        let content = component.painted_content_area.unwrap();
        assert_eq!(
            component.handle_mouse(&left_down(content.x, content.y)),
            Some(Msg::Shell(ShellRequest::SelectSession(
                SessionTargetKey::Emby("a".into())
            )))
        );
    }

    #[test]
    fn test_session_mouse_click_on_unselected_item_selects_then_reclick_connects() {
        let mut component = painted_component();
        let point = Position {
            x: 10,
            y: component.painted_content_area.unwrap().y + 4,
        };
        // The second item starts immediately after the first item's three lines.
        assert_eq!(component.handle_mouse(&left_down(point.x, point.y)), None);
        assert_eq!(
            component.list.selected_target(),
            Some(&SessionTargetKey::Emby("b".into()))
        );
        component.view_for_test();
        component.reset_mouse_gestures_for_test();
        assert_eq!(
            component.handle_mouse(&left_down(point.x, point.y)),
            Some(Msg::Shell(ShellRequest::SelectSession(
                SessionTargetKey::Emby("b".into())
            )))
        );
    }

    #[test]
    fn test_session_mouse_click_outside_painted_panel_dismisses() {
        let mut component = painted_component();
        assert_eq!(
            component.handle_mouse(&left_down(100, 100)),
            Some(Msg::Shell(ShellRequest::DismissSessions))
        );
    }

    #[test]
    fn test_session_mouse_wheel_steps_selection_independent_of_pointer_location() {
        let mut component = painted_component();
        component.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            component.list.selected_target(),
            Some(&SessionTargetKey::Emby("b".into()))
        );
        component.reset_mouse_gestures_for_test();
        component.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 50,
            row: 20,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            component.list.selected_target(),
            Some(&SessionTargetKey::Emby("a".into()))
        );
    }

    impl SessionsComponent {
        fn view_for_test(&mut self) {
            let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
            terminal
                .draw(|frame| self.view(frame, frame.area()))
                .unwrap();
        }
    }
}
