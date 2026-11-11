//! The Settings screen's section loads and the F2-row commands
//! (`add-gui-settings-screen` design D9, rows 5.1): the choice lists and
//! library routes a section shows, loaded when the section opens and cached
//! until the screen closes, and the three commands that edit an F2 row —
//! `activate`, `applyChoices`, and `assignRoute`.

use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::settings::{SettingKey, SettingScope};

use super::projection::{
    GuiSettingsChoiceProjection, GuiSettingsRouteDeviceProjection, gui_settings_values,
};
use super::{GuiSettingsRouteProjection, GuiSettingsSection};
use crate::app::App;

impl App {
    /// Start the open section's load (design D9): the section's data lands in
    /// the screen state's caches, which live until the screen closes. A
    /// section with no load of its own changes nothing.
    pub(super) fn load_gui_settings_section(&mut self, section: GuiSettingsSection) {
        match section {
            // The Library section lists the routable libraries with their
            // assigned devices, and its two choice lists.
            GuiSettingsSection::Library => {
                let routes = self.route_libraries().map(|rows| {
                    rows.into_iter()
                        .map(GuiSettingsRouteProjection::from)
                        .collect()
                });
                let hidden = self.gui_settings_choice_rows(MultiSelectKind::HiddenLibraries);
                let feed_view = self.gui_settings_choice_rows(MultiSelectKind::FeedViewLibraries);
                let Some(state) = self.gui_settings.as_mut() else {
                    return;
                };
                state.routes = routes;
                for (kind, rows) in [
                    (MultiSelectKind::HiddenLibraries, hidden),
                    (MultiSelectKind::FeedViewLibraries, feed_view),
                ] {
                    if let Some(rows) = rows {
                        state.set_choices(kind, rows);
                    }
                }
            }
            // The Playback section's My Languages list needs no Emby client.
            GuiSettingsSection::Playback => {
                if let Some(rows) = self.gui_settings_choice_rows(MultiSelectKind::MyLanguages)
                    && let Some(state) = self.gui_settings.as_mut()
                {
                    state.set_choices(MultiSelectKind::MyLanguages, rows);
                }
            }
            // Playlists lists the saved playlists as it opens (design D9).
            GuiSettingsSection::Playlists => self.spawn_load_playlists(),
            GuiSettingsSection::Services
            | GuiSettingsSection::Display
            | GuiSettingsSection::Session
            | GuiSettingsSection::Queue
            | GuiSettingsSection::Mpv
            | GuiSettingsSection::Feeds
            | GuiSettingsSection::Actions
            | GuiSettingsSection::Keys
            | GuiSettingsSection::Sessions => {}
        }
    }

    /// One choice list as the frame's rows carry it (design D9), through the
    /// shared `App::settings_choices` build the TUI's multiselect uses. `None`
    /// when the kind has no Emby client to enumerate.
    fn gui_settings_choice_rows(
        &self,
        kind: MultiSelectKind,
    ) -> Option<Vec<GuiSettingsChoiceProjection>> {
        self.settings_choices(kind).map(|rows| {
            rows.into_iter()
                .map(|(id, label, selected)| GuiSettingsChoiceProjection {
                    id,
                    label,
                    selected,
                })
                .collect()
        })
    }

    /// Re-load one choice list after a commit, so the rows paint the saved
    /// state (design D9's cache).
    fn reload_gui_settings_choices(&mut self, kind: MultiSelectKind) {
        let Some(rows) = self.gui_settings_choice_rows(kind) else {
            return;
        };
        if let Some(state) = self.gui_settings.as_mut() {
            state.set_choices(kind, rows);
        }
    }

    /// Row 5.1's `activate` arm (design D5): activate one F2 row through
    /// `App::handle_settings_activate`. A key whose scope is not `AllClients`
    /// is refused, because the TUI client scope or an inert row has no GUI
    /// effect (design D4). A command whose screen is closed changes nothing.
    pub(super) fn apply_gui_settings_activate(&mut self, setting: SettingKey) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("activate", "the Settings screen is closed");
            return;
        }
        if setting.scope() != SettingScope::AllClients {
            Self::log_ignored_gui_settings_command(
                "activate",
                format_args!("`{}` is not an all-clients setting", setting.name()),
            );
            return;
        }
        self.handle_settings_activate(setting);
    }

    /// Row 9.2's `setValue` arm (design D5, design D13): pick one value for a
    /// value setting. The key's scope must be `AllClients`, and the value must
    /// be one the row's projected list carries, so a command built from a
    /// stale frame — or a value the list does not offer — changes nothing.
    pub(super) fn apply_gui_settings_set_value(&mut self, setting: SettingKey, value: &str) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("setValue", "the Settings screen is closed");
            return;
        }
        if setting.scope() != SettingScope::AllClients {
            Self::log_ignored_gui_settings_command(
                "setValue",
                format_args!("`{}` is not an all-clients setting", setting.name()),
            );
            return;
        }
        let my_languages = self.config.lock().unwrap().my_languages.clone();
        let listed = gui_settings_values(setting, &my_languages)
            .is_some_and(|values| values.iter().any(|listed| listed == value));
        if !listed {
            Self::log_ignored_gui_settings_command(
                "setValue",
                format_args!("`{value}` is not a value `{}` lists", setting.name()),
            );
            return;
        }
        self.set_settings_value(setting, value);
    }

    /// Row 5.1's `applyChoices` arm (design D5, design D7): run the shared
    /// multiselect commit for `kind` with `selected` as the marked ids, then
    /// re-load the list so its rows paint the saved state. A library kind's
    /// commit returns the refetched Home content, which becomes the Home store
    /// the GUI Library reads.
    pub(super) fn apply_gui_settings_choices(
        &mut self,
        kind: MultiSelectKind,
        selected: &[String],
    ) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("applyChoices", "the Settings screen is closed");
            return;
        }
        let Some(rows) = self.settings_choices(kind) else {
            Self::log_ignored_gui_settings_command(
                "applyChoices",
                "the kind has no library list to mark",
            );
            return;
        };
        let marked: Vec<(String, String, bool)> = rows
            .into_iter()
            .map(|(id, label, _)| {
                let marked = selected.contains(&id);
                (id, label, marked)
            })
            .collect();
        if let Some(content) = self.commit_settings_choices(kind, &marked) {
            self.apply_home_content(Some(content));
        }
        self.reload_gui_settings_choices(kind);
    }

    /// Row 5.1's `assignRoute` arm (design D5, design D7): write the chosen
    /// device route for `library`, or clear the route for no endpoint, through
    /// the shared `App::assign_library_route`, then re-cache the refreshed
    /// library rows the section paints.
    pub(super) fn apply_gui_settings_route(&mut self, library: &str, endpoint: Option<String>) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("assignRoute", "the Settings screen is closed");
            return;
        }
        let library = library.to_lowercase();
        // The picked device row names the library's display name, which the
        // shared refusal prints; the loaded route rows hold it.
        let name = self
            .gui_settings
            .as_ref()
            .and_then(|state| state.routes.as_ref())
            .and_then(|rows| rows.iter().find(|row| row.library == library))
            .map_or_else(|| library.clone(), |row| row.name.clone());
        let device = endpoint.map(|endpoint| (name, Some(endpoint)));
        let refreshed = self.assign_library_route(&library, device.as_ref());
        let Some((rows, _cursor)) = refreshed else {
            return;
        };
        let rows: Vec<GuiSettingsRouteProjection> = rows
            .into_iter()
            .map(GuiSettingsRouteProjection::from)
            .collect();
        if let Some(state) = self.gui_settings.as_mut() {
            state.routes = Some(rows);
        }
    }

    /// Row 5.1's `routeDevices` arm (design D5, design D9): load the candidate
    /// devices of `library` through the shared `App::route_devices`, and cache
    /// them for the Library section's routes list until the screen closes. The
    /// library must be one the section's loaded route rows list, so a command
    /// built from a stale frame changes nothing.
    pub(super) fn apply_gui_settings_route_devices(&mut self, library: &str) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("routeDevices", "the Settings screen is closed");
            return;
        }
        let library = library.to_lowercase();
        let loaded = self
            .gui_settings
            .as_ref()
            .and_then(|state| state.routes.as_ref())
            .is_some_and(|rows| rows.iter().any(|row| row.library == library));
        if !loaded {
            Self::log_ignored_gui_settings_command(
                "routeDevices",
                format_args!("`{library}` is not a loaded route library"),
            );
            return;
        }
        let devices = self.route_devices(&library).map(|(devices, _cursor)| {
            devices
                .into_iter()
                .map(GuiSettingsRouteDeviceProjection::from)
                .collect::<Vec<_>>()
        });
        if let Some(state) = self.gui_settings.as_mut() {
            state.route_selection = Some(library);
            state.route_devices = devices;
        }
    }
}
