use super::super::components::{
    ComponentId, ConfirmComponent, DaemonLostComponent, ModalId, RemoteReanchorComponent,
};
use super::super::shell::Model;

impl Model {
    // --- Confirm modal ------------------------------------------------------
    //
    // The Confirm modal is a blocking overlay mounted when `App::confirm_modal`
    // transitions from `None` to `Some`. The component owns rendering and
    // forwards keys to the shell's existing `handle_key_confirm_modal`; the
    // shell owns `ConfirmAction` dispatch (design D4/D9).

    fn confirm_id() -> ComponentId {
        ComponentId::Modal(ModalId::Confirm)
    }

    /// Sync the Confirm component mount state with `App::confirm_modal`.
    pub(in crate::app) fn sync_confirm_modal(&mut self) {
        let id = Self::confirm_id();
        let mounted = self.application.mounted(&id);
        if self.app.confirm_modal.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(ConfirmComponent::new()), vec![])
                .expect("mount Confirm");
            self.application.active(&id).expect("activate Confirm");
        } else if self.app.confirm_modal.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the Confirm overlay if mounted.
    pub(in crate::app) fn render_confirm_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::confirm_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(confirm) = comp.as_any_mut().downcast_mut::<ConfirmComponent>() {
                if let Some(ref modal) = self.app.confirm_modal {
                    confirm.set_content(&modal.title, &modal.message, &modal.hint);
                }
            }
        }
        self.application.view(&id, f, f.area());
    }

    // --- Daemon-lost modal --------------------------------------------------

    fn daemon_lost_id() -> ComponentId {
        ComponentId::Modal(ModalId::DaemonLost)
    }

    /// Sync the DaemonLost component mount state with `App::daemon_lost_modal`.
    pub(in crate::app) fn sync_daemon_lost_modal(&mut self) {
        let id = Self::daemon_lost_id();
        let mounted = self.application.mounted(&id);
        if self.app.daemon_lost_modal.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(DaemonLostComponent::new()), vec![])
                .expect("mount DaemonLost");
            self.application.active(&id).expect("activate DaemonLost");
        } else if self.app.daemon_lost_modal.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the DaemonLost overlay if mounted.
    pub(in crate::app) fn render_daemon_lost_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::daemon_lost_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(daemon_lost) = comp.as_any_mut().downcast_mut::<DaemonLostComponent>() {
                if let Some(ref modal) = self.app.daemon_lost_modal {
                    daemon_lost.set_content(
                        modal.last_playing_title.as_deref(),
                        &modal.daemon_log_path,
                        modal.restart_error.as_deref(),
                    );
                }
            }
        }
        self.application.view(&id, f, f.area());
    }

    // --- Remote-reanchor popup ----------------------------------------------

    fn remote_reanchor_id() -> ComponentId {
        ComponentId::Modal(ModalId::RemoteReanchor)
    }

    /// Sync the RemoteReanchor component mount state with
    /// `App::remote_reanchor_popup`.
    pub(in crate::app) fn sync_remote_reanchor_popup(&mut self) {
        let id = Self::remote_reanchor_id();
        let mounted = self.application.mounted(&id);
        if self.app.remote_reanchor_popup.is_some() && !mounted {
            self.application
                .mount(id.clone(), Box::new(RemoteReanchorComponent::new()), vec![])
                .expect("mount RemoteReanchor");
            self.application
                .active(&id)
                .expect("activate RemoteReanchor");
        } else if self.app.remote_reanchor_popup.is_none() && mounted {
            let _ = self.application.umount(&id);
        }
    }

    /// Render the RemoteReanchor overlay if mounted.
    pub(in crate::app) fn render_remote_reanchor_overlay(&mut self, f: &mut ratatui::Frame) {
        let id = Self::remote_reanchor_id();
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(reanchor) = comp.as_any_mut().downcast_mut::<RemoteReanchorComponent>() {
                if let Some(ref popup) = self.app.remote_reanchor_popup {
                    reanchor.set_content(&popup.targets, popup.cursor);
                }
            }
        }
        self.application.view(&id, f, f.area());
    }
}
