use super::super::components::{
    ComponentId, ConfirmComponent, DaemonLostComponent, ModalId, OverlayId,
    RemoteReanchorComponent, SavePlaylistComponent, SelectionModalComponent,
};
use super::super::shell::Model;
use super::super::types_overlay::OverlayRequest;

impl Model {
    fn confirm_id() -> ComponentId {
        ComponentId::Modal(ModalId::Confirm)
    }

    /// Render the Confirm overlay if mounted.
    pub(in crate::app) fn render_confirm_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::confirm_id();
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, f, f.area());
    }

    // --- Daemon-lost modal --------------------------------------------------

    fn daemon_lost_id() -> ComponentId {
        ComponentId::Modal(ModalId::DaemonLost)
    }

    /// Render the DaemonLost overlay if mounted.
    pub(in crate::app) fn render_daemon_lost_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::daemon_lost_id();
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, f, f.area());
    }

    // --- Remote-reanchor popup ----------------------------------------------

    fn remote_reanchor_id() -> ComponentId {
        ComponentId::Modal(ModalId::RemoteReanchor)
    }

    /// Render the RemoteReanchor overlay if mounted.
    pub(in crate::app) fn render_remote_reanchor_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::remote_reanchor_id();
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, f, f.area());
    }

    pub(in crate::app) fn sync_modal_requests(&mut self) {
        let Some(request) = self.app.pending_overlay.take() else {
            self.assert_modal_mount_exclusive();
            self.app.blocking_overlay_active = self.blocking_overlay_active();
            return;
        };
        match request {
            OverlayRequest::Confirm(modal) => {
                self.dismiss_modal(&Self::confirm_id());
                let id = Self::confirm_id();
                self.application
                    .mount(id.clone(), Box::new(ConfirmComponent::new()), vec![])
                    .expect("mount Confirm");
                self.application.active(&id).expect("activate Confirm");
                if let Some(comp) = self.application.get_component_mut(&id) {
                    comp.as_any_mut()
                        .downcast_mut::<ConfirmComponent>()
                        .expect("Confirm component")
                        .set_modal(&modal);
                }
            }
            OverlayRequest::DaemonLost(modal) => {
                self.dismiss_blocking_modals();
                let id = Self::daemon_lost_id();
                self.application
                    .mount(id.clone(), Box::new(DaemonLostComponent::new()), vec![])
                    .expect("mount DaemonLost");
                self.application.active(&id).expect("activate DaemonLost");
                if let Some(comp) = self.application.get_component_mut(&id) {
                    comp.as_any_mut()
                        .downcast_mut::<DaemonLostComponent>()
                        .expect("DaemonLost component")
                        .set_content(
                            modal.last_playing_title.as_deref(),
                            &modal.daemon_log_path,
                            modal.restart_error.as_deref(),
                        );
                }
            }
            OverlayRequest::RemoteReanchor(popup) => {
                self.dismiss_blocking_modals();
                let id = Self::remote_reanchor_id();
                self.application
                    .mount(id.clone(), Box::new(RemoteReanchorComponent::new()), vec![])
                    .expect("mount RemoteReanchor");
                self.application
                    .active(&id)
                    .expect("activate RemoteReanchor");
                if let Some(comp) = self.application.get_component_mut(&id) {
                    comp.as_any_mut()
                        .downcast_mut::<RemoteReanchorComponent>()
                        .expect("RemoteReanchor component")
                        .set_content(&popup.targets, popup.cursor);
                }
            }
            OverlayRequest::SavePlaylist(dialog) => {
                self.dismiss_blocking_modals();
                let id = ComponentId::Modal(ModalId::SavePlaylist);
                self.application
                    .mount(id.clone(), Box::new(SavePlaylistComponent::new()), vec![])
                    .expect("mount SavePlaylist");
                self.application.active(&id).expect("activate SavePlaylist");
                if let Some(comp) = self.application.get_component_mut(&id) {
                    comp.as_any_mut()
                        .downcast_mut::<SavePlaylistComponent>()
                        .expect("SavePlaylist component")
                        .set_dialog(dialog.input, dialog.stage);
                }
            }
            OverlayRequest::SelectionModal(modal) => {
                self.dismiss_blocking_modals();
                let id = ComponentId::Overlay(OverlayId::SelectionModal);
                self.application
                    .mount(id.clone(), Box::new(SelectionModalComponent::new()), vec![])
                    .expect("mount SelectionModal");
                self.application
                    .active(&id)
                    .expect("activate SelectionModal");
                if let Some(comp) = self.application.get_component_mut(&id) {
                    comp.as_any_mut()
                        .downcast_mut::<SelectionModalComponent>()
                        .expect("SelectionModal component")
                        .set_content(&modal);
                }
            }
            OverlayRequest::RefreshSelectionModal {
                source,
                state,
                filter,
            } => {
                let id = ComponentId::Overlay(OverlayId::SelectionModal);
                if let Some(comp) = self.application.get_component_mut(&id) {
                    comp.as_any_mut()
                        .downcast_mut::<SelectionModalComponent>()
                        .expect("SelectionModal component")
                        .refresh(&source, state, filter);
                }
            }
            OverlayRequest::RefreshSelectionModalAtSelectedFilter { source } => {
                let id = ComponentId::Overlay(OverlayId::SelectionModal);
                let matches = self
                    .application
                    .get_component(&id)
                    .and_then(|component| {
                        component.as_any().downcast_ref::<SelectionModalComponent>()
                    })
                    .and_then(SelectionModalComponent::source)
                    .is_some_and(|current| current == &source);
                if matches {
                    self.handle_selection_modal_request(
                        super::super::components::ShellRequest::SelectionModalRefresh,
                    );
                }
            }
            OverlayRequest::DismissConfirm => self.dismiss_modal(&Self::confirm_id()),
            OverlayRequest::DismissDaemonLost => self.dismiss_modal(&Self::daemon_lost_id()),
            OverlayRequest::DismissRemoteReanchor => {
                self.dismiss_modal(&Self::remote_reanchor_id())
            }
            OverlayRequest::DismissSavePlaylist => {
                self.dismiss_modal(&ComponentId::Modal(ModalId::SavePlaylist))
            }
            OverlayRequest::DismissSelectionModal => {
                self.dismiss_modal(&ComponentId::Overlay(OverlayId::SelectionModal))
            }
        }
        self.assert_modal_mount_exclusive();
        self.app.blocking_overlay_active = self.blocking_overlay_active();
    }

    fn assert_modal_mount_exclusive(&self) {
        debug_assert!(
            !(self
                .application
                .mounted(&ComponentId::Modal(ModalId::SavePlaylist))
                && self.application.mounted(&Self::confirm_id())),
            "SavePlaylist and Confirm mounts must not both be active"
        );
    }

    pub(in crate::app) fn dismiss_modal(&mut self, id: &ComponentId) {
        if self.application.mounted(id) {
            let _ = self.application.umount(id);
        }
    }

    fn dismiss_blocking_modals(&mut self) {
        self.dismiss_modal(&Self::confirm_id());
        self.dismiss_modal(&Self::daemon_lost_id());
        self.dismiss_modal(&Self::remote_reanchor_id());
        self.dismiss_modal(&ComponentId::Modal(ModalId::SavePlaylist));
        self.dismiss_modal(&ComponentId::Overlay(OverlayId::SelectionModal));
    }
}
