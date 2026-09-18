use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{
    LeafKeyResult, Msg, ServiceRequest, SettingsIntent, ShellRequest, TerminalObserverEvent,
};
use super::user_event::UserEvent;
use crate::app::render::{render_settings_content, SettingsRenderGeometry, SettingsRenderModel};
use crate::app::types_settings::SettingsDestination;
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SettingsRow {
    pub label: String,
    pub value: String,
    pub section: bool,
    pub cursor: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ServiceRow {
    pub name: String,
    pub detail: String,
    pub muted: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SetupDraft {
    Emby {
        fields: [String; 3],
        focus: usize,
        busy: bool,
        error: String,
    },
    Audiobookshelf {
        fields: [String; 2],
        focus: usize,
        busy: bool,
        error: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SettingsSnapshot {
    pub destination: SettingsDestination,
    pub rows: Vec<SettingsRow>,
    pub services: Vec<ServiceRow>,
    /// The read-only Keys destination's content (design D7): group-header
    /// rows (`section: true`) for each populated `KeySection` plus one row
    /// per declared registry action with its configured chords. The
    /// destination is read-only — chords change in the config file, never
    /// in place (ADR 0023).
    pub keys: Vec<SettingsRow>,
    pub setup: Option<SetupDraft>,
    pub area: Rect,
}

pub struct SettingsComponent {
    destination: SettingsDestination,
    rows: Vec<SettingsRow>,
    services: Vec<ServiceRow>,
    keys: Vec<SettingsRow>,
    setup: Option<SetupDraft>,
    cursor: usize,
    services_cursor: usize,
    keys_cursor: usize,
    scroll: usize,
    area: Rect,
    geometry: SettingsRenderGeometry,
    /// Irregular painted chrome (task 5.2, design.md D6): the
    /// cursor-activatable rows, repopulated in `view()` from the geometry
    /// the painter just produced. Tag = the row's cursor index.
    hit_rows: HitRegions<usize>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
    initialized: bool,
}

impl SettingsComponent {
    pub fn new() -> Self {
        Self {
            destination: SettingsDestination::Main,
            rows: Vec::new(),
            services: Vec::new(),
            keys: Vec::new(),
            setup: None,
            cursor: 0,
            services_cursor: 0,
            keys_cursor: 0,
            scroll: 0,
            area: Rect::default(),
            geometry: SettingsRenderGeometry::default(),
            hit_rows: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
            initialized: false,
        }
    }

    pub(in crate::app) fn set_content(&mut self, snapshot: SettingsSnapshot) {
        let same_setup = matches!(
            (&self.setup, &snapshot.setup),
            (Some(SetupDraft::Emby { .. }), Some(SetupDraft::Emby { .. }))
                | (
                    Some(SetupDraft::Audiobookshelf { .. }),
                    Some(SetupDraft::Audiobookshelf { .. }),
                )
        );
        if !same_setup {
            self.setup = snapshot.setup;
        } else if let (Some(current), Some(incoming)) = (&mut self.setup, snapshot.setup) {
            match (current, incoming) {
                (
                    SetupDraft::Emby { busy, error, .. },
                    SetupDraft::Emby {
                        busy: next_busy,
                        error: next_error,
                        ..
                    },
                )
                | (
                    SetupDraft::Audiobookshelf { busy, error, .. },
                    SetupDraft::Audiobookshelf {
                        busy: next_busy,
                        error: next_error,
                        ..
                    },
                ) => {
                    *busy = next_busy;
                    *error = next_error;
                }
                _ => {}
            }
        }
        let destination_changed = self.destination != snapshot.destination;
        self.destination = snapshot.destination;
        self.rows = snapshot.rows;
        self.services = snapshot.services;
        self.keys = snapshot.keys;
        // The component owns its interaction state; content pushes never
        // carry cursor/scroll values. First content (and each destination
        // change) starts from the component-local defaults — Services
        // re-entry is already handled by the Back reset, so a fresh
        // destination begins at the top. Only the bounds follow the pushed
        // content, so a cursor can never land on a removed row.
        if !self.initialized || destination_changed {
            self.cursor = 0;
            self.services_cursor = 0;
            self.keys_cursor = 0;
            self.scroll = 0;
        }
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.services_cursor = self
            .services_cursor
            .min(self.services.len().saturating_sub(1));
        // The Keys cursor numbers actions only (headers carry no cursor),
        // matching the painter's highlight and the `cursor_lines` geometry
        // indexed by action ordinal.
        self.keys_cursor = self
            .keys_cursor
            .min(self.keys_action_count().saturating_sub(1));
        self.area = snapshot.area;
        self.initialized = true;
    }

    fn service_key(&self, key: &KeyEvent) -> Option<Msg> {
        let request = match key.code {
            Key::Enter | Key::Char(' ') => ServiceRequest::ActivateService(self.services_cursor),
            Key::Char('d') | Key::Char('D') if self.services_cursor == 0 => {
                ServiceRequest::RemoveEmby
            }
            Key::Char('t') | Key::Char('T') if self.services_cursor == 1 => {
                ServiceRequest::TestAudiobookshelfConnection
            }
            Key::Char('r') | Key::Char('R') if self.services_cursor == 1 => {
                ServiceRequest::ReplaceAudiobookshelf
            }
            Key::Char('d') | Key::Char('D') if self.services_cursor == 1 => {
                ServiceRequest::RemoveAudiobookshelf
            }
            _ => return None,
        };
        Some(Msg::Service(request))
    }

    fn setup_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let setup = self.setup.as_mut()?;
        let busy = match setup {
            SetupDraft::Emby { busy, .. } | SetupDraft::Audiobookshelf { busy, .. } => *busy,
        };
        if busy {
            return (key.code == Key::Esc).then_some(Msg::Service(ServiceRequest::CancelSetup));
        }
        match setup {
            SetupDraft::Emby {
                fields,
                focus,
                error,
                ..
            } => Self::edit_form(key, fields, focus, error, 3),
            SetupDraft::Audiobookshelf {
                fields,
                focus,
                error,
                ..
            } => Self::edit_form(key, fields, focus, error, 2),
        }
    }

    fn edit_form(
        key: &KeyEvent,
        fields: &mut [String],
        focus: &mut usize,
        error: &mut String,
        field_count: usize,
    ) -> Option<Msg> {
        match key.code {
            Key::Esc => Some(Msg::Service(ServiceRequest::CancelSetup)),
            Key::Tab | Key::Down => {
                *focus = (*focus + 1) % field_count;
                None
            }
            Key::BackTab | Key::Up => {
                *focus = if *focus == 0 {
                    field_count - 1
                } else {
                    *focus - 1
                };
                None
            }
            Key::Enter if *focus + 1 < field_count => {
                *focus += 1;
                None
            }
            Key::Enter => {
                let request = if field_count == 3 {
                    ServiceRequest::SubmitEmbySetup {
                        server_url: fields[0].clone(),
                        username: fields[1].clone(),
                        password: fields[2].clone(),
                    }
                } else {
                    ServiceRequest::SubmitAudiobookshelfSetup {
                        server_url: fields[0].clone(),
                        api_key: fields[1].clone(),
                    }
                };
                Some(Msg::Service(request))
            }
            Key::Backspace => {
                fields[*focus].pop();
                error.clear();
                None
            }
            Key::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                fields[*focus].push(c);
                error.clear();
                None
            }
            _ => None,
        }
    }

    /// Keys rows: the count of cursor-numbered action rows (group headers
    /// are not addressable). The Keys cursor is an ordinal over actions so
    /// it stays aligned with the painter's highlight and the render
    /// geometry's `cursor_lines`.
    fn keys_action_count(&self) -> usize {
        self.keys.iter().filter(|row| row.cursor.is_some()).count()
    }

    /// Largest valid scroll offset: the last geometry line fully in view.
    fn max_scroll(&self) -> usize {
        self.geometry
            .cursor_lines
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_sub((self.geometry.content_area.height as usize).saturating_sub(1))
    }

    /// Keep the Keys cursor's document line inside the scrolled window
    /// (geometry from the latest paint; a no-op before the first paint).
    fn scroll_keys_to_cursor(&mut self) {
        let Some(&line) = self.geometry.cursor_lines.get(self.keys_cursor) else {
            return;
        };
        let height = self.geometry.content_area.height as usize;
        if height == 0 {
            return;
        }
        if line < self.scroll {
            self.scroll = line;
        } else if line >= self.scroll + height {
            self.scroll = line + 1 - height;
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.setup.is_some() {
            return self.setup_key(key);
        }
        if self.destination == SettingsDestination::Keys {
            return match key.code {
                Key::Up => {
                    self.keys_cursor = self.keys_cursor.saturating_sub(1);
                    self.scroll_keys_to_cursor();
                    None
                }
                Key::Down => {
                    self.keys_cursor =
                        (self.keys_cursor + 1).min(self.keys_action_count().saturating_sub(1));
                    self.scroll_keys_to_cursor();
                    None
                }
                Key::PageUp => {
                    self.scroll = self.scroll.saturating_sub(10);
                    None
                }
                Key::PageDown => {
                    self.scroll = self.scroll.saturating_add(10).min(self.max_scroll());
                    None
                }
                Key::Home => {
                    self.scroll = 0;
                    None
                }
                Key::End => {
                    self.scroll = self.max_scroll();
                    None
                }
                // Read-only destination (design D7): Enter/Space select
                // nothing — the chord changes in the config file, never in
                // place (ADR 0023). Returning None lets `on()`'s fallback
                // claim them like the cursor moves.
                Key::Esc => {
                    // Leaving Keys zeroes the local cursor so the next
                    // entry starts at the top (the Services precedent).
                    self.keys_cursor = 0;
                    Some(Msg::Shell(ShellRequest::SettingsIntent(
                        SettingsIntent::Back,
                    )))
                }
                Key::Function(3) => Some(Msg::Shell(ShellRequest::SettingsIntent(
                    SettingsIntent::OpenSessions,
                ))),
                Key::Function(4) => Some(Msg::Shell(ShellRequest::SettingsIntent(
                    SettingsIntent::OpenPlaylists,
                ))),
                Key::Char('q') => Some(Msg::Shell(ShellRequest::SettingsIntent(
                    SettingsIntent::Quit,
                ))),
                _ => None,
            };
        }
        if self.destination == SettingsDestination::Services {
            return match key.code {
                Key::Up => {
                    self.services_cursor = self.services_cursor.saturating_sub(1);
                    None
                }
                Key::Down => {
                    self.services_cursor =
                        (self.services_cursor + 1).min(self.services.len().saturating_sub(1));
                    None
                }
                Key::Enter
                | Key::Char(' ')
                | Key::Char('d')
                | Key::Char('D')
                | Key::Char('t')
                | Key::Char('T')
                | Key::Char('r')
                | Key::Char('R') => self.service_key(key),
                Key::Esc | Key::Function(3) | Key::Function(4) | Key::Char('q') => {
                    let intent = match key.code {
                        Key::Esc => SettingsIntent::Back,
                        Key::Function(3) => SettingsIntent::OpenSessions,
                        Key::Function(4) => SettingsIntent::OpenPlaylists,
                        Key::Char('q') => SettingsIntent::Quit,
                        _ => unreachable!(),
                    };
                    if matches!(intent, SettingsIntent::Back) {
                        // Leaving Services zeroes the local cursor so the next
                        // entry starts at the top; the shell-side mirror of
                        // this reset is being deleted.
                        self.services_cursor = 0;
                    }
                    Some(Msg::Shell(ShellRequest::SettingsIntent(intent)))
                }
                _ => None,
            };
        }
        match key.code {
            Key::Esc | Key::Function(3) | Key::Function(4) | Key::Char('q') => {
                Some(Msg::Shell(ShellRequest::SettingsIntent(match key.code {
                    Key::Esc => SettingsIntent::Back,
                    Key::Function(3) => SettingsIntent::OpenSessions,
                    Key::Function(4) => SettingsIntent::OpenPlaylists,
                    Key::Char('q') => SettingsIntent::Quit,
                    _ => unreachable!(),
                })))
            }
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            Key::Down => {
                self.cursor = (self.cursor + 1).min(self.rows.len().saturating_sub(1));
                None
            }
            Key::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                None
            }
            Key::Left | Key::Right | Key::Char(' ') | Key::Enter => Some(Msg::Shell(
                ShellRequest::SettingsIntent(SettingsIntent::Activate(self.cursor)),
            )),
            _ => None,
        }
    }

    /// Mouse handling (task 5.2): recognition via the component's own
    /// `MouseGestureState` (ADR 0024, design.md D3); row geometry via
    /// `HitRegions` (D6). Behaviour unchanged from the ad-hoc handler: a
    /// click outside the panel dismisses, a click on a cursor-activatable
    /// row selects and activates it (the Enter/Space equivalent), and the
    /// focused overlay's wheel scrolls by one document line per throttled step.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click { at, .. } | MouseGesture::DoubleClick(at) => {
                if !self.geometry.panel_area.contains(at) {
                    return Some(Msg::Shell(ShellRequest::DismissSettings));
                }
                let &cursor = self.hit_rows.resolve(at)?;
                if self.destination == SettingsDestination::Services {
                    self.services_cursor = cursor;
                    return Some(Msg::Service(ServiceRequest::ActivateService(cursor)));
                }
                if self.destination == SettingsDestination::Keys {
                    // Read-only destination: a click selects the row; there
                    // is no activation.
                    self.keys_cursor = cursor;
                    return None;
                }
                self.cursor = cursor;
                Some(Msg::Shell(ShellRequest::SettingsIntent(
                    SettingsIntent::Activate(cursor),
                )))
            }
            MouseGesture::Scroll { delta, .. } => {
                let max_scroll = self.max_scroll();
                self.scroll = self
                    .scroll
                    .saturating_add_signed(delta as isize)
                    .min(max_scroll);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_rows(&self) -> &HitRegions<usize> {
        &self.hit_rows
    }
}

impl Default for SettingsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SettingsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = if self.area.width > 0 { self.area } else { area };
        // The Keys destination paints through the same row painter as the
        // main list (group headers + label/value rows); only the content
        // source and its cursor differ.
        let (rows, cursor) = if self.destination == SettingsDestination::Keys {
            (&self.keys, self.keys_cursor)
        } else {
            (&self.rows, self.cursor)
        };
        render_settings_content(
            frame,
            area,
            SettingsRenderModel {
                destination: self.destination,
                rows,
                keys: &self.keys,
                services: &self.services,
                setup: self.setup.as_ref(),
                cursor,
                services_cursor: self.services_cursor,
                scroll: self.scroll,
            },
            &mut self.geometry,
        );
        // Adopt the cursor-activatable rows the painter just produced into
        // the irregular-chrome registry (task 5.2, design.md D6). The
        // painter's `cursor_lines` are document line numbers indexed by
        // cursor; document line `line` paints at `content_area.y + line -
        // scroll`.
        self.hit_rows.clear();
        for (cursor, &line) in self.geometry.cursor_lines.iter().enumerate() {
            if line < self.scroll {
                continue;
            }
            let offset = (line - self.scroll) as u16;
            if offset >= self.geometry.content_area.height {
                continue;
            }
            self.hit_rows.push(
                Rect {
                    x: self.geometry.content_area.x,
                    y: self.geometry.content_area.y + offset,
                    width: self.geometry.content_area.width,
                    height: 1,
                },
                cursor,
            );
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

impl AppComponent<Msg, UserEvent> for SettingsComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(message)).into_option(),
                None if self.destination == SettingsDestination::Keys
                    && matches!(key.code, Key::PageUp | Key::PageDown | Key::Home | Key::End) =>
                {
                    // The Keys destination consumed the chord as a local
                    // scroll move; claim it like the cursor moves.
                    LeafKeyResult::Consumed(None).into_option()
                }
                None if matches!(
                    key.code,
                    Key::Up
                        | Key::Down
                        | Key::Tab
                        | Key::BackTab
                        | Key::Enter
                        | Key::Backspace
                        | Key::Esc
                        | Key::Char(_)
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
    use ratatui::Terminal;
    use tuirealm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn mouse_down(column: u16, row: u16) -> Event<UserEvent> {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn painted_settings(destination: SettingsDestination) -> SettingsComponent {
        let mut component = SettingsComponent::new();
        let (rows, services): (Vec<SettingsRow>, Vec<ServiceRow>) = match destination {
            SettingsDestination::Services => (
                Vec::new(),
                vec![
                    ServiceRow {
                        name: "Emby".into(),
                        detail: "Connected".into(),
                        muted: false,
                    },
                    ServiceRow {
                        name: "Audiobookshelf".into(),
                        detail: "Not configured".into(),
                        muted: false,
                    },
                ],
            ),
            _ => (
                vec![
                    SettingsRow {
                        label: "Section".into(),
                        value: String::new(),
                        section: true,
                        cursor: None,
                    },
                    SettingsRow {
                        label: "Stay alive".into(),
                        value: "off".into(),
                        section: false,
                        cursor: Some(0),
                    },
                ],
                Vec::new(),
            ),
        };
        component.set_content(SettingsSnapshot {
            destination,
            rows,
            services,
            keys: Vec::new(),
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        component
    }

    #[test]
    fn settings_mouse_click_selects_and_activates_a_row() {
        let mut component = painted_settings(SettingsDestination::Main);
        let (rect, cursor) = component.test_rows().regions()[0];
        assert!(matches!(
            component.on(&mouse_down(rect.x, rect.y)),
            Some(Msg::Shell(ShellRequest::SettingsIntent(
                SettingsIntent::Activate(c)
            ))) if c == cursor
        ));
        assert_eq!(component.cursor, cursor);
    }

    #[test]
    fn settings_mouse_click_on_a_service_selects_and_activates_it() {
        let mut component = painted_settings(SettingsDestination::Services);
        let (rect, cursor) = component.test_rows().regions()[0];
        assert_eq!(
            component.on(&mouse_down(rect.x, rect.y)),
            Some(Msg::Service(ServiceRequest::ActivateService(cursor)))
        );
        assert_eq!(component.services_cursor, cursor);
    }

    #[test]
    fn settings_mouse_click_outside_the_painted_panel_dismisses() {
        let mut component = painted_settings(SettingsDestination::Main);
        let (x, width) = (
            component.geometry.panel_area.x,
            component.geometry.panel_area.width,
        );
        assert_eq!(
            component.on(&mouse_down(x + width + 5, 1)),
            Some(Msg::Shell(ShellRequest::DismissSettings))
        );
    }

    #[test]
    fn settings_mouse_wheel_moves_one_line_inside_content() {
        let mut component = painted_settings(SettingsDestination::Main);
        component.geometry.content_area = Rect::new(0, 0, 40, 12);
        component.geometry.cursor_lines = vec![0, 20];
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.scroll, 1);
    }

    #[test]
    fn settings_mouse_wheel_moves_off_panel_content() {
        let mut component = painted_settings(SettingsDestination::Main);
        component.geometry.panel_area = Rect::new(2, 2, 10, 4);
        component.geometry.content_area = Rect::new(2, 2, 10, 4);
        component.geometry.cursor_lines = vec![0, 20];
        component.scroll = 2;
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.scroll, 3);
    }

    #[test]
    fn setup_edits_are_local_and_submit_is_typed_service_request() {
        let mut component = SettingsComponent::new();
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Services,
            rows: Vec::new(),
            services: vec![ServiceRow {
                name: "Emby".into(),
                detail: "Not configured".into(),
                muted: false,
            }],
            keys: Vec::new(),
            setup: Some(SetupDraft::Emby {
                fields: ["https://server".into(), "user".into(), String::new()],
                focus: 2,
                busy: false,
                error: String::new(),
            }),
            area: Rect::new(0, 0, 40, 12),
        });
        component.on(&key(Key::Char('x')));
        assert!(matches!(
            component.on(&key(Key::Enter)),
            Some(Msg::Service(ServiceRequest::SubmitEmbySetup { password, .. }))
                if password == "x"
        ));
    }

    #[test]
    fn settings_renders_without_app_state() {
        let mut component = SettingsComponent::new();
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Main,
            rows: vec![SettingsRow {
                label: "Stay alive".into(),
                value: "off".into(),
                section: false,
                cursor: Some(0),
            }],
            services: Vec::new(),
            keys: Vec::new(),
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("SETTINGS"));
        assert!(output.contains("Stay alive"));
    }

    /// The Keys destination paints through the main list's row painter:
    /// group headers, one row per action with its chord column, the KEYS
    /// shell, and an override rendering its configured chord (task 7.1,
    /// design D7).
    #[test]
    fn keys_destination_paints_groups_actions_and_the_configured_chord() {
        let keys = vec![
            SettingsRow {
                label: "Playback".into(),
                value: String::new(),
                section: true,
                cursor: None,
            },
            SettingsRow {
                label: "toggle_play_pause".into(),
                value: "k".into(),
                section: false,
                cursor: Some(0),
            },
            SettingsRow {
                label: "stop".into(),
                value: "Esc".into(),
                section: false,
                cursor: Some(1),
            },
        ];
        let mut component = SettingsComponent::new();
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Keys,
            rows: Vec::new(),
            services: Vec::new(),
            keys,
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("KEYS"));
        assert!(output.contains("Playback"), "group header painted");
        assert!(output.contains("toggle_play_pause"), "action row painted");
        assert!(
            output.contains("k"),
            "the override's configured chord paints"
        );
        assert!(output.contains("Esc"), "the default chord paints");
    }

    /// Keys is a read-only destination (design D7 / ADR 0023): the arrows
    /// move the local cursor, Enter/Space select nothing, and Back returns
    /// to the main list and zeroes the cursor for the next entry.
    #[test]
    fn keys_destination_is_read_only_and_back_resets_the_cursor() {
        let keys = vec![
            SettingsRow {
                label: "a_first".into(),
                value: "F1".into(),
                section: false,
                cursor: Some(0),
            },
            SettingsRow {
                label: "b_second".into(),
                value: "F2".into(),
                section: false,
                cursor: Some(1),
            },
        ];
        let mut component = SettingsComponent::new();
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Keys,
            rows: Vec::new(),
            services: Vec::new(),
            keys,
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        // Down moves the local cursor; no shell intent is emitted.
        assert!(matches!(
            component.on(&key(Key::Down)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        assert_eq!(component.keys_cursor, 1);
        // Enter/Space select nothing (read-only) — no Activate intent.
        assert!(!matches!(
            component.on(&key(Key::Enter)),
            Some(Msg::Shell(ShellRequest::SettingsIntent(_)))
        ));
        assert!(matches!(
            component.on(&key(Key::Enter)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        // Back returns to the main list and zeroes the cursor.
        assert!(matches!(
            component.on(&key(Key::Esc)),
            Some(Msg::Shell(ShellRequest::SettingsIntent(
                SettingsIntent::Back
            )))
        ));
        assert_eq!(component.keys_cursor, 0);
    }

    /// Keys cursor and scroll fixture: one group header plus `actions`
    /// action rows, painted once so the render geometry (cursor lines,
    /// content area) is live.
    fn painted_keys_content(actions: usize) -> SettingsComponent {
        let mut keys = vec![SettingsRow {
            label: "Playback".into(),
            value: String::new(),
            section: true,
            cursor: None,
        }];
        keys.extend((0..actions).map(|i| SettingsRow {
            label: format!("action_{i}"),
            value: "k".into(),
            section: false,
            cursor: Some(i),
        }));
        let mut component = SettingsComponent::new();
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Keys,
            rows: Vec::new(),
            services: Vec::new(),
            keys,
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        component
    }

    /// The Keys cursor is an ordinal over action rows (headers are not
    /// addressable): it clamps to the action count on both ends, and a
    /// stale cursor value pushed in with content clamps to the last action
    /// instead of highlighting a header or nothing.
    #[test]
    fn keys_cursor_numbers_actions_and_clamps_to_the_action_count() {
        let mut component = painted_keys_content(2);
        for _ in 0..5 {
            assert!(matches!(
                component.on(&key(Key::Down)),
                Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
            ));
        }
        assert_eq!(component.keys_cursor, 1, "Down clamps to the last action");
        for _ in 0..5 {
            component.on(&key(Key::Up));
        }
        assert_eq!(component.keys_cursor, 0, "Up clamps at the first action");

        // A stale cursor larger than the action count clamps on the next
        // content push (headers carry no cursor, so they are never
        // highlighted).
        component.keys_cursor = 7;
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Keys,
            rows: Vec::new(),
            services: Vec::new(),
            keys: component.keys.clone(),
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        assert_eq!(component.keys_cursor, 1);
    }

    /// Arrow moves scroll the Keys window so the cursor's row stays
    /// painted: ~40 rows overflow a 12-row panel, so moving the cursor off
    /// the bottom pulls the scroll down, and moving back up restores it.
    #[test]
    fn keys_arrow_moves_scroll_the_cursor_into_view() {
        let mut component = painted_keys_content(40);
        let height = component.geometry.content_area.height as usize;
        assert!(
            component.geometry.cursor_lines.len() > height,
            "fixture overflows the viewport"
        );
        for _ in 0..40 {
            component.on(&key(Key::Down));
        }
        assert_eq!(component.keys_cursor, 39);
        let line = component.geometry.cursor_lines[39];
        assert!(
            line >= component.scroll && line < component.scroll + height,
            "cursor row must be inside the scrolled window after Down"
        );
        assert!(
            component.scroll > 0,
            "the window scrolled to reach the last row"
        );
        for _ in 0..40 {
            component.on(&key(Key::Up));
        }
        assert_eq!(component.keys_cursor, 0);
        let top = component.geometry.cursor_lines[0];
        assert!(
            top >= component.scroll && top < component.scroll + height,
            "the first action row is inside the scrolled window again"
        );
    }

    /// PageUp/PageDown/Home/End scroll the Keys window (claimed, not
    /// fallen through to the router), clamped to the document.
    #[test]
    fn keys_page_and_edge_keys_scroll_and_are_claimed() {
        let mut component = painted_keys_content(40);
        let height = component.geometry.content_area.height as usize;
        let max_scroll = component
            .geometry
            .cursor_lines
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_sub(height - 1);
        assert!(matches!(
            component.on(&key(Key::PageDown)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        assert_eq!(component.scroll, 10.min(max_scroll));
        assert!(matches!(
            component.on(&key(Key::End)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        assert_eq!(component.scroll, max_scroll);
        assert!(matches!(
            component.on(&key(Key::Home)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        assert_eq!(component.scroll, 0);
        assert!(matches!(
            component.on(&key(Key::PageUp)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        assert_eq!(component.scroll, 0, "PageUp clamps at the top");
    }

    /// Scrolling the Keys window moves the painted rows (buffer evidence:
    /// the rows above the window scroll off, later rows paint in their
    /// place).
    #[test]
    fn keys_scroll_moves_the_painted_window() {
        let mut component = painted_keys_content(40);
        component.on(&key(Key::PageDown));
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(
            !output.contains("action_0"),
            "rows above the window scrolled off"
        );
        assert!(
            output.contains("action_15"),
            "rows below the window painted in"
        );
    }

    #[test]
    fn back_from_services_zeroes_the_local_services_cursor() {
        let mut component = SettingsComponent::new();
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Services,
            rows: Vec::new(),
            services: vec![
                ServiceRow {
                    name: "Emby".into(),
                    detail: "Not configured".into(),
                    muted: false,
                },
                ServiceRow {
                    name: "Audiobookshelf".into(),
                    detail: "Not configured".into(),
                    muted: false,
                },
            ],
            keys: Vec::new(),
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        // Move the Services cursor off the top row (Down is a local cursor
        // move and emits the framework-local claim marker).
        assert!(matches!(
            component.on(&key(Key::Down)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        // Leaving Services via Back zeroes the component's own cursor, so the
        // next Services entry starts back at the top instead of remembering
        // the old position.
        assert!(matches!(
            component.on(&key(Key::Esc)),
            Some(Msg::Shell(ShellRequest::SettingsIntent(
                SettingsIntent::Back
            )))
        ));
        component.set_content(SettingsSnapshot {
            destination: SettingsDestination::Services,
            rows: Vec::new(),
            services: vec![
                ServiceRow {
                    name: "Emby".into(),
                    detail: "Not configured".into(),
                    muted: false,
                },
                ServiceRow {
                    name: "Audiobookshelf".into(),
                    detail: "Not configured".into(),
                    muted: false,
                },
            ],
            keys: Vec::new(),
            setup: None,
            area: Rect::new(0, 0, 40, 12),
        });
        // Effective local cursor after re-entry: 0 (the fix), not 1 (the
        // stale position the component would otherwise carry into the next
        // entry). The re-entry push itself carries no cursor value.
        assert!(matches!(
            component.on(&key(Key::Enter)),
            Some(Msg::Service(ServiceRequest::ActivateService(0)))
        ));
    }
}
