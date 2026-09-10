use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, PanelMode, TabSelection};
use mbv_core::config::ServiceKind;
use ratatui::layout::Rect;

/// The gap columns and the pointer→width resolution inputs for the active
/// Wide hero surface's two-pane split, when that surface is painting one this
/// frame.
struct WideHeroBoundaryGeometry {
    /// The shared `WIDE_HERO_PANE_GAP` gutter between the panes.
    gap: Rect,
    /// Left edge of the surface's content area; the pointer column minus this
    /// is the resolved list-pane width.
    pane_origin_x: u16,
    /// The content area's width, used to clamp the resolved width.
    content_width: u16,
    /// The list-pane width the current override (or default ratio) paints.
    list_pane_width: u16,
}

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

    pub(super) fn library_child_id(&self) -> Option<ComponentId> {
        match self.app.tab {
            TabSelection::Home => Some(ComponentId::Home),
            TabSelection::Feeds => Some(ComponentId::Feeds),
            TabSelection::EmbyLibrary(index) => self.emby_library_child_id(index),
            TabSelection::AudiobookshelfLibrary(index) => self.abs_library_child_id(index),
        }
    }

    /// Whether the right-panel library destination is painted this frame.
    /// The queue-only mode (including the narrow mini view) hides the library
    /// entirely, so its mounted destination must not paint over the queue that
    /// now owns the whole frame.
    pub(super) fn library_panel_visible(&self) -> bool {
        self.app.effective_panel_mode() != PanelMode::QueueOnly
    }

    fn emby_library_child_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.libs.get(index)?;
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        // Wide TV focuses `TvWorkspaceComponent` under its distinct
        // `ComponentId::TvWorkspace`; narrow TV focuses the mounted
        // `BrowserComponent` under `ComponentId::Browser` (D4). The two
        // mount gates share `wide_tv_library_area(index)`, so mirror that split here.
        if kind == BrowserKind::TvShows && self.app.wide_tv_library_area(index).is_some() {
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
        let panel_mode = self.app.effective_panel_mode();
        if let Some(child) = self
            .library_child_id()
            .filter(|child| self.library_panel_visible() && self.application.mounted(child))
        {
            ids.push(child);
        }
        for id in [
            ComponentId::Queue,
            ComponentId::QueueBoundary,
            ComponentId::Playback,
        ] {
            if self.application.mounted(&id)
                && (id != ComponentId::QueueBoundary || self.queue_boundary_mouse_eligible())
                && (id == ComponentId::Playback || panel_mode != super::PanelMode::LibraryOnly)
            {
                ids.push(id);
            }
        }
        if self.application.mounted(&ComponentId::WideHeroBoundary)
            && self.wide_hero_boundary_mouse_eligible()
        {
            ids.push(ComponentId::WideHeroBoundary);
        }
        ids
    }

    /// Whether the Queue boundary column may receive mouse input. Shared by
    /// `mouse_eligible_ids` (subscription) and `sync_queue_boundary` (arming)
    /// so the two can never disagree.
    pub(super) fn queue_boundary_mouse_eligible(&self) -> bool {
        self.app.effective_panel_mode() == PanelMode::Both && self.panel_mouse_eligible()
    }

    pub(super) fn sync_queue_boundary(&mut self) {
        let id = ComponentId::QueueBoundary;
        let area = self.app.layout.main.queue_boundary_area;
        let enabled = self.queue_boundary_mouse_eligible() && area.width == 1 && area.height > 0;
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(boundary) = comp
                .as_any_mut()
                .downcast_mut::<super::components::QueueBoundaryComponent>()
            {
                boundary.sync(
                    area,
                    self.app.layout.main.left_area.x,
                    self.app.terminal_width,
                    self.app.queue_column_width,
                    matches!(self.app.effective_panel_focus(), PanelFocus::Queue),
                    enabled,
                );
            }
        }
    }

    /// The active Wide hero surface's content area when it is painting its
    /// two-pane split this frame, else `None`.
    ///
    /// One central tab→area match (design.md Decision: "One component, one
    /// tab→area match with painted-split eligibility"). Each arm reads the
    /// rect `layout.rs` published for that surface and carries the surface's
    /// own wide-paint gate: breakpoint fit alone is not enough, because
    /// empty/loading/no-selection states return before painting the hero pane
    /// and would otherwise expose a grab zone over an unsplit frame.
    pub(super) fn wide_hero_boundary_content_area(&self) -> Option<Rect> {
        if !self.library_panel_visible() {
            return None;
        }
        match self.app.tab {
            TabSelection::Home => {
                let area = self.app.layout.main.home_area;
                crate::app::render::wide_hero_fits(area).then_some(area)
            }
            TabSelection::Feeds => {
                let area = self.app.layout.main.feeds_area;
                let state = &self.app.feed_tab;
                (crate::app::render::wide_hero_fits(area)
                    && !state.subscriptions.is_empty()
                    && !state.all_entries.is_empty())
                .then_some(area)
            }
            TabSelection::AudiobookshelfLibrary(index) => self.abs_wide_hero_content_area(index),
            TabSelection::EmbyLibrary(index) => self.emby_wide_hero_content_area(index),
        }
    }

    fn abs_wide_hero_content_area(&self, index: usize) -> Option<Rect> {
        match self.app.audiobookshelf_kind_at(index)? {
            AudiobookshelfBrowseKind::Book => {
                let area = self.app.layout.main.audiobookshelf_book_area;
                // The empty/loading early return skips the wide branch.
                let has_books = self
                    .app
                    .audiobookshelf_book_browse
                    .get(index)
                    .is_some_and(|state| !state.books.is_empty());
                (has_books && crate::app::render::wide_hero_fits(area)).then_some(area)
            }
            // Podcast paints its hero pane whenever the breakpoint fits, even
            // with no shows (the empty placeholder is painted in the rail).
            AudiobookshelfBrowseKind::Podcast => {
                let area = self.app.layout.main.audiobookshelf_podcast_area;
                crate::app::render::wide_hero_fits(area).then_some(area)
            }
        }
    }

    fn emby_wide_hero_content_area(&self, index: usize) -> Option<Rect> {
        let library = self.app.libs.get(index)?;
        let kind = BrowserKind::from_collection_type(&library.library.collection_type);
        match kind {
            // Wide TV (series list) paints the two-pane workspace; narrow TV
            // is `BrowserComponent` and paints no split.
            BrowserKind::TvShows => self
                .app
                .wide_tv_library_area(index)
                .map(|_| self.app.layout.main.tv_wide_area),
            BrowserKind::Music => (self.app.is_music_group_view(index)
                && self.app.is_viewing_album_folders(index))
            .then_some(self.app.layout.main.wide_music_area)
            .filter(|area| crate::app::render::wide_hero_fits(*area)),
            BrowserKind::Movies | BrowserKind::HomeVideos => {
                let area = self.app.layout.main.left_area;
                crate::app::render::wide_hero_fits(area).then_some(area)
            }
            // Generic libraries never paint a wide split; only the
            // feed-home-video group picker (rendered by `BrowserComponent`)
            // does.
            BrowserKind::Generic => self
                .app
                .is_feed_home_video_group_view(index)
                .then_some(self.app.layout.main.left_area)
                .filter(|area| crate::app::render::wide_hero_fits(*area)),
            BrowserKind::AudiobookshelfPodcast | BrowserKind::AudiobookshelfBook => None,
        }
    }

    /// The gap columns and resolution inputs shared by the boundary's arming
    /// (`sync_wide_hero_boundary`) and painting
    /// (`render_wide_hero_boundary`), or `None` when no split is painted.
    fn wide_hero_boundary_geometry(&self) -> Option<WideHeroBoundaryGeometry> {
        let content_area = self.wide_hero_boundary_content_area()?;
        let panes =
            crate::app::render::wide_library_panes(content_area, 0, 0, self.app.list_pane_width)?;
        let gap = Rect {
            x: panes.browser_panel.right(),
            y: content_area.y,
            width: panes
                .hero_panel
                .x
                .saturating_sub(panes.browser_panel.right()),
            height: content_area.height,
        };
        (gap.width > 0 && gap.height > 0).then_some(WideHeroBoundaryGeometry {
            gap,
            pane_origin_x: content_area.x,
            content_width: content_area.width,
            list_pane_width: panes.browser_panel.width,
        })
    }

    /// The gap rect the active surface paints this frame, if any. Shared with
    /// the width-resolution test path.
    #[cfg(test)]
    pub(super) fn wide_hero_boundary_gap_rect(&self) -> Option<Rect> {
        self.wide_hero_boundary_geometry()
            .map(|geometry| geometry.gap)
    }

    /// Whether the Wide hero boundary gap may receive mouse input. Shared by
    /// `mouse_eligible_ids` (subscription) and `sync_wide_hero_boundary`
    /// (arming) so the two can never disagree.
    pub(super) fn wide_hero_boundary_mouse_eligible(&self) -> bool {
        self.panel_mouse_eligible() && self.wide_hero_boundary_geometry().is_some()
    }

    pub(super) fn sync_wide_hero_boundary(&mut self) {
        let id = ComponentId::WideHeroBoundary;
        let geometry = self.wide_hero_boundary_geometry();
        let enabled = self.panel_mouse_eligible() && geometry.is_some();
        let (area, pane_origin_x, content_width, width) = match geometry {
            Some(geometry) => (
                geometry.gap,
                geometry.pane_origin_x,
                geometry.content_width,
                geometry.list_pane_width,
            ),
            None => (Rect::default(), 0, 0, 0),
        };
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(boundary) = comp
                .as_any_mut()
                .downcast_mut::<super::components::WideHeroBoundaryComponent>()
            {
                boundary.sync(area, pane_origin_x, content_width, width, enabled);
            }
        }
    }

    /// Paint the Wide hero gap columns with the backdrop they already showed.
    pub(super) fn render_wide_hero_boundary(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::WideHeroBoundary;
        if !self.application.mounted(&id) {
            return;
        }
        let area = self
            .wide_hero_boundary_geometry()
            .map_or(Rect::default(), |geometry| geometry.gap);
        self.application.view(&id, frame, area);
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
