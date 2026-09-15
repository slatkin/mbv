//! The Library panel's embedded content-owner contract (tasks 5.7–5.9,
//! design D2/D3). A content owner is a plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps its Service content,
//! media lists, cursor/scroll/focus state, and typed intent translation. The
//! mounted [`super::panel::LibraryPanel`] is the library area's one event
//! boundary: it resolves pointer input against its own painted slot geometry
//! and hands the active owner semantic slot events, which the owner
//! translates into its existing typed `Msg`s.

use std::any::Any;
use std::collections::HashMap;

use tuirealm::event::KeyEvent;

use crate::app::components::media_list::{MediaListSurfaceInput, SelectionSummary};
use crate::app::components::msg::{LeafKeyResult, Msg};
use mbv_core::config::ServiceKind;

use super::content::{HeroImageState, LibraryPanelContent};
use super::hero::HeroContentData;

/// The behavioural category of one service library.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LibraryKind {
    Generic,
    Movies,
    TvShows,
    Music,
    HomeVideos,
    AudiobookshelfPodcast,
    AudiobookshelfBook,
}

impl LibraryKind {
    pub fn from_collection_type(collection_type: &str) -> Self {
        match collection_type {
            "movies" => Self::Movies,
            "tvshows" => Self::TvShows,
            "music" => Self::Music,
            "homevideos" => Self::HomeVideos,
            _ => Self::Generic,
        }
    }
}

#[cfg(test)]
mod library_kind_tests {
    use super::LibraryKind;

    #[test]
    fn maps_known_collection_types() {
        assert_eq!(
            LibraryKind::from_collection_type("movies"),
            LibraryKind::Movies
        );
        assert_eq!(
            LibraryKind::from_collection_type("tvshows"),
            LibraryKind::TvShows
        );
        assert_eq!(
            LibraryKind::from_collection_type("music"),
            LibraryKind::Music
        );
        assert_eq!(
            LibraryKind::from_collection_type("homevideos"),
            LibraryKind::HomeVideos
        );
    }

    #[test]
    fn unrecognized_collection_types_fall_back_to_generic() {
        assert_eq!(
            LibraryKind::from_collection_type("boxsets"),
            LibraryKind::Generic
        );
        assert_eq!(
            LibraryKind::from_collection_type("mixed"),
            LibraryKind::Generic
        );
        assert_eq!(LibraryKind::from_collection_type(""), LibraryKind::Generic);
    }
}

/// The identity of one library destination, keying the panel's owner map.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LibraryKey {
    Home,
    Feeds,
    Service {
        service: ServiceKind,
        library_id: String,
        kind: LibraryKind,
    },
}

/// A semantic slot event the panel resolved from pointer input against its
/// own painted geometry (design D3). `List` delegation is already normalized
/// row-local input — the owner performs the typed point resolution through
/// its own carrier, exactly as a mounted destination does today; no raw
/// event crosses to the shell for re-resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum LibrarySlotEvent {
    /// A Selector-row pill was picked (its painted index).
    SelectorPicked(usize),
    /// A List-controls pill was picked.
    ControlPicked(usize),
    /// A Workspace selector pill was picked.
    WorkspaceSelectorPicked(usize),
    /// An already-normalized row-local input for the active owner's list.
    List(MediaListSurfaceInput),
    /// An already-normalized pointer input inside the hero pane, delivered
    /// when no painted pill row or list slot claimed it. The panel resolves
    /// the pointer against the hero pane rect it painted; the owner decides
    /// whether the point lies in its Workspace list (resolving the row's
    /// stable target through its own carrier), in the pane but off every
    /// row, or nowhere it claims.
    HeroPane(MediaListSurfaceInput),
}

/// The embedded content owner contract: one producer per frame plus the slot
/// event translation. Object-safe so the panel can host owners for every
/// library in one map.
pub(in crate::app) trait LibraryContentOwner {
    /// This frame's panel content, borrowed from the owner's own media lists
    /// and search session. The panel borrows the owner and its content
    /// together inside `view` (design D3: no list is copied or mirrored).
    fn content(&mut self) -> LibraryPanelContent<'_>;

    /// Translate one resolved slot event into the owner's existing typed
    /// `Msg`s (design D2). `None` when the owner claims nothing for it.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg>;

    /// Handle one already-routed chord with an explicit leaf disposition.
    /// Legacy compatibility for direct component tests; mounted routing uses
    /// `on_key_result` implementations below.
    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let _ = key;
        None
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        self.on_key(key)
            .map(|message| LeafKeyResult::Consumed(Some(message)))
            .unwrap_or(LeafKeyResult::Unhandled)
    }

    fn hero_overlay_available(&mut self) -> bool {
        self.content().hero.is_some()
    }

    fn inline_search_active(&self) -> bool {
        false
    }

    fn focus_hero_workspace(&mut self) -> bool {
        false
    }

    /// Release the destination-local workspace focus acquired for the Hero
    /// overlay, without changing its retained cursor or scroll.
    fn clear_hero_workspace_focus(&mut self) {}

    /// Activate the currently selected Hero target through the owner's typed
    /// destination intent. Pointer gestures call this semantic operation, not
    /// a fabricated keyboard event.
    fn activate_hero_selection(&mut self) -> LeafKeyResult {
        // Destination owners already define the Enter activation semantics;
        // this semantic hook keeps pointer delivery out of the panel's key
        // interpreter while allowing unmigrated owners to opt in naturally.
        self.on_key_result(&KeyEvent::new(
            tuirealm::event::Key::Enter,
            tuirealm::event::KeyModifiers::NONE,
        ))
    }

    /// The current hero's content data for the shell's image projection
    /// (task 5.10, design D9), or `None` when the owner shows no hero. The
    /// projection runs the artwork box and the fetch; painting reads the
    /// projected state only.
    fn hero_data(&mut self) -> Option<HeroContentData> {
        None
    }

    /// Receive the projection's image state for the current hero (task
    /// 5.10, design D9).
    fn set_hero_image(&mut self, _state: HeroImageState) {}

    /// Clear local multi-selection when the panel activates a different
    /// destination identity. Overlay activation never calls this.
    fn clear_selection(&mut self) {}

    /// Read-only focused-list projection for the Status bar. The owner never
    /// exposes membership; the panel caches only this summary.
    fn selection_summary(&self) -> Option<SelectionSummary> {
        None
    }

    fn set_selection_origin(
        &mut self,
        _origin: crate::app::components::media_list::SelectionOrigin,
    ) {
    }

    /// The owner-resolved cursor and resting scroll after local movement.
    /// `None` is used by owners whose position is not persisted by App.
    fn scroll_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn hero_scroll_offset(&self) -> usize {
        0
    }
    fn hero_scroll(&mut self, _delta: i16, _max_offset: usize) -> bool {
        false
    }

    /// Downcast support for the shell's per-destination pushes (the shell
    /// projects destination-specific content into a typed owner it knows
    /// by name, addressed through the panel's `LibraryKey` map).
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// The shared-borrow twin of [`LibraryContentOwner::as_any_mut`] for
    /// the shell's pure reads (section identity, painted geometry).
    fn as_any(&self) -> &dyn Any;
}

/// The panel's owner map (design D2): every embedded content owner keyed by
/// its [`LibraryKey`], retained while the library is in the catalog. Not a
/// component registry — the owners are plain types the panel borrows for
/// content and slot events.
#[derive(Default)]
pub(in crate::app) struct LibraryOwners {
    owners: HashMap<LibraryKey, Box<dyn LibraryContentOwner>>,
    /// The active library's key, driven by the shell's tab resolution each
    /// sync pass. The pointer to the painted/event-bound owner; not a second
    /// store of owner state.
    active: Option<LibraryKey>,
}

impl LibraryOwners {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install or replace the owner for `key`. The shell pushes owners as
    /// their destinations migrate (task 5.11+); the push is addressed by
    /// `LibraryKey`, never by a destination component.
    pub fn insert(&mut self, key: LibraryKey, owner: Box<dyn LibraryContentOwner>) {
        self.owners.insert(key, owner);
    }

    /// Whether an owner exists for `key` (the transitional branch's
    /// migrated-owner check).
    pub fn has(&self, key: &LibraryKey) -> bool {
        self.owners.contains_key(key)
    }

    /// Drop every owner whose key is not in `live` — the catalog-retention
    /// rule (moved inside from the shell's `reconcile_destination_mounts`,
    /// design D2): an owner is retained while its library is in the catalog.
    pub fn retain(&mut self, live: &[LibraryKey]) {
        self.owners.retain(|key, _| live.contains(key));
    }

    /// The owner currently painted and event-bound, or `None`.
    pub fn active_mut(&mut self) -> Option<&mut Box<dyn LibraryContentOwner>> {
        self.owners.get_mut(self.active.as_ref()?)
    }

    /// The owner installed for `key`, whether active or not (the shell's
    /// destination-specific pushes reach inactive owners too — an inactive
    /// owner's content is refreshed while its library is in the catalog).
    pub fn get_mut(&mut self, key: &LibraryKey) -> Option<&mut dyn LibraryContentOwner> {
        match self.owners.get_mut(key) {
            Some(owner) => Some(owner.as_mut()),
            None => None,
        }
    }

    /// The shared-borrow twin of [`LibraryOwners::get_mut`].
    pub fn get(&self, key: &LibraryKey) -> Option<&dyn LibraryContentOwner> {
        self.owners.get(key).map(|owner| &**owner)
    }

    /// The active owner's key, for the shell's transitional branch.
    pub fn active_key(&self) -> Option<&LibraryKey> {
        self.active.as_ref()
    }

    /// Point the panel at the active library's owner (the shell drives this
    /// from its tab resolution each sync pass).
    pub fn set_active(&mut self, key: Option<LibraryKey>) {
        self.active = key;
    }
}
