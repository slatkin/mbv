use super::components::{ComponentId, ModalId, OverlayId, PopupId, UiRootComponent};
use super::shell::Model;

impl Model {
    /// Overlay paint order is the canonical `OVERLAY_IDS` order filtered by
    /// mount state (the deleted `sync_overlay_stack`/`UiRootComponent::sync_overlay_order`
    /// mirror kept a retained mount order; TuiRealm's native LIFO focus stack
    /// owns actual stacking, so the paint order only needs to be a stable
    /// canonical order — task 5.3d).
    pub(super) fn render_overlay_stack(&mut self, frame: &mut ratatui::Frame) {
        let mounted: Vec<ComponentId> = UiRootComponent::overlay_ids()
            .iter()
            .filter(|id| self.application.mounted(id))
            .cloned()
            .collect();
        for id in mounted {
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
        [
            ComponentId::Overlay(OverlayId::ContextMenu),
            ComponentId::Overlay(OverlayId::SelectionModal),
            ComponentId::Modal(ModalId::Confirm),
            ComponentId::Modal(ModalId::DaemonLost),
            ComponentId::Modal(ModalId::RemoteReanchor),
            ComponentId::Modal(ModalId::SavePlaylist),
            ComponentId::Popup(PopupId::Multiselect),
            ComponentId::Popup(PopupId::LibraryRoutes),
            ComponentId::Popup(PopupId::FeedManage),
        ]
        .iter()
        .any(|id| self.application.mounted(id))
    }

    /// Whether a blocking overlay is open (the F1 Help-open guard). The
    /// prompt removal deleted `shell_playback_prompt.rs`; the wrapper now
    /// lives next to the fact it mirrors.
    pub(super) fn is_blocking_overlay_open(&self) -> bool {
        self.blocking_overlay_active()
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
