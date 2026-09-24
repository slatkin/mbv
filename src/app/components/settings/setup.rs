use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::{SettingsComponent, SetupDraft};
use crate::app::components::msg::{Msg, ServiceRequest};

impl SettingsComponent {
    pub(super) fn service_key(&self, key: &KeyEvent) -> Option<Msg> {
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

    pub(super) fn setup_key(&mut self, key: &KeyEvent) -> Option<Msg> {
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
}
