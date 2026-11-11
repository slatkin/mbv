//! The Services section's commands (`add-gui-settings-screen` design D5, row
//! 5.2): the two setup form submissions, the Service actions, and the setup
//! cancellations. Each one fills or clears the `App`'s own setup state and
//! runs the shared submit, so the GUI and the TUI validate with one code path.

use mbv_ui_model::settings::ServiceActionIntent;

use super::{GuiSettingsSection, GuiSetupService};
use crate::app::App;

impl App {
    /// Row 5.2's `embySetup` arm: fill the Emby setup form with the command's
    /// fields and submit it through the shared validation.
    pub(super) fn apply_gui_settings_emby_setup(
        &mut self,
        server_url: String,
        username: String,
        password: String,
    ) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("embySetup", "the Settings screen is closed");
            return;
        }
        self.fill_emby_setup_form(server_url, username, password);
        self.submit_emby_setup();
    }

    /// Row 5.2's `audiobookshelfSetup` arm: fill the Audiobookshelf setup form
    /// with the command's fields and submit it through the shared validation.
    pub(super) fn apply_gui_settings_audiobookshelf_setup(
        &mut self,
        server_url: String,
        api_key: String,
    ) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "audiobookshelfSetup",
                "the Settings screen is closed",
            );
            return;
        }
        self.fill_audiobookshelf_setup_form(server_url, api_key);
        self.submit_audiobookshelf_setup();
    }

    /// Row 5.2's `serviceAction` arm: run one Service action through the
    /// shared routing. `ManageFeeds` is the exception (design D5): routing it
    /// raises the TUI's `OpenFeedsManage` request, which the GUI presents
    /// nothing for, so it shows the Feeds section instead.
    pub(super) fn apply_gui_settings_service_action(&mut self, action: ServiceActionIntent) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "serviceAction",
                "the Settings screen is closed",
            );
            return;
        }
        if action == ServiceActionIntent::ManageFeeds {
            self.select_gui_settings_section(GuiSettingsSection::Feeds);
            return;
        }
        self.route_service_action(action);
    }

    /// Row 5.2's `cancelSetup` arm: abandon the named setup form, cancelling
    /// its generation as the TUI's Esc does.
    pub(super) fn apply_gui_settings_cancel_setup(&mut self, service: GuiSetupService) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("cancelSetup", "the Settings screen is closed");
            return;
        }
        match service {
            GuiSetupService::Emby => self.cancel_emby_setup(),
            GuiSetupService::Audiobookshelf => self.cancel_audiobookshelf_setup(),
        }
    }
}
