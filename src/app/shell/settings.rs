use super::components::{
    ComponentId, PopupId, ServiceRequest, ServiceRow, SettingsComponent, SettingsIntent,
    SettingsRow, SettingsSnapshot, SetupDraft,
};
use super::Model;
use crate::app::state::types::settings;
use crate::app::state::types::settings::{SettingsDestination, SERVICE_ENTRIES, SETTING_SECTIONS};
use mbv_core::keybinds::{KeybindAction, KEYBIND_ACTIONS, KEY_SECTIONS};
use ratatui::layout::Rect;
use std::fmt::Write as _;

impl Model {
    pub(in crate::app) fn update_settings_content(&mut self) {
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
        // Prefix-armed capture (design D6, task 6.1) holds focus off while
        // armed; the disarm restores what the arm displaced.
        if !child_open && self.application.focus() != Some(&id) && !self.app.prefix_armed {
            self.application.active(&id).expect("activate Settings");
        }

        let snapshot = self.settings_snapshot();
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(settings) = comp.as_any_mut().downcast_mut::<SettingsComponent>() {
                settings.set_content(snapshot);
            }
        }
    }

    pub(in crate::app) fn render_settings_overlay(&mut self, frame: &mut ratatui::Frame) {
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
            label: settings::setting_label(crate::app::state::types::settings::SettingKey::LogOut)
                .into(),
            value: String::new(),
            section: false,
            cursor: Some(cursor),
        });

        let keys = self.keys_snapshot();
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
                    muted: matches!(
                        *entry,
                        crate::app::state::types::settings::ServiceEntry::Audiobookshelf
                    ),
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
            keys,
            setup,
            area: if let Some(panel_area) = super::chrome_panels::sync_panel_area(&self.app) {
                panel_area
            } else {
                Rect {
                    x: 0,
                    y: 0,
                    width: self.app.terminal_width,
                    height: self.app.terminal_height,
                }
            },
        }
    }

    /// The Keys destination's read-only content (design D7), derived from
    /// the registry and the loaded `Keybinds`: group-header rows for every
    /// populated `KeySection` in shared-vocabulary order, then one row per
    /// declared action with its router chord(s) and, where assigned, its
    /// prefix chord. Groups and rows cannot drift from routing — both are
    /// projections of `KEYBIND_ACTIONS`.
    fn keys_snapshot(&self) -> Vec<SettingsRow> {
        let mut rows = Vec::new();
        let mut cursor = 0usize;
        for section in KEY_SECTIONS {
            let actions: Vec<&KeybindAction> = KEYBIND_ACTIONS
                .iter()
                .filter(|action| action.section == *section)
                .collect();
            // A section with no declared action contributes no group (spec:
            // a section with no configured bindings is absent).
            if actions.is_empty() {
                continue;
            }
            rows.push(SettingsRow {
                label: section.name().into(),
                value: String::new(),
                section: true,
                cursor: None,
            });
            for action in actions {
                let mut value = self
                    .keybinds
                    .router_chords(action)
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" / ");
                if let Some(prefix) = self.keybinds.prefix_assignment(action.id) {
                    let _ = write!(value, " · pfx {prefix}");
                }
                rows.push(SettingsRow {
                    label: action.id.into(),
                    value,
                    section: false,
                    cursor: Some(cursor),
                });
                cursor += 1;
            }
        }
        rows
    }

    pub(in crate::app) fn handle_service_request(&mut self, request: ServiceRequest) -> bool {
        match request {
            ServiceRequest::RemoveEmby => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                self.app.request_emby_removal();
                false
            }
            ServiceRequest::TestAudiobookshelfConnection => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                self.app.test_audiobookshelf_connection();
                false
            }
            ServiceRequest::ReplaceAudiobookshelf => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                self.app.route_service_action(
                    crate::app::state::types::settings::ServiceActionIntent::ReplaceAudiobookshelf,
                );
                false
            }
            ServiceRequest::RemoveAudiobookshelf => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                self.app.route_service_action(
                    crate::app::state::types::settings::ServiceActionIntent::RemoveAudiobookshelf,
                );
                false
            }
            ServiceRequest::ActivateService(cursor) => {
                self.mount_sidebar(super::SidebarId::Settings);
                self.app.settings_destination = SettingsDestination::Services;
                if let Some(&entry) = SERVICE_ENTRIES.get(cursor) {
                    self.app.activate_service_entry(entry);
                }
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

    pub(in crate::app) fn handle_settings_intent(&mut self, intent: SettingsIntent) -> bool {
        match intent {
            SettingsIntent::Back => {
                if matches!(
                    self.app.settings_destination,
                    SettingsDestination::Services | SettingsDestination::Keys
                ) {
                    self.app.settings_destination = SettingsDestination::Main;
                } else {
                    self.app.close_settings();
                }
                false
            }
            SettingsIntent::OpenSessions => {
                self.app.close_settings();
                self.mount_sidebar(super::SidebarId::Sessions);
                false
            }
            SettingsIntent::OpenPlaylists => {
                self.app.close_settings();
                self.mount_sidebar(super::SidebarId::Playlists);
                self.app.open_playlists_panel();
                false
            }
            SettingsIntent::Quit => self.app.try_quit(),
            SettingsIntent::Activate(cursor) => {
                self.app.handle_settings_activate(
                    crate::app::state::types::settings::settings_cursor_to_key(cursor),
                );
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::keybinds::{Chord, KeySection, SectionBindings};

    #[test]
    fn settings_service_request_stays_at_shell_boundary() {
        let mut app = crate::app::tests::make_app_stub();
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

    /// The settings main list's Keys row summarizes the live configuration
    /// (task 7.2, design D7): the configured prefix plus the number of
    /// actions whose router binding deviates from the declared default.
    #[test]
    fn keys_row_summary_follows_the_loaded_configuration() {
        use crate::app::state::types::settings;
        use crate::app::SettingKey;
        let app = crate::app::tests::make_app_stub();
        let cfg = app.config.lock().unwrap().clone();
        let ui = app.ui_config_snapshot();
        assert_eq!(
            settings::setting_label(SettingKey::Keys),
            "Keys",
            "the Keys row is present in the settings label table"
        );
        assert_eq!(
            settings::setting_value(SettingKey::Keys, &cfg, &ui),
            "defaults",
            "no [keys] section: the summary reports the declared defaults"
        );

        let mut configured = cfg.clone();
        configured.keybinds.prefix = Some(Chord::parse("Ctrl+k").unwrap());
        configured.keybinds.sections = vec![(
            KeySection::Playback,
            SectionBindings {
                router: vec![
                    ("toggle_play_pause", Chord::parse("k").unwrap()),
                    ("stop", Chord::parse("s").unwrap()),
                ],
                prefix: vec![],
            },
        )];
        assert_eq!(
            settings::setting_value(SettingKey::Keys, &configured, &ui),
            "Ctrl+k \u{b7} 2 overridden"
        );
    }

    /// The Keys destination's content is a projection of the registry and
    /// the loaded `Keybinds` (task 7.1, design D7): groups equal the shared
    /// section list (populated `KeySection`s in order), action rows equal
    /// the registry set, and an override renders the configured chord.
    #[test]
    fn keys_destination_lists_registry_groups_rows_and_overrides() {
        let mut app = crate::app::tests::make_app_stub();
        app.settings_destination = SettingsDestination::Keys;
        let mut model = Model::new(app);
        model.keybinds = mbv_core::keybinds::Keybinds {
            prefix: Some(Chord::parse("Ctrl+k").unwrap()),
            sections: vec![(
                KeySection::Playback,
                SectionBindings {
                    router: vec![("toggle_play_pause", Chord::parse("k").unwrap())],
                    prefix: vec![("volume_up", Chord::parse("g").unwrap())],
                },
            )],
        };

        let snapshot = model.settings_snapshot();
        let groups: Vec<&str> = snapshot
            .keys
            .iter()
            .filter(|row| row.section)
            .map(|row| row.label.as_str())
            .collect();
        let expected_groups: Vec<&str> = KEY_SECTIONS
            .iter()
            .filter(|section| KEYBIND_ACTIONS.iter().any(|a| a.section == **section))
            .map(|section| section.name())
            .collect();
        assert_eq!(
            groups, expected_groups,
            "groups equal the shared section list"
        );

        let action_rows: Vec<&SettingsRow> =
            snapshot.keys.iter().filter(|row| !row.section).collect();
        let mut ids: Vec<&str> = action_rows.iter().map(|row| row.label.as_str()).collect();
        let mut expected_ids: Vec<&str> = KEYBIND_ACTIONS.iter().map(|a| a.id).collect();
        ids.sort_unstable();
        expected_ids.sort_unstable();
        assert_eq!(ids, expected_ids, "rows equal the registry set");
        // Every action row is cursor-activatable for selection (read-only:
        // no activation follows).
        assert!(action_rows
            .iter()
            .all(|row| row.cursor.is_some() && !row.value.is_empty()));

        let toggle = action_rows
            .iter()
            .find(|row| row.label == "toggle_play_pause")
            .expect("declared action is listed");
        assert_eq!(
            toggle.value, "k",
            "an override renders the configured chord"
        );
        let volume = action_rows
            .iter()
            .find(|row| row.label == "volume_up")
            .expect("declared action is listed");
        assert_eq!(
            volume.value, "+ / = \u{b7} pfx g",
            "a prefix assignment is shown beside the router chords"
        );
        let stop = action_rows
            .iter()
            .find(|row| row.label == "stop")
            .expect("declared action is listed");
        assert_eq!(
            stop.value, "Esc",
            "an unconfigured action shows its default"
        );
    }
}
