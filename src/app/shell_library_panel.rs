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

use super::components::library_panel::{LibraryContentOwner, LibraryKey, LibraryPanel};
use super::components::{BrowserKey, BrowserKind, ComponentId};
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

    /// Mount/unmount the `LibraryPanel` to the library column's visibility
    /// (design D1's mount rule for the Library panel), drive its owner map —
    /// the active pointer and the catalog-retention rule — and push the
    /// session split width. The panel paints only a migrated owner; with
    /// zero migrated owners the old destination stays the surface, so this
    /// never makes the panel claim a surface it does not paint.
    pub(super) fn sync_library_panel(&mut self) {
        let id = ComponentId::Library;
        if !self.library_panel_visible() {
            if self.application.mounted(&id) {
                let _ = self.application.umount(&id);
            }
            return;
        }
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(LibraryPanel::new()), vec![])
                .expect("mount LibraryPanel");
        }
        let active = self.active_library_key();
        let live = self.live_library_keys();
        let list_pane_width = self.app.list_pane_width;
        if let Some(panel) = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            // Owner retention while the library is in the catalog (design
            // D2): the rule `reconcile_destination_mounts` applies to the old
            // destination components, moved inside for the panel's owners.
            panel.retain_owners(&live);
            panel.set_active(active);
            panel.set_list_pane_width(list_pane_width);
        }
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
    }
}

use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
