use super::components::{ComponentId, ModalId, OverlayId, PopupId, UiRootComponent};
use super::shell::Model;

impl Model {
    pub(super) fn sync_overlay_stack(&mut self) {
        let mounted: Vec<_> = UiRootComponent::overlay_ids()
            .iter()
            .filter(|id| self.application.mounted(id))
            .cloned()
            .collect();
        if let Some(component) = self.application.get_component_mut(&ComponentId::UiRoot) {
            if let Some(root) = component.as_any_mut().downcast_mut::<UiRootComponent>() {
                root.sync_overlay_order(&mounted);
            }
        }
    }

    pub(super) fn overlay_stack(&self) -> Vec<ComponentId> {
        self.application
            .get_component(&ComponentId::UiRoot)
            .and_then(|component| component.as_any().downcast_ref::<UiRootComponent>())
            .map(|root| root.overlay_order().to_vec())
            .unwrap_or_default()
    }

    pub(super) fn render_overlay_stack(&mut self, frame: &mut ratatui::Frame) {
        for id in self.overlay_stack() {
            match id {
                ComponentId::Overlay(OverlayId::Settings) => self.render_settings_overlay(frame),
                ComponentId::Overlay(OverlayId::Playlists) => self.render_playlists_overlay(frame),
                ComponentId::Modal(ModalId::SavePlaylist) => {
                    self.render_save_playlist_overlay(frame)
                }
                ComponentId::Overlay(OverlayId::Help) => self.render_help_overlay(frame),
                ComponentId::Modal(ModalId::Confirm) => self.render_confirm_overlay(frame),
                ComponentId::Modal(ModalId::DaemonLost) => self.render_daemon_lost_overlay(frame),
                ComponentId::Modal(ModalId::RemoteReanchor) => {
                    self.render_remote_reanchor_overlay(frame)
                }
                ComponentId::Overlay(OverlayId::ContextMenu) => {
                    self.render_context_menu_overlay(frame)
                }
                ComponentId::Overlay(OverlayId::SelectionModal) => {
                    self.render_selection_modal_overlay(frame)
                }
                ComponentId::Popup(PopupId::Multiselect) => self.render_multiselect_popup(frame),
                ComponentId::Popup(PopupId::LibraryRoutes) => {
                    self.render_library_routes_popup(frame)
                }
                ComponentId::Popup(PopupId::FeedManage) => self.render_feeds_manage_popup(frame),
                ComponentId::Overlay(OverlayId::Search) => self.render_search_overlay(frame),
                ComponentId::Overlay(OverlayId::Sessions) => self.render_sessions_overlay(frame),
                _ => {}
            }
        }
    }

    pub(super) fn blocking_overlay_active(&self) -> bool {
        self.overlay_stack().iter().any(|id| {
            matches!(
                id,
                ComponentId::Overlay(OverlayId::ContextMenu)
                    | ComponentId::Overlay(OverlayId::SelectionModal)
                    | ComponentId::Modal(ModalId::Confirm)
                    | ComponentId::Modal(ModalId::DaemonLost)
                    | ComponentId::Modal(ModalId::RemoteReanchor)
                    | ComponentId::Modal(ModalId::SavePlaylist)
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{HelpComponent, ModalId, OverlayId};
    use crate::app::tests::make_app_stub;

    #[test]
    fn root_ui_uses_native_lifo_focus_restoration() {
        let mut model = Model::new(make_app_stub());
        let help = ComponentId::Overlay(OverlayId::Help);
        let confirm = ComponentId::Modal(ModalId::Confirm);

        model
            .application
            .mount(help.clone(), Box::new(HelpComponent::new()), vec![])
            .unwrap();
        model.application.active(&help).unwrap();
        model
            .application
            .mount(
                confirm.clone(),
                Box::new(crate::app::components::ConfirmComponent::new()),
                vec![],
            )
            .unwrap();
        model.application.active(&confirm).unwrap();

        model.application.umount(&confirm).unwrap();
        assert_eq!(model.application.focus(), Some(&help));
        model.application.umount(&help).unwrap();
        assert_eq!(model.application.focus(), Some(&ComponentId::UiRoot));
    }
}
