//! The sidebar and modal shell-request helpers (the `pending_overlay`
//! writers): the App-side half of raising a sidebar or modal for the
//! Client's shell to mount.

use super::app_struct::App;
use crate::app::{ConfirmModal, SavePlaylistDialog};

impl App {
    /// Take the pending shell request (`add-gui-settings-screen` design D2):
    /// the GUI core thread's drain reads it once per tick, after input
    /// routing. The TUI shell drains it through `sync_modal_requests`
    /// instead.
    pub fn take_pending_overlay(&mut self) -> Option<mbv_ui_model::overlay::OverlayRequest> {
        self.pending_overlay.take()
    }

    pub fn request_sidebar_open(&mut self, sidebar: mbv_ui_model::overlay::SidebarId) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::OpenSidebar(sidebar));
    }

    pub fn request_sidebar_dismiss(&mut self, sidebar: mbv_ui_model::overlay::SidebarId) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissSidebar(
            sidebar,
        ));
    }

    pub fn request_sidebar_toggle(&mut self, sidebar: mbv_ui_model::overlay::SidebarId) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::ToggleSidebar(
            sidebar,
        ));
    }

    pub fn ask_confirm(&mut self, modal: ConfirmModal) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal));
    }

    pub fn open_save_playlist_dialog(&mut self, dialog: SavePlaylistDialog) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::SavePlaylist(dialog));
    }

    pub fn dismiss_confirm(&mut self) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissConfirm);
    }

    pub fn dismiss_daemon_lost(&mut self) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissDaemonLost);
    }

    pub fn dismiss_save_playlist(&mut self) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissSavePlaylist);
    }
}
