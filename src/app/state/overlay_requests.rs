//! The sidebar and modal shell-request helpers (the `pending_overlay`
//! writers): the App-side half of raising a sidebar or modal for the
//! Client's shell to mount.

use super::app_struct::App;
use crate::app::gui_client_state::GuiOverlay;
use crate::app::{ConfirmModal, SavePlaylistDialog};

impl App {
    /// Take the pending shell request (`add-gui-settings-screen` design D2):
    /// the GUI core thread's drain reads it once per tick, after input
    /// routing. The TUI shell drains it through `sync_modal_requests`
    /// instead.
    pub fn take_pending_overlay(&mut self) -> Option<mbv_ui_model::overlay::OverlayRequest> {
        self.pending_overlay.take()
    }

    pub(in crate::app) fn request_sidebar_open(
        &mut self,
        sidebar: mbv_ui_model::overlay::SidebarId,
    ) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::OpenSidebar(sidebar));
    }

    pub(in crate::app) fn request_sidebar_dismiss(
        &mut self,
        sidebar: mbv_ui_model::overlay::SidebarId,
    ) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissSidebar(
            sidebar,
        ));
    }

    pub(in crate::app) fn request_sidebar_toggle(
        &mut self,
        sidebar: mbv_ui_model::overlay::SidebarId,
    ) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::ToggleSidebar(
            sidebar,
        ));
    }

    pub(in crate::app) fn ask_confirm(&mut self, modal: ConfirmModal) {
        // The GUI Client presents the shared confirmation as its own blocking
        // overlay (the canvas's modal); the shell request stays undrained —
        // the GUI mounts no TUI modal component.
        if self.gui_client {
            self.gui_overlay = Some(GuiOverlay::Confirm(modal.clone()));
        }
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal));
    }

    pub(in crate::app) fn open_save_playlist_dialog(&mut self, dialog: SavePlaylistDialog) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::SavePlaylist(dialog));
    }

    pub(in crate::app) fn dismiss_confirm(&mut self) {
        if self.gui_client && matches!(self.gui_overlay, Some(GuiOverlay::Confirm(_))) {
            self.gui_overlay = None;
        }
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissConfirm);
    }

    pub(in crate::app) fn dismiss_daemon_lost(&mut self) {
        // The GUI Client presents the dialog as its own blocking overlay and
        // mounts no TUI modal component, so the dismissal clears that held
        // overlay; the shell request stays undrained, as the confirmation's
        // does. A successful restart reaches here from
        // `restart_local_daemon`.
        if self.gui_client && matches!(self.gui_overlay, Some(GuiOverlay::DaemonLost(_))) {
            self.gui_overlay = None;
        }
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissDaemonLost);
    }

    pub(in crate::app) fn dismiss_save_playlist(&mut self) {
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DismissSavePlaylist);
    }
}
