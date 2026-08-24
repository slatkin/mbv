use super::components::{
    ComponentId, PopupId, ServiceRequest, ServiceRow, SettingsComponent, SettingsRow,
    SettingsSnapshot, SetupDraft,
};
use super::shell::Model;
use super::types_settings::{SettingsDestination, SERVICE_ENTRIES, SETTING_SECTIONS};
use super::{settings, SETTINGS_PANEL_W};
use ratatui::layout::Rect;

impl Model {
    pub(super) fn update_settings_content(&mut self) {
        let id = ComponentId::Overlay(super::components::OverlayId::Settings);
        if !self.application.mounted(&id) {
            return;
        }
        let child_open = self
            .application
            .mounted(&ComponentId::Popup(PopupId::Multiselect))
            || self
                .application
                .mounted(&ComponentId::Popup(PopupId::LibraryRoutes))
            || self
                .application
                .mounted(&ComponentId::Popup(PopupId::FeedManage));
        if !child_open && self.application.focus() != Some(&id) {
            self.application.active(&id).expect("activate Settings");
        }

        let snapshot = self.settings_snapshot();
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(settings) = comp.as_any_mut().downcast_mut::<SettingsComponent>() {
                settings.set_content(snapshot);
            }
        }
    }

    pub(super) fn render_settings_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Overlay(super::components::OverlayId::Settings);
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, frame, frame.area());
    }

    fn settings_snapshot(&self) -> SettingsSnapshot {
        let cfg = self.app.config.lock().unwrap().clone();
        let ui = self.app.ui_config_snapshot();
        let mut rows = Vec::new();
        let mut cursor = 0;
        for (section, keys) in &SETTING_SECTIONS[..SETTING_SECTIONS.len() - 1] {
            rows.push(SettingsRow {
                label: (*section).into(),
                value: String::new(),
                section: true,
                cursor: None,
            });
            for &key in *keys {
                rows.push(SettingsRow {
                    label: settings::setting_label(key).into(),
                    value: settings::setting_value(key, &cfg, &ui),
                    section: false,
                    cursor: Some(cursor),
                });
                cursor += 1;
            }
        }
        rows.push(SettingsRow {
            label: settings::setting_label(super::types_settings::SettingKey::LogOut).into(),
            value: String::new(),
            section: false,
            cursor: Some(cursor),
        });

        let services = SERVICE_ENTRIES
            .iter()
            .map(|entry| {
                let context = self.app.service_context(*entry);
                let action = self.app.service_action_label(*entry);
                ServiceRow {
                    name: super::App::service_entry_name(*entry).into(),
                    detail: if context.is_empty() {
                        format!("{} · {}", self.app.service_state_label(*entry), action)
                    } else {
                        format!(
                            "{} · {} · {}",
                            self.app.service_state_label(*entry),
                            context,
                            action
                        )
                    },
                    muted: matches!(*entry, super::types_settings::ServiceEntry::Audiobookshelf),
                }
            })
            .collect();

        let setup =
            self.app
                .emby_setup_form
                .as_ref()
                .map(|form| SetupDraft::Emby {
                    fields: form.fields.clone(),
                    focus: form.focus,
                    busy: form.busy,
                    error: form.error.clone(),
                })
                .or_else(|| {
                    self.app.audiobookshelf_setup_form.as_ref().map(|form| {
                        SetupDraft::Audiobookshelf {
                            fields: form.fields.clone(),
                            focus: form.focus,
                            busy: form.busy,
                            error: form.error.clone(),
                        }
                    })
                });
        SettingsSnapshot {
            destination: self.app.settings_destination,
            rows,
            services,
            setup,
            cursor: self.app.settings_cursor,
            services_cursor: self.app.services_cursor,
            scroll: self.app.settings_scroll,
            area: (self.app.layout.main.panel_area.width > 0)
                .then_some(self.app.layout.main.panel_area)
                .unwrap_or(Rect {
                    x: 0,
                    y: 0,
                    width: SETTINGS_PANEL_W.min(self.app.terminal_width),
                    height: self.app.terminal_height,
                }),
        }
    }

    pub(super) fn handle_service_request(&mut self, request: ServiceRequest) -> bool {
        match request {
            ServiceRequest::SettingsKey { cursor, key } => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                self.app.services_cursor = cursor;
                match key.code {
                    crossterm::event::KeyCode::Enter | crossterm::event::KeyCode::Char(' ') => {
                        self.app.activate_service_entry();
                    }
                    crossterm::event::KeyCode::Char('d') | crossterm::event::KeyCode::Char('D')
                        if cursor == 0 =>
                    {
                        self.app.request_emby_removal();
                    }
                    crossterm::event::KeyCode::Char('t') | crossterm::event::KeyCode::Char('T')
                        if cursor == 1 =>
                    {
                        self.app.test_audiobookshelf_connection();
                    }
                    crossterm::event::KeyCode::Char('r') | crossterm::event::KeyCode::Char('R')
                        if cursor == 1 =>
                    {
                        self.app.route_service_action(
                            super::types_settings::ServiceActionIntent::ReplaceAudiobookshelf,
                        );
                    }
                    crossterm::event::KeyCode::Char('d') | crossterm::event::KeyCode::Char('D')
                        if cursor == 1 =>
                    {
                        self.app.route_service_action(
                            super::types_settings::ServiceActionIntent::RemoveAudiobookshelf,
                        );
                    }
                    _ => {}
                }
                false
            }
            ServiceRequest::ActivateService(cursor) => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                self.app.services_cursor = cursor;
                self.app.activate_service_entry();
                false
            }
            ServiceRequest::SubmitEmbySetup {
                server_url,
                username,
                password,
            } => {
                if let Some(form) = self.app.emby_setup_form.as_mut() {
                    form.fields = [server_url, username, password];
                    form.focus = 2;
                }
                self.app.submit_emby_setup();
                false
            }
            ServiceRequest::SubmitAudiobookshelfSetup {
                server_url,
                api_key,
            } => {
                if let Some(form) = self.app.audiobookshelf_setup_form.as_mut() {
                    form.fields = [server_url, api_key];
                    form.focus = 1;
                }
                self.app.submit_audiobookshelf_setup();
                false
            }
            ServiceRequest::CancelSetup => {
                if self.app.emby_setup_form.is_some() {
                    self.app.cancel_emby_setup();
                } else if self.app.audiobookshelf_setup_form.is_some() {
                    self.app.cancel_audiobookshelf_setup();
                }
                false
            }
            ServiceRequest::SearchQuery(query) => {
                if let Some(client) = self.app.emby_snapshot() {
                    self.app.spawn_search_sidebar_query(client, query);
                }
                false
            }
        }
    }

    pub(super) fn handle_persist_request(
        &mut self,
        request: super::components::PersistRequest,
    ) -> bool {
        let super::components::PersistRequest::SettingsKey { cursor, key } = request;
        if self.app.settings_destination == SettingsDestination::Services {
            match key.code {
                crossterm::event::KeyCode::Esc => {
                    self.app.settings_destination = SettingsDestination::Main;
                    self.app.services_cursor = 0;
                }
                crossterm::event::KeyCode::F(3) => {
                    self.app.close_settings();
                    self.mount_sidebar(super::SidebarId::Sessions);
                }
                crossterm::event::KeyCode::F(4) => {
                    self.app.close_settings();
                    self.mount_sidebar(super::SidebarId::Playlists);
                    self.app.open_playlists_panel();
                }
                crossterm::event::KeyCode::Char('q') if key.modifiers.is_empty() => {
                    return self.app.try_quit()
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            crossterm::event::KeyCode::Esc => self.app.close_settings(),
            crossterm::event::KeyCode::F(3) => {
                self.app.close_settings();
                self.mount_sidebar(super::SidebarId::Sessions);
            }
            crossterm::event::KeyCode::F(4) => {
                self.app.close_settings();
                self.mount_sidebar(super::SidebarId::Playlists);
                self.app.open_playlists_panel();
            }
            crossterm::event::KeyCode::Char('q') if key.modifiers.is_empty() => {
                return self.app.try_quit()
            }
            crossterm::event::KeyCode::Left
            | crossterm::event::KeyCode::Right
            | crossterm::event::KeyCode::Char(' ')
            | crossterm::event::KeyCode::Enter => {
                self.app.settings_cursor = cursor;
                self.app.handle_settings_activate();
            }
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::ServiceRequest;

    #[test]
    fn settings_service_request_stays_at_shell_boundary() {
        let mut app = super::super::tests::make_app_stub();
        app.open_services_settings();
        let mut model = Model::new(app);
        model.handle_service_request(ServiceRequest::ActivateService(0));
        assert!(model.app.emby_setup_form.is_some());

        model.handle_service_request(ServiceRequest::SubmitEmbySetup {
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
        });
        assert!(model
            .app
            .emby_setup_form
            .as_ref()
            .is_some_and(|form| !form.error.is_empty()));
    }
}
