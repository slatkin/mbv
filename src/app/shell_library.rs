use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// Route TuiRealm's native LIFO focus to the active destination's child
    /// component, or back to `UiRoot` when the destination has no mounted
    /// surface component (e.g. a narrow non-wide grouped-Music Emby
    /// library). Idempotent: `active()` on the already-active component is a
    /// no-op. The destination child is derived directly from `App.tab` via
    /// `library_child_id`; there is no Library-parent component or routing
    /// mirror.
    ///
    /// Short-circuits when Queue owns panel focus (and no blocking overlay
    /// is up): `sync_queue` already activated `ComponentId::Queue` a few
    /// lines earlier in the same tick, and we must not stomp it by
    /// re-activating the Library child or `UiRoot` on top. Without this
    /// guard Queue falls back to legacy key routing (issue #610, blocks
    /// the #607 acceptance gate). Mirrors the exact condition
    /// `sync_queue` uses to claim focus.
    pub(super) fn sync_active_destination(&mut self) {
        if self.overlay_holds_focus() {
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
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        // Wide TV focuses `TvWorkspaceComponent` under its distinct
        // `ComponentId::TvWorkspace`; narrow TV focuses the mounted
        // `BrowserComponent` under `ComponentId::Browser` (D4). The two
        // mount gates share `is_wide_tv_active()`, so mirror that split here.
        if kind == BrowserKind::TvShows && self.app.layout.main.is_wide_tv_active() {
            return Some(ComponentId::TvWorkspace(BrowserKey {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind,
            }));
        }
        let mounted_surface = match kind {
            BrowserKind::Generic | BrowserKind::Movies | BrowserKind::HomeVideos => true,
            // Narrow TV focuses the mounted BrowserComponent (D4), matching
            // `emby_browser_component_id`.
            BrowserKind::TvShows => true,
            BrowserKind::Music => {
                // Music mounts one component type at all widths (no TV-style
                // split), so narrow Music is focusable too — the mount gate is
                // already width-agnostic; only this focus gate was wide-only.
                self.app.is_music_group_view(index) && self.app.is_viewing_album_folders(index)
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

    /// ADR 0024 D2: the mouse-eligible component set for the current frame, a
    /// three-rung ladder derived off the same `library_child_id()` the
    /// active-destination pass uses (no second "did I paint" ledger).
    pub(super) fn mouse_eligible_ids(&self) -> Vec<ComponentId> {
        use super::components::{ModalId, OverlayId, PopupId};

        // Rung 1: a mounted blocking overlay/modal is eligible alone.
        const BLOCKING: &[ComponentId] = &[
            ComponentId::Overlay(OverlayId::ContextMenu),
            ComponentId::Overlay(OverlayId::SelectionModal),
            ComponentId::Modal(ModalId::Confirm),
            ComponentId::Modal(ModalId::DaemonLost),
            ComponentId::Modal(ModalId::RemoteReanchor),
            ComponentId::Modal(ModalId::SavePlaylist),
            ComponentId::Popup(PopupId::Multiselect),
            ComponentId::Popup(PopupId::LibraryRoutes),
            ComponentId::Popup(PopupId::FeedManage),
        ];
        if let Some(id) = BLOCKING.iter().find(|id| self.application.mounted(id)) {
            return vec![id.clone()];
        }

        // Rung 2: else the topmost mounted panel-covering overlay/popup alone.
        // `OVERLAY_IDS` is canonical bottom-to-top, so the last mounted one is
        // topmost. No blocking overlay is mounted at this point, so every
        // remaining match is a non-blocking panel-covering overlay.
        if let Some(id) = super::components::UiRootComponent::overlay_ids()
            .iter()
            .rev()
            .find(|id| self.application.mounted(id))
        {
            return vec![id.clone()];
        }

        // Rung 3: the components painted this frame — active destination,
        // Queue, and Playback (the transport chrome).
        let mut ids = Vec::new();
        if let Some(child) = self
            .library_child_id()
            .filter(|child| self.application.mounted(child))
        {
            ids.push(child);
        }
        for id in [ComponentId::Queue, ComponentId::Playback] {
            if self.application.mounted(&id) {
                ids.push(id);
            }
        }
        ids
    }

    /// ADR 0024 D2: reconcile the `mouse_sub()` subscription table to
    /// `mouse_eligible_ids()`. Runs in `sync_mounted_surfaces` immediately
    /// after `sync_active_destination`.
    ///
    /// `tuirealm` 4.1's `Application::unsubscribe(id, clause)` retains
    /// `s.target() != id && s.event() != &clause`, so it drops *every*
    /// subscription whose clause equals the mouse clause, not just `id`'s.
    /// Any change therefore wipes the whole mouse table and rebuilds it from
    /// the eligible set; `self.mouse_subscribed` mirrors the result so the
    /// reconciler knows the current state without querying `Application`.
    pub(super) fn sync_mouse_subscriptions(&mut self) {
        let eligible: std::collections::HashSet<ComponentId> =
            self.mouse_eligible_ids().into_iter().collect();
        if eligible == self.mouse_subscribed {
            return;
        }
        // Wipe: one successful unsubscribe clears every mouse subscription.
        if let Some(anchor) = self
            .mouse_subscribed
            .iter()
            .find(|id| self.application.mounted(id))
            .cloned()
        {
            let _ = self
                .application
                .unsubscribe(&anchor, super::components::mouse_event_clause());
        }
        self.mouse_subscribed.clear();
        for id in eligible {
            if self.application.mounted(&id)
                && self
                    .application
                    .subscribe(&id, super::components::mouse_sub())
                    .is_ok()
            {
                self.mouse_subscribed.insert(id);
            }
        }
    }

    /// Whether any sidebar, modal, or popup overlay is mounted. Every
    /// focus-management sync pass (`sync_active_destination`, `sync_queue`)
    /// consults this before re-activating its own surface, so a mounted
    /// overlay that just took focus is never stolen back on the next tick.
    pub(super) fn overlay_holds_focus(&self) -> bool {
        [
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
#[path = "shell_library_tests.rs"]
mod tests;
