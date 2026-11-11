//! The Feeds section's commands (`add-gui-settings-screen` design D5, row
//! 5.3): add, edit, and remove, each addressed by the subscription's URL — the
//! config list's only stable identity (the TUI's row index can move). The add
//! worker's channel and pending id live on the `App` (design D7), so the GUI
//! creates the popup state on its first add and reads the pending flag from
//! it.

use mbv_queue::FeedKind;
use mbv_ui_model::feeds_manage::FeedsManagePopup;

use crate::app::App;

impl App {
    /// Row 5.3's `feedAdd` arm: start the background fetch through the shared
    /// `App::begin_feed_add`, whose result the Client tick host drains
    /// (`App::drain_feed_add_results`).
    pub(super) fn apply_gui_settings_feed_add(&mut self, name: &str, url: &str, kind: FeedKind) {
        if self.feeds_manage.is_none() {
            self.feeds_manage = Some(FeedsManagePopup::new());
        }
        self.begin_feed_add(name, url, kind);
    }

    /// Row 5.3's `feedEdit` arm: change the name and kind of the subscription
    /// at `url` through the shared `App::edit_feed`. An unknown URL changes
    /// nothing.
    pub(super) fn apply_gui_settings_feed_edit(&mut self, url: &str, name: &str, kind: FeedKind) {
        self.edit_feed(url, name, kind);
    }

    /// Row 5.3's `feedRemove` arm: raise the shared remove confirmation for
    /// the subscription at `url`. An unknown URL changes nothing.
    pub(super) fn apply_gui_settings_feed_remove(&mut self, url: &str) {
        self.request_feed_removal(url);
    }
}
