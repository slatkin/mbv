use super::components::{BrowserKey, BrowserKind, ComponentId, LibraryComponent};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn sync_library_parent(&mut self) {
        let child = self.library_child_id();
        let id = ComponentId::Library;
        if let Some(component) = self.application.get_component_mut(&id) {
            if let Some(library) = component.as_any_mut().downcast_mut::<LibraryComponent>() {
                library.set_content(
                    self.app.tab,
                    self.app.effective_panel_focus(),
                    self.app.effective_panel_mode(),
                    child,
                );
            }
        }

        if self.library_overlay_mounted() {
            return;
        }
        let target = self
            .application
            .get_component(&id)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<LibraryComponent>()
                    .and_then(|library| library.active_child().cloned())
            })
            .filter(|child| self.application.mounted(child))
            .unwrap_or(ComponentId::UiRoot);
        if self.application.mounted(&target) {
            self.application
                .active(&target)
                .expect("activate Library child");
        }
    }

    fn library_child_id(&self) -> Option<ComponentId> {
        match self.app.tab {
            TabSelection::Home => Some(ComponentId::Home),
            TabSelection::Feeds => Some(ComponentId::Feeds),
            TabSelection::EmbyLibrary(index) => self.emby_library_child_id(index),
            TabSelection::AudiobookshelfLibrary(index) => self.abs_library_child_id(index),
        }
    }

    fn emby_library_child_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        if self.inline_search_id.is_some() {
            return Some(ComponentId::InlineSearch(BrowserKey {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind: BrowserKind::from_collection_type(&library.library.collection_type),
            }));
        }
        if self.app.is_podcast_library(index) || self.app.is_feed_home_video_group_view(index) {
            return None;
        }
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        let mounted_surface = match kind {
            BrowserKind::Generic | BrowserKind::Movies | BrowserKind::HomeVideos => true,
            BrowserKind::TvShows => self.app.layout.main.is_wide_tv_active(),
            BrowserKind::Music => {
                self.app.is_music_group_view(index)
                    && self.app.is_viewing_album_folders(index)
                    && self.app.layout.main.is_wide_music_active()
            }
            BrowserKind::AudiobookshelfPodcast | BrowserKind::AudiobookshelfBook => false,
        };
        mounted_surface.then_some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind,
        }))
    }

    fn abs_library_child_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.audiobookshelf_libraries.get(index)?;
        let kind = match self.app.audiobookshelf_kind_at(index)? {
            AudiobookshelfBrowseKind::Podcast => BrowserKind::AudiobookshelfPodcast,
            AudiobookshelfBrowseKind::Book => BrowserKind::AudiobookshelfBook,
        };
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: library.id.clone(),
            kind,
        }))
    }

    fn library_overlay_mounted(&self) -> bool {
        [
            ComponentId::PlaybackPrompt,
            ComponentId::Overlay(super::components::OverlayId::Search),
            ComponentId::Overlay(super::components::OverlayId::Settings),
            ComponentId::Overlay(super::components::OverlayId::Sessions),
            ComponentId::Overlay(super::components::OverlayId::Playlists),
            ComponentId::Overlay(super::components::OverlayId::Help),
            ComponentId::Overlay(super::components::OverlayId::ContextMenu),
            ComponentId::Overlay(super::components::OverlayId::SelectionModal),
            ComponentId::Modal(super::components::ModalId::Confirm),
            ComponentId::Modal(super::components::ModalId::DaemonLost),
            ComponentId::Modal(super::components::ModalId::RemoteReanchor),
            ComponentId::Modal(super::components::ModalId::SavePlaylist),
            ComponentId::Popup(super::components::PopupId::Multiselect),
            ComponentId::Popup(super::components::PopupId::LibraryRoutes),
            ComponentId::Popup(super::components::PopupId::FeedManage),
        ]
        .iter()
        .any(|id| self.application.mounted(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::BrowserComponent;
    use crate::app::render::make_movie_app;
    use crate::app::{PanelFocus, PanelMode, TabSelection};

    #[test]
    fn library_parent_shell_sync_mirrors_and_routes_generic_child() {
        let mut model = Model::new(make_movie_app());
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Library;
        model.app.panel_mode = PanelMode::Both;
        model.sync_emby_browser();
        model.sync_library_parent();

        let parent = model
            .application
            .get_component(&ComponentId::Library)
            .expect("Library parent mounted")
            .as_any()
            .downcast_ref::<LibraryComponent>()
            .expect("Library parent type");
        assert_eq!(parent.destination(), TabSelection::EmbyLibrary(0));
        assert_eq!(parent.panel_focus(), PanelFocus::Library);
        assert_eq!(parent.panel_mode(), PanelMode::Both);
        assert_eq!(
            parent.active_child(),
            model.emby_browser_id.as_ref(),
            "parent routes the active destination to its mounted child"
        );
        assert!(model
            .application
            .get_component(parent.active_child().unwrap())
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());
    }
}
