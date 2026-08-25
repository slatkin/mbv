use super::components::{AudiobookshelfPodcastComponent, BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_audiobookshelf_podcast_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> bool {
        self.app.handle_key(key)
    }

    fn abs_podcast_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.audiobookshelf_libraries.get(index)?;
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: library.id.clone(),
            kind: BrowserKind::AudiobookshelfPodcast,
        }))
    }

    pub(super) fn sync_audiobookshelf_podcast(&mut self) {
        let next_id = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index)
                if matches!(
                    self.app.audiobookshelf_kind_at(index),
                    Some(AudiobookshelfBrowseKind::Podcast)
                ) =>
            {
                self.abs_podcast_component_id(index)
            }
            _ => None,
        };
        if self.abs_podcast_id != next_id {
            if let Some(id) = self.abs_podcast_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(
                        id.clone(),
                        Box::new(AudiobookshelfPodcastComponent::new()),
                        vec![],
                    )
                    .expect("mount Audiobookshelf podcast browser");
                self.application
                    .active(&id)
                    .expect("activate Audiobookshelf podcast browser");
                self.abs_podcast_id = Some(id);
            }
        }
        let Some(id) = self.abs_podcast_id.as_ref() else {
            return;
        };
        let index = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index) => index,
            _ => return,
        };
        let Some(snapshot) = self.app.audiobookshelf_browse.get(index) else {
            return;
        };
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(podcast) = comp
                .as_any_mut()
                .downcast_mut::<AudiobookshelfPodcastComponent>()
            {
                podcast.set_content(snapshot, focused, self.app.images_enabled());
            }
        }
    }

    pub(super) fn render_audiobookshelf_podcast_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.abs_podcast_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.audiobookshelf_podcast_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{Msg, ShellRequest};
    use crate::app::tests_podcast::audiobookshelf_app;
    use mbv_core::audiobookshelf::AudiobookshelfShow;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn abs_podcast_shell_mounts_and_routes_component() {
        let mut model = Model::new(audiobookshelf_app());
        model.app.audiobookshelf_browse[0].append_page(
            1,
            20,
            2,
            vec![AudiobookshelfShow {
                library_item_id: "show-b".into(),
                title: "Show B".into(),
                author: None,
                description: None,
                cover_path: None,
            }],
        );
        model.sync_audiobookshelf_podcast();
        let id = model
            .abs_podcast_id
            .clone()
            .expect("podcast component mounted");
        let message = model
            .application
            .get_component_mut(&id)
            .expect("podcast component")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastKey(key))) = message else {
            panic!("podcast movement should be routed as a shell request");
        };
        assert!(!model.handle_audiobookshelf_podcast_key(key));
        assert_eq!(model.app.audiobookshelf_browse[0].cursor(), 1);
    }
}
