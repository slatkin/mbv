//! The Settings, Service, feed, and artwork-mutator facts a Client changes.
//!
//! A service request and a Settings intent already reach the core through its
//! own entry points; this module adds the writes those paths left to the
//! shell: which Settings pane a service action shows, the values a submitted
//! setup form carries, and the feed-manage popup's background channel and
//! in-flight marker.

use crate::app::App;
use mbv_ui_model::feeds_manage::FeedsManagePopup;
use mbv_ui_model::settings::SettingsDestination;

impl App {
    /// Show the Services pane of Settings: what a service action selected in
    /// the Services list shows before it runs.
    pub fn show_services_settings(&mut self) {
        self.settings_destination = SettingsDestination::Services;
    }

    /// Return Settings to its main pane. A Back step inside a Services, Keys,
    /// or Panel pane does this; any other destination closes Settings.
    /// Answers whether the Back step stayed inside Settings.
    pub fn show_main_settings_pane(&mut self) -> bool {
        if matches!(
            self.settings_destination,
            SettingsDestination::Services | SettingsDestination::Keys | SettingsDestination::Panel
        ) {
            self.settings_destination = SettingsDestination::Main;
            return true;
        }
        false
    }

    /// Hold the fields a submitted Emby setup form carries. An open form keeps
    /// its generation, so a resubmit while one runs is refused by
    /// [`App::submit_emby_setup`] instead of orphaning it; a submit with no
    /// open form leaves the setup untouched.
    pub fn set_emby_setup_form_fields(&mut self, fields: [String; 3]) {
        if let Some(form) = self.setup.emby_setup_form.as_mut() {
            form.fields = fields;
            form.focus = 2;
        }
    }

    /// Hold the fields a submitted Audiobookshelf setup form carries, by the
    /// rule of [`App::set_emby_setup_form_fields`].
    pub fn set_audiobookshelf_setup_form_fields(&mut self, fields: [String; 2]) {
        if let Some(form) = self.setup.audiobookshelf_setup_form.as_mut() {
            form.fields = fields;
            form.focus = 1;
        }
    }

    /// Open the feed-manage popup's background add-feed channel, in-flight add
    /// marker, and add-attempt counter (add-gui-settings-screen design D7).
    /// Its mpsc receiver drops on close, discarding any in-flight result.
    pub fn open_feeds_manage_channel(&mut self) {
        if self.feeds_manage.is_none() {
            self.feeds_manage = Some(FeedsManagePopup::new());
        }
    }

    /// Drop the feed-manage popup's background channel and its markers: the
    /// surface closed, so a fetch still in flight is discarded.
    pub fn close_feeds_manage_channel(&mut self) {
        self.feeds_manage = None;
    }

    /// Forget the in-flight add-feed marker (§6.2). The fetch's own result then
    /// finds no pending id to match, so a later answer is dropped as stale by
    /// `drain_feed_add_results`.
    pub fn cancel_feed_add(&mut self) {
        if let Some(popup) = &mut self.feeds_manage {
            popup.pending_add = None;
        }
    }
}
