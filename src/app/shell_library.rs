use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    /// Route TuiRealm's native LIFO focus to the active destination's child
    /// component, or back to `UiRoot` when the destination has no mounted
    /// surface component (e.g. an un-componented podcast/feed-group Emby
    /// library). Idempotent: `active()` on the already-active component is a
    /// no-op. The old `sync_library_parent` mirrored routing state into
    /// `LibraryComponent` each tick and read it back to activate the child;
    /// the child is derived directly now (task 5.3d — no routing mirror).
    pub(super) fn sync_active_destination(&mut self) {
        if self.library_overlay_mounted() {
            return;
        }
        let target = self
            .library_child_id()
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
    use crate::app::components::OverlayId;
    use crate::app::render::make_movie_app;
    use crate::app::{PanelFocus, PanelMode, TabSelection};

    #[test]
    fn shell_routes_focus_to_the_active_destination_child() {
        let mut model = Model::new(make_movie_app());
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Library;
        model.app.panel_mode = PanelMode::Both;
        model.sync_emby_browser();
        model.sync_active_destination();

        let child = model
            .emby_browser_id
            .clone()
            .expect("generic browser mounted");
        assert_eq!(model.application.focus(), Some(&child));
        assert!(model
            .application
            .get_component(&child)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());
    }

    #[test]
    fn shell_routes_focus_back_to_ui_root_without_a_mounted_child() {
        let mut model = Model::new(make_movie_app());
        // Podcast libraries have no surface component; the destination falls
        // back to UiRoot (whose terminal translation owns the remaining
        // legacy key dispatch for those surfaces).
        model.app.libs[0].library.item_type = "Channel".into();
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Library;
        model.app.panel_mode = PanelMode::Both;
        model.sync_active_destination();

        assert_eq!(model.application.focus(), Some(&ComponentId::UiRoot));
    }

    #[test]
    fn shell_skips_focus_routing_while_an_overlay_is_mounted() {
        let mut model = Model::new(make_movie_app());
        model.app.tab = TabSelection::Home;
        model.mount_help();
        model.sync_active_destination();

        assert_eq!(
            model.application.focus(),
            Some(&ComponentId::Overlay(OverlayId::Help))
        );
    }
}
