use super::components::{BrowserKey, BrowserKind, ComponentId, MusicWorkspaceComponent};
use super::render::MusicWideRenderCtx;
use super::shell::Model;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    fn music_workspace_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if library.library.collection_type != "music"
            || !self.app.is_music_group_view(index)
            || !self.app.is_viewing_album_folders(index)
        {
            return None;
        }
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::Music,
        }))
    }

    pub(super) fn sync_music_workspace(&mut self) {
        let next_id = self.music_workspace_component_id();
        if self.music_workspace_id != next_id {
            if let Some(id) = self.music_workspace_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(id.clone(), Box::new(MusicWorkspaceComponent::new()), vec![])
                    .expect("mount Music workspace");
                self.application
                    .active(&id)
                    .expect("activate Music workspace");
                self.music_workspace_id = Some(id);
            }
        }

        let Some(id) = self.music_workspace_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let context: MusicWideRenderCtx = self.app.wide_music_render_ctx(index);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(music) = comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>() {
                music.set_content(context);
            }
        }
    }

    pub(super) fn render_music_workspace_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.music_workspace_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.wide_music_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
        let image_paint = self
            .application
            .get_component_mut(id)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>())
            .and_then(MusicWorkspaceComponent::take_image_paint);
        self.app.paint_music_image(frame, image_paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{LegacyTerminalEvent, Msg};
    use crate::app::render::make_music_group_app;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn shell_mounts_and_syncs_music_workspace() {
        let mut model = Model::new(make_music_group_app());
        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("Music workspace mounted");
        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
        ));
    }

    #[test]
    fn shell_mounts_music_workspace_in_narrow_mode() {
        let mut model = Model::new(make_music_group_app());
        assert!(model.app.is_music_group_view(0));
        assert!(model.app.is_viewing_album_folders(0));
        assert!(!model.app.layout.main.is_wide_music_active());

        let wide_area = model.app.layout.main.wide_music_area;
        assert_eq!(wide_area.width, 0);
        assert_eq!(wide_area.height, 0);
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("narrow Music workspace mounted");
        assert!(model.application.mounted(&id));
        assert_eq!(model.app.layout.main.wide_music_area, wide_area);
    }
}
