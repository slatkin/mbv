use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{LegacyTerminalEvent, Msg, PersistRequest, ServiceRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_settings_content, SettingsRenderModel};
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
    pub setup: Option<SetupDraft>,
    pub cursor: usize,
    pub services_cursor: usize,
    pub scroll: usize,
    pub area: Rect,
}

pub struct SettingsComponent {
    destination: SettingsDestination,
    rows: Vec<SettingsRow>,
    services: Vec<ServiceRow>,
    setup: Option<SetupDraft>,
    cursor: usize,
    services_cursor: usize,
    scroll: usize,
    area: Rect,
    initialized: bool,
}

impl SettingsComponent {
    pub fn new() -> Self {
        Self {
            destination: SettingsDestination::Main,
            rows: Vec::new(),
            services: Vec::new(),
            setup: None,
            cursor: 0,
            services_cursor: 0,
            scroll: 0,
            area: Rect::default(),
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
        if !self.initialized || destination_changed {
            self.cursor = snapshot.cursor.min(self.rows.len().saturating_sub(1));
            self.services_cursor = snapshot
                .services_cursor
                .min(self.services.len().saturating_sub(1));
        } else {
            self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
            self.services_cursor = self
                .services_cursor
                .min(self.services.len().saturating_sub(1));
        }
        self.scroll = snapshot.scroll;
        self.area = snapshot.area;
        self.initialized = true;
    }

    fn service_key(&self, key: &KeyEvent) -> Option<Msg> {
        Some(Msg::Service(ServiceRequest::SettingsKey {
            cursor: self.services_cursor,
            key: super::legacy_input::to_crossterm_key_event(key),
        }))
    }

    fn setup_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let Some(setup) = self.setup.as_mut() else {
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        };
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
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.setup.is_some() {
            return self.setup_key(key);
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
                    Some(Msg::Persist(PersistRequest::SettingsKey {
                        cursor: self.cursor,
                        key: super::legacy_input::to_crossterm_key_event(key),
                    }))
                }
                _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
            };
        }
        match key.code {
            Key::Esc | Key::Function(3) | Key::Function(4) | Key::Char('q') => {
                Some(Msg::Persist(PersistRequest::SettingsKey {
                    cursor: self.cursor,
                    key: super::legacy_input::to_crossterm_key_event(key),
                }))
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
            Key::PageDown => {
                self.scroll += 10;
                None
            }
            Key::Left | Key::Right | Key::Char(' ') | Key::Enter => {
                Some(Msg::Persist(PersistRequest::SettingsKey {
                    cursor: self.cursor,
                    key: super::legacy_input::to_crossterm_key_event(key),
                }))
            }
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
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
        render_settings_content(
            frame,
            area,
            SettingsRenderModel {
                destination: self.destination,
                rows: &self.rows,
                services: &self.services,
                setup: self.setup.as_ref(),
                cursor: self.cursor,
                services_cursor: self.services_cursor,
                scroll: self.scroll,
            },
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

impl AppComponent<Msg, UserEvent> for SettingsComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::event::KeyModifiers;

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
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
            setup: Some(SetupDraft::Emby {
                fields: ["https://server".into(), "user".into(), String::new()],
                focus: 2,
                busy: false,
                error: String::new(),
            }),
            cursor: 0,
            services_cursor: 0,
            scroll: 0,
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
            setup: None,
            cursor: 0,
            services_cursor: 0,
            scroll: 0,
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
}
