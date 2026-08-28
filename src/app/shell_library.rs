use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// Route TuiRealm's native LIFO focus to the active destination's child
    /// component, or back to `UiRoot` when the destination has no mounted
    /// surface component (e.g. an un-componented podcast/feed-group Emby
    /// library). Idempotent: `active()` on the already-active component is a
    /// no-op. The old `sync_library_parent` mirrored routing state into
    /// `LibraryComponent` each tick and read it back to activate the child;
    /// the child is derived directly now (task 5.3d — no routing mirror).
    ///
    /// Short-circuits when Queue owns panel focus (and no blocking overlay
    /// is up): `sync_queue` already activated `ComponentId::Queue` a few
    /// lines earlier in the same tick, and we must not stomp it by
    /// re-activating the Library child or `UiRoot` on top. Without this
    /// guard Queue falls back to legacy key routing (issue #610, blocks
    /// the #607 acceptance gate). Mirrors the exact condition
    /// `sync_queue` uses to claim focus.
    pub(super) fn sync_active_destination(&mut self) {
        if self.library_overlay_mounted() {
            return;
        }
        let queue_owns_focus = matches!(self.app.effective_panel_focus(), PanelFocus::Queue)
            && !self.blocking_overlay_active();
        if queue_owns_focus {
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
        if let Some(id) = self.inline_search_component_id(index) {
            return Some(id);
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

    /// Production-style acceptance test for #610 / #607: when Queue owns
    /// panel focus, the per-tick sync sequence (`sync_queue` followed by
    /// `sync_active_destination` in `shell_run.rs`) must leave
    /// `ComponentId::Queue` as the active TuiRealm component. Without the
    /// Queue-owner guard in `sync_active_destination`, the destination
    /// sync re-activates the Library child (or `UiRoot`) on top of Queue,
    /// and Queue falls back to legacy key routing.
    #[test]
    fn shell_preserves_queue_focus_across_destination_sync() {
        use crate::app::components::QueueComponent;
        let mut model = Model::new(make_movie_app());
        // Pretend a user action (Alt+Right, mouse click, etc.) just
        // moved panel focus to Queue. With no overlay mounted, this is
        // exactly the precondition the production main loop sees each
        // tick once `sync_queue` activates the component.
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Queue;
        model.app.panel_mode = PanelMode::Both;

        // Mirror the production call order at shell_run.rs:427-433.
        model.sync_queue();
        model.sync_active_destination();

        assert!(
            model.application.mounted(&ComponentId::Queue),
            "sync_queue must mount Queue so it can claim focus"
        );
        assert_eq!(
            model.application.focus(),
            Some(&ComponentId::Queue),
            "Queue must remain the active TuiRealm component when it owns panel focus"
        );
        // The component is the Queue surface, not a re-claimed destination
        // or UiRoot fallback. A downcast succeeds iff focus is actually
        // on the Queue component (i.e., it's mounted and active).
        let component = model
            .application
            .get_component(&ComponentId::Queue)
            .expect("Queue mounted")
            .as_any()
            .downcast_ref::<QueueComponent>();
        assert!(
            component.is_some(),
            "active component must be QueueComponent when Queue owns panel focus"
        );
    }

    /// Symmetric regression guard: when a blocking modal is up but
    /// `panel_focus` is still `Queue` (e.g. a stale focus from a just-
    /// closed sidebar), the destination sync must still route focus to
    /// the Library child. `sync_queue` itself skips activation under
    /// blocking overlays, so the destination is the only legitimate
    /// focus owner in that window. Help is non-blocking (it dims the
    /// Library but does not consume the keyboard), so we mount a
    /// blocking modal — `ComponentId::Modal(ModalId::Confirm)` — that
    /// `blocking_overlay_active` actually reports.
    #[test]
    fn shell_routes_focus_to_destination_when_blocking_overlay_suppresses_queue() {
        use crate::app::components::{BrowserComponent, ConfirmComponent, ModalId};
        let mut model = Model::new(make_movie_app());
        model.app.tab = TabSelection::EmbyLibrary(0);
        model.app.panel_focus = PanelFocus::Queue;
        model.app.panel_mode = PanelMode::Both;
        // Mount a blocking modal so `blocking_overlay_active` is true and
        // `sync_queue` will not claim focus.
        model
            .application
            .mount(
                ComponentId::Modal(ModalId::Confirm),
                Box::new(ConfirmComponent::new()),
                vec![],
            )
            .expect("mount Confirm");
        model.sync_emby_browser();

        model.sync_queue();
        model.sync_active_destination();

        let child = model
            .emby_browser_id
            .clone()
            .expect("generic browser mounted");
        assert_eq!(
            model.application.focus(),
            Some(&child),
            "destination must own TuiRealm focus when a blocking overlay suppresses Queue"
        );
        assert!(model
            .application
            .get_component(&child)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .is_some());
    }
}
