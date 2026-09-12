//! The Library panel's shell integration (task 5.9, design D2). The mounted
//! `LibraryPanel` is the library area's event boundary; the shell drives it
//! one-way: it mounts/unmounts it with the library column, points it at the
//! active library's owner, retains owners while their libraries are in the
//! catalog (the rule moved inside from `reconcile_destination_mounts`), and
//! hands it the session-only split width. The transitional branch lives in
//! the draw path: `RootFrame` gives the library rect to the panel only while
//! the active library's owner has migrated — otherwise the old mounted
//! destination stays the surface.

use ratatui::layout::Rect;

use super::components::library_panel::content::HeroImageState;
use super::components::library_panel::{LibraryContentOwner, LibraryKey, LibraryPanel};
use super::components::{BrowserKey, BrowserKind, ComponentId, MusicWorkspaceComponent};
use super::shell::Model;
use super::{PanelMode, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// The active library's [`LibraryKey`] from the resolved tab: the owner
    /// map's addressing key (design D2: `Home | Feeds | Service(BrowserKey)`).
    pub(super) fn active_library_key(&self) -> Option<LibraryKey> {
        match self.app.tab {
            TabSelection::Home => Some(LibraryKey::Home),
            TabSelection::Feeds => Some(LibraryKey::Feeds),
            TabSelection::EmbyLibrary(index) => {
                let library = self.app.libs.get(index)?;
                Some(LibraryKey::Service(BrowserKey {
                    service: ServiceKind::Emby,
                    library_id: library.library.id.clone(),
                    kind: BrowserKind::from_collection_type(&library.library.collection_type),
                }))
            }
            TabSelection::AudiobookshelfLibrary(index) => {
                let library = self.app.audiobookshelf_libraries.get(index)?;
                let kind = match self.app.audiobookshelf_kind_at(index)? {
                    AudiobookshelfBrowseKind::Podcast => BrowserKind::AudiobookshelfPodcast,
                    AudiobookshelfBrowseKind::Book => BrowserKind::AudiobookshelfBook,
                };
                Some(LibraryKey::Service(BrowserKey {
                    service: ServiceKind::Audiobookshelf,
                    library_id: library.id.clone(),
                    kind,
                }))
            }
        }
    }

    /// Every [`LibraryKey`] currently in the catalog: the shell tabs that are
    /// always live (Home, Feeds) plus one `Service` key per configured
    /// library, independent of the active tab. `LibraryPanel::retain_owners`
    /// drops owners whose key is not here (design D2's retention rule).
    pub(super) fn live_library_keys(&self) -> Vec<LibraryKey> {
        let mut keys = vec![LibraryKey::Home, LibraryKey::Feeds];
        keys.extend(self.app.libs.iter().map(|tab| {
            LibraryKey::Service(super::components::BrowserKey {
                service: ServiceKind::Emby,
                library_id: tab.library.id.clone(),
                kind: BrowserKind::from_collection_type(&tab.library.collection_type),
            })
        }));
        keys.extend(self.app.audiobookshelf_libraries.iter().map(|library| {
            // `from_media_type` maps every ABS media type to exactly one of
            // Book | Podcast (same rule the browse surfaces apply).
            let kind = match AudiobookshelfBrowseKind::from_media_type(&library.media_type) {
                AudiobookshelfBrowseKind::Podcast => BrowserKind::AudiobookshelfPodcast,
                AudiobookshelfBrowseKind::Book => BrowserKind::AudiobookshelfBook,
            };
            LibraryKey::Service(BrowserKey {
                service: ServiceKind::Audiobookshelf,
                library_id: library.id.clone(),
                kind,
            })
        }));
        keys
    }

    /// The mounted panel's migrated-owner check for the active library: the
    /// transitional branch's one condition.
    pub(super) fn active_library_owner_migrated(&self) -> bool {
        let Some(key) = self.active_library_key() else {
            return false;
        };
        self.library_panel_has_owner(&key)
    }

    /// Whether the panel hosts an owner for `key`.
    pub(super) fn library_panel_has_owner(&self, key: &LibraryKey) -> bool {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .is_some_and(|panel| panel.has_owner(key))
    }

    /// Push one content owner into the panel, addressed by `LibraryKey`
    /// (design D2). Production callers are the per-destination conversion
    /// slices (tasks 5.11+); the test harness pushes fixture owners to prove
    /// the mount/focus/mouse/retention wiring ahead of any conversion.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn push_library_owner(
        &mut self,
        key: LibraryKey,
        owner: Box<dyn LibraryContentOwner>,
    ) {
        if let Some(panel) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            panel.insert_owner(key, owner);
        }
    }

    /// Mount the `LibraryPanel` with the library column (design D1's mount
    /// rule for the Library panel), drive its owner map — the active pointer
    /// and the catalog-retention rule — and push the session split width. The
    /// panel paints only a migrated owner; with zero migrated owners the old
    /// destination stays the surface, so this never makes the panel claim a
    /// surface it does not paint.
    ///
    /// D1 mount-rule exception (design D2): D1 would unmount the panel when
    /// the library column hides, but the owners live inside it, and an
    /// unmount would destroy every owner's cursor/scroll/focus/drafts — the
    /// retention D2 grants while the library is in the catalog. Once mounted
    /// the panel therefore stays mounted across Panel modes (mounted ≠
    /// painted): `render_library_panel` views it only with a real placement
    /// and a migrated active owner, and the mouse eligibility ladder
    /// subscribes it only while the library column is visible, so a hidden
    /// panel never paints or claims input.
    pub(super) fn sync_library_panel(&mut self) {
        let id = ComponentId::Library;
        if self.library_panel_visible() && !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(LibraryPanel::new()), vec![])
                .expect("mount LibraryPanel");
        }
        let active = self.active_library_key();
        let live = self.live_library_keys();
        let list_pane_width = self.app.list_pane_width;
        // The split gesture's eligibility-loss reset is decided before the
        // panel borrow (mirrors `WideHeroBoundaryComponent::sync`, which the
        // panel's split drag moved in from): an overlay mount mid-drag must
        // not leave a stale armed drag behind.
        let mouse_eligible = self.panel_mouse_eligible();
        let Some(panel) = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        else {
            return;
        };
        // Owner retention while the library is in the catalog (design
        // D2): the rule `reconcile_destination_mounts` applies to the old
        // destination components, moved inside for the panel's owners. Driven
        // in every Panel mode so a hidden library's owners keep their state.
        panel.retain_owners(&live);
        panel.set_active(active);
        panel.set_list_pane_width(list_pane_width);
        panel.sync_mouse_eligibility(mouse_eligible);
    }

    /// The library content rect the old destinations paint — the same
    /// derivation `render_main` applies to `RootFrame.library` — so the
    /// panel's transitional paint and the old path never disagree about the
    /// surface's extent.
    pub(super) fn library_panel_content_area(&self) -> Option<Rect> {
        let area = self.app.layout.root_frame.library?;
        let collapsed = self.app.effective_panel_mode() != PanelMode::Both;
        Some(crate::app::render::components::widgets::right_panel_content_area(area, collapsed))
    }

    /// The transitional draw step: give the library rect to the mounted
    /// `LibraryPanel` when the active library's owner has migrated; otherwise
    /// the old destination components paint (the caller gates them).
    pub(super) fn render_library_panel(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Library;
        if !self.application.mounted(&id) || !self.active_library_owner_migrated() {
            return;
        }
        let Some(area) = self.library_panel_content_area() else {
            return;
        };
        self.application.view(&id, frame, area);
        // The projected hero image's pixel paint (task 5.10, design D9): the
        // painters read projected state and reserve the box; the shell paints
        // the cached protocol into it right after view returns — the same
        // defer-the-pixel-paint seam every destination component uses.
        let image_paint = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(LibraryPanel::take_image_paint);
        if let Some(paint) = image_paint {
            self.app.paint_panel_hero_image(frame, &paint);
        }
    }

    /// The library heroes' image projection (task 5.10, design D9, TV's
    /// `push_tv_workspace_content` shape generalized): for the active
    /// migrated owner's current hero, run the artwork policy's source and the
    /// paint-free `hero_artwork_box` with the Library panel's `RootFrame`
    /// area, issue every `fetch_card_image` here, and project the image
    /// state (loading/ready, decoded size, cover-fit box keyed by size)
    /// into the owner. Painting reads the projected state only; the queue
    /// visual slot's separate projection (task 3.4) is untouched.
    /// Driven every sync pass so a cursor move, breakpoint change, or split
    /// drag re-projects on the next pass.
    pub(super) fn sync_library_hero_images(&mut self) {
        if !self.library_panel_visible() {
            return;
        }
        let Some(area) = self.library_panel_content_area() else {
            return;
        };
        let list_pane_width = self.app.list_pane_width;
        // Music is still mounted as its legacy destination boundary in this
        // slice, but its Wide view now paints the shared skeleton. Project its
        // Square hero through the same shell-owned path as registered panel
        // owners; Narrow continues to use its legacy image painter.
        if !self.active_library_owner_migrated() {
            if crate::app::render::wide_hero_fits(area) {
                if let Some(id) = self.music_workspace_component_id() {
                    let hero_data = self
                        .application
                        .get_component_mut(&id)
                        .and_then(|component| {
                            component
                                .as_any_mut()
                                .downcast_mut::<MusicWorkspaceComponent>()
                        })
                        .and_then(MusicWorkspaceComponent::hero_data);
                    if let Some(data) = hero_data {
                        let state =
                            self.app
                                .project_hero_image(&data.facts, true, area, list_pane_width);
                        if let Some(music) =
                            self.application
                                .get_component_mut(&id)
                                .and_then(|component| {
                                    component
                                        .as_any_mut()
                                        .downcast_mut::<MusicWorkspaceComponent>()
                                })
                        {
                            music.set_hero_image(state);
                        }
                    }
                }
            }
            return;
        }
        let hero_data = {
            let panel = self
                .application
                .get_component_mut(&ComponentId::Library)
                .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>());
            match panel.and_then(LibraryPanel::active_hero_data) {
                Some(data) => data,
                None => {
                    // No hero this frame: the placeholder is final.
                    if let Some(panel) = self
                        .application
                        .get_component_mut(&ComponentId::Library)
                        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
                    {
                        panel.set_active_hero_image(HeroImageState::None);
                    }
                    return;
                }
            }
        };
        let state = self
            .app
            .project_hero_image(&hero_data.facts, false, area, list_pane_width);
        if let Some(panel) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            panel.set_active_hero_image(state);
        }
    }
}

use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
