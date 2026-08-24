use super::components::{ComponentId, ModalId, OverlayId, PlaylistsComponent};
use super::shell::Model;

impl Model {
    pub(super) fn sync_playlists(&mut self) {
        let id = ComponentId::Overlay(OverlayId::Playlists);
        let mounted = self.application.mounted(&id);
        if self.app.is_sidebar_open(super::SidebarId::Playlists) && !mounted {
            self.application
                .mount(id.clone(), Box::new(PlaylistsComponent::new()), vec![])
                .expect("mount Playlists");
            self.application.active(&id).expect("activate Playlists");
        } else if !self.app.is_sidebar_open(super::SidebarId::Playlists) && mounted {
            let _ = self.application.umount(&id);
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(playlists) = comp.as_any_mut().downcast_mut::<PlaylistsComponent>() {
                playlists.set_content(
                    self.app.playlists.clone(),
                    self.app.playlists_cursor,
                    self.app.playlists_scroll,
                    self.app.playlists_loading,
                    self.app.playlists_open.clone(),
                    self.app.playlists_open_items.clone(),
                    self.app.playlists_open_cursor,
                    self.app.playlists_open_scroll,
                    self.app.playlists_open_loading,
                    match &self.app.queue_source {
                        crate::config::QueueSource::Playlist { id: Some(id), .. } => {
                            Some(id.clone())
                        }
                        _ => None,
                    },
                );
                let panel = (self.app.layout.main.panel_area.width > 0)
                    .then_some(self.app.layout.main.panel_area);
                playlists.set_panel_area(panel);
            }
        }
    }

    pub(super) fn render_playlists_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Overlay(OverlayId::Playlists);
        if !self.application.mounted(&id) {
            return;
        }
        let panel =
            (self.app.layout.main.panel_area.width > 0).then_some(self.app.layout.main.panel_area);
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(playlists) = comp.as_any_mut().downcast_mut::<PlaylistsComponent>() {
                playlists.set_panel_area(panel);
            }
        }
        self.application.view(&id, frame, frame.area());
    }

    pub(super) fn render_save_playlist_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Modal(ModalId::SavePlaylist);
        if self.application.mounted(&id) {
            self.application.view(&id, frame, frame.area());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{Msg, ShellRequest};
    use crate::app::tests::make_app_stub;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn playlists_shell_mounts_and_routes_component() {
        let mut app = make_app_stub();
        app.open_sidebar(crate::app::SidebarId::Playlists);
        let mut model = Model::new(app);
        model.sync_playlists();
        let id = ComponentId::Overlay(OverlayId::Playlists);
        let message = model
            .application
            .get_component_mut(&id)
            .expect("Playlists component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::PlaylistsKey(_)))
        ));
    }

    #[test]
    fn save_playlist_shell_mounts_and_routes_component() {
        let mut app = make_app_stub();
        app.open_save_playlist_dialog(crate::app::SavePlaylistDialog {
            input: "Playlist".into(),
            stage: crate::app::SavePlaylistStage::EnterName,
        });
        let mut model = Model::new(app);
        model.sync_modal_requests();
        let id = ComponentId::Modal(ModalId::SavePlaylist);
        let message = model
            .application
            .get_component_mut(&id)
            .expect("Save-playlist component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::SavePlaylistKey(_)))
        ));
    }
}
