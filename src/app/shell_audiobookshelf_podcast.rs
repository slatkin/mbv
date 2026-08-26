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

    /// Mounts / unmounts the Audiobookshelf podcast browser component to follow
    /// the active tab (task 5.3d). This is the mount lifecycle only: content is
    /// no longer mirrored into the component on every tick. The per-frame
    /// `set_content` projection was replaced by the event-scoped
    /// `push_audiobookshelf_podcast_content` at the writers of its projected
    /// inputs (active-tab, key/effect, async completion, progress,
    /// refresh/reset, and saved-position restore). Content is pushed right
    /// after a fresh mount so the newly mounted component paints the current
    /// browse snapshot.
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
                // Fresh mount: project the active tab's browse state so the
                // component is initialized with the current shows/selection
                // before it is painted (the active-tab writer).
                self.push_audiobookshelf_podcast_content();
            }
        }
    }

    /// Event-scoped projection replacing the per-frame content mirror (task
    /// 5.3d, `sync_audiobookshelf_podcast` Phase A): runs only when the active
    /// tab is the mounted podcast browser and mirrors the validated browse
    /// snapshot plus panel focus into `AudiobookshelfPodcastComponent` via
    /// `set_content` (preserving its selected-show/episode/scroll semantics
    /// exactly). Called at the writers of the projected inputs, so it is
    /// deterministic in `App` state and duplicate pushes are idempotent.
    /// `sync_audiobookshelf_podcast` keeps only mount lifecycle management.
    pub(super) fn push_audiobookshelf_podcast_content(&mut self) {
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
    use crate::app::components::msg::PodcastShowMove;
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
        let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(movement))) = message
        else {
            panic!("show movement should be routed as a typed show-move request");
        };
        assert_eq!(movement, PodcastShowMove::NextRow);
        // The shell arm maps NextRow onto the legacy row-stride move and
        // re-projects content (task 5.3d.5), preserving the App target.
        model.app.move_audiobookshelf_show_rows(1);
        model.push_audiobookshelf_podcast_content();
        assert_eq!(model.app.audiobookshelf_browse[0].cursor(), 1);
    }
}
