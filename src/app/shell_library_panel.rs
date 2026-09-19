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

use super::components::book_content::BookContent;
use super::components::library_panel::content::HeroImageState;
use super::components::library_panel::{LibraryContentOwner, LibraryPanel};
use super::components::podcast_content::PodcastContent;
use super::components::{ComponentId, LibraryKey, LibraryKind};
use super::shell::Model;
use super::{PanelMode, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// The active library's [`LibraryKey`] from the resolved tab: the owner
    /// map's addressing key (design D2: `Home | Feeds | Service(LibraryKey)`).
    pub(super) fn active_library_key(&self) -> Option<LibraryKey> {
        match self.app.tab {
            TabSelection::Home => Some(LibraryKey::Home),
            TabSelection::Feeds => Some(LibraryKey::Feeds),
            TabSelection::EmbyLibrary(index) => {
                let library = self.app.libs.get(index)?;
                Some(LibraryKey::Service {
                    service: ServiceKind::Emby,
                    library_id: library.library.id.clone(),
                    kind: LibraryKind::from_collection_type(&library.library.collection_type),
                })
            }
            TabSelection::AudiobookshelfLibrary(index) => {
                let library = self.app.audiobookshelf_libraries.get(index)?;
                let kind = match self.app.audiobookshelf_kind_at(index)? {
                    AudiobookshelfBrowseKind::Podcast => LibraryKind::AudiobookshelfPodcast,
                    AudiobookshelfBrowseKind::Book => LibraryKind::AudiobookshelfBook,
                };
                Some(LibraryKey::Service {
                    service: ServiceKind::Audiobookshelf,
                    library_id: library.id.clone(),
                    kind,
                })
            }
        }
    }

    /// The active library's stable selection origin (design D6/D7): the
    /// identity a projection or delayed action carries so a clear intent can
    /// route to the list that produced it, never the dispatch-time focus.
    pub(super) fn active_library_selection_origin(
        &self,
    ) -> Option<crate::app::components::media_list::SelectionOrigin> {
        self.active_library_key().map(|key| {
            crate::app::components::media_list::SelectionOrigin::Library(
                crate::app::components::media_list::LibrarySelectionOrigin::from(key),
            )
        })
    }

    /// Every [`LibraryKey`] currently in the catalog: the shell tabs that are
    /// always live (Home, Feeds) plus one `Service` key per configured
    /// library, independent of the active tab. `LibraryPanel::retain_owners`
    /// drops owners whose key is not here (design D2's retention rule).
    pub(super) fn live_library_keys(&self) -> Vec<LibraryKey> {
        let mut keys = vec![LibraryKey::Home, LibraryKey::Feeds];
        keys.extend(self.app.libs.iter().map(|tab| LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: tab.library.id.clone(),
            kind: LibraryKind::from_collection_type(&tab.library.collection_type),
        }));
        keys.extend(self.app.audiobookshelf_libraries.iter().map(|library| {
            // `from_media_type` maps every ABS media type to exactly one of
            // Book | Podcast (same rule the browse surfaces apply).
            let kind = match AudiobookshelfBrowseKind::from_media_type(&library.media_type) {
                AudiobookshelfBrowseKind::Podcast => LibraryKind::AudiobookshelfPodcast,
                AudiobookshelfBrowseKind::Book => LibraryKind::AudiobookshelfBook,
            };
            LibraryKey::Service {
                service: ServiceKind::Audiobookshelf,
                library_id: library.id.clone(),
                kind,
            }
        }));
        keys
    }

    /// Whether the panel hosts an owner for `key`.
    pub(super) fn library_panel_has_owner(&self, key: &LibraryKey) -> bool {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .is_some_and(|panel| panel.has_owner(key))
    }

    /// Typed immutable access to the panel's owner for `key`: the shared
    /// lookup/downcast path every per-content-type `*_owner` reader collapses
    /// to (design D2 — one panel, many typed owners).
    pub(super) fn library_owner<T: LibraryContentOwner + 'static>(
        &self,
        key: &LibraryKey,
    ) -> Option<&T> {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.owner(key))
            .and_then(|owner| owner.as_any().downcast_ref::<T>())
    }

    /// Typed mutable access to the panel's owner for `key`.
    pub(super) fn library_owner_mut<T: LibraryContentOwner + 'static>(
        &mut self,
        key: &LibraryKey,
    ) -> Option<&mut T> {
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|panel| panel.owner_mut(key))
            .and_then(|owner| owner.as_any_mut().downcast_mut::<T>())
    }

    /// Mutate the owner for `key`, creating it via `make` on first reach: the
    /// shared create-if-absent path every per-content-type `update_*_owner`
    /// collapses to.
    pub(super) fn update_library_owner<T: LibraryContentOwner + 'static, R>(
        &mut self,
        key: LibraryKey,
        make: impl FnOnce() -> Box<T>,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        if !self.library_panel_has_owner(&key) {
            self.push_library_owner(key.clone(), make());
        }
        self.library_owner_mut(&key).map(f)
    }

    /// Open the active owner's Hero through the Library panel's local overlay
    /// contract. Destination actions use this instead of constructing the
    /// Library Hero overlay.
    pub(super) fn open_library_hero_overlay(&mut self) -> bool {
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .is_some_and(LibraryPanel::open_hero_overlay_for_active)
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
        // Content normally arrives at its event writers. Seed only a newly
        // visited Emby owner after mounting; re-projecting every tick clones
        // and groups the whole library during each live resize.
        let active = self.active_library_key();
        if active
            .as_ref()
            .is_some_and(|key| !self.library_panel_has_owner(key))
        {
            self.push_active_emby_library_owner_content();
            self.push_music_workspace_content();
        }
        // Register the Books owner as part of panel/catalog reconciliation,
        // not from the Books content projection. A newly active Books tab is
        // then populated by this discrete registration hand-off.
        let register_book = active.as_ref().is_some_and(|key| {
            matches!(
                key,
                LibraryKey::Service {
                    service: ServiceKind::Audiobookshelf,
                    kind: LibraryKind::AudiobookshelfBook,
                    ..
                }
            ) && !self.library_panel_has_owner(key)
        });
        if register_book {
            if let Some(key) = active.clone() {
                self.push_library_owner(key, Box::new(BookContent::new()));
                self.push_audiobookshelf_book_content();
            }
        }
        let register_podcast = active.as_ref().is_some_and(|key| {
            matches!(
                key,
                LibraryKey::Service {
                    service: ServiceKind::Audiobookshelf,
                    kind: LibraryKind::AudiobookshelfPodcast,
                    ..
                }
            ) && !self.library_panel_has_owner(key)
        });
        if register_podcast {
            if let Some(key) = active.clone() {
                self.push_library_owner(key, Box::new(PodcastContent::new()));
                self.push_audiobookshelf_podcast_content();
            }
        }
        let live = self.live_library_keys();
        let list_pane_width = self.app.list_pane_width;
        // The split gesture's eligibility-loss reset is decided before the
        // panel borrow (the panel owns the split gesture state, so the
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
        panel.sync_overlay_state();
        panel.set_list_pane_width(list_pane_width);
        panel.set_terminal_height(self.app.terminal_height);
        panel.sync_mouse_eligibility(mouse_eligible);
    }

    /// The library content rect painted by the mounted panel.
    pub(super) fn library_panel_content_area(&self) -> Option<Rect> {
        let area = self.app.layout.root_frame.library?;
        let collapsed = self.app.effective_panel_mode() != PanelMode::Both;
        Some(crate::app::render::components::widgets::right_panel_content_area(area, collapsed))
    }

    /// The Library column's body fill: the column's fixed backdrop in every
    /// geometry and every focus state. The non-Wide panel is the Wide browser
    /// pane without a Hero, so its body no longer follows the panel focus bit
    /// and no non-Wide surface identity exists — the single named authority
    /// both the placement fill and the status band's padding rows read.
    pub(super) fn library_body_fill(&self) -> ratatui::style::Color {
        crate::app::palette::surface_colors(crate::app::palette::Surface::LibraryColumn, false).fill
    }

    /// The transitional draw step: give the library rect to the mounted
    /// `LibraryPanel` when the active library's owner has migrated; otherwise
    /// the old destination components paint (the caller gates them).
    pub(super) fn render_library_panel_at(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let id = ComponentId::Library;
        if !self.application.mounted(&id) {
            return;
        }
        // Fill the placement's own background first (task 12.2): the panel
        // paints a narrower inset content rect (`library_panel_content_area`
        // insets by `TAB_LEFT_PAD`/one column, matching the tab bar's own
        // left indent), so the margin columns outside that inset must still
        // show the column's own background rather than whatever was painted
        // underneath before this panel owned the placement.
        frame.render_widget(ratatui::widgets::Clear, area);
        // The panel body is the library column's fixed backdrop in every
        // geometry and focus state: the non-Wide panel is the Wide browser
        // pane without a Hero, so one backdrop serves both skeletons.
        let content_area = self.library_panel_content_area().unwrap_or(area);
        frame.render_widget(
            ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.library_body_fill())),
            area,
        );
        self.application.view(&id, frame, content_area);
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
        let overlay_box = self
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(LibraryPanel::active_hero_image_box);
        let state = self.app.project_hero_image(
            &hero_data.facts,
            false,
            area,
            list_pane_width,
            overlay_box,
        );
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
