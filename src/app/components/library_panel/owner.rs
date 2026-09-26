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

use crate::app::components::inline_search::InlineSearchHost;
use crate::app::components::media_list::{MediaListSurfaceInput, SelectionSummary};
use crate::app::components::msg::{LeafKeyResult, Msg};
use mbv_core::config::{
    LibraryItemIdentity, SelectorIdentity, ServiceKind, TabIdentity, TuiLaunchState,
};

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

    /// Task 2.1: every `LibraryKey` maps to a stable launch-state tab
    /// identity. Fixed tabs map to themselves; every `LibraryKind`
    /// maps through with its Service kind plus library ID intact (the
    /// browse kind selects the pill/item interpretation, never the tab).
    /// Uses `super::*` so the mapping test reads the same names as the
    /// contract it pins.
    #[test]
    fn tab_identity_covers_every_key_shape() {
        use super::*;

        assert_eq!(LibraryKey::Home.tab_identity(), TabIdentity::Home);
        assert_eq!(LibraryKey::Feeds.tab_identity(), TabIdentity::Feeds);
        for kind in [
            LibraryKind::Generic,
            LibraryKind::Movies,
            LibraryKind::TvShows,
            LibraryKind::Music,
            LibraryKind::HomeVideos,
            LibraryKind::AudiobookshelfPodcast,
            LibraryKind::AudiobookshelfBook,
        ] {
            assert_eq!(
                LibraryKey::Service {
                    service: ServiceKind::Emby,
                    library_id: "lib-1".to_string(),
                    kind,
                }
                .tab_identity(),
                TabIdentity::ServiceLibrary {
                    kind: ServiceKind::Emby,
                    library_id: "lib-1".to_string(),
                },
                "kind {kind:?} must map through"
            );
        }
        assert_eq!(
            LibraryKey::Service {
                service: ServiceKind::Audiobookshelf,
                library_id: "abs-lib".to_string(),
                kind: LibraryKind::AudiobookshelfPodcast,
            }
            .tab_identity(),
            TabIdentity::ServiceLibrary {
                kind: ServiceKind::Audiobookshelf,
                library_id: "abs-lib".to_string(),
            }
        );
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

impl LibraryKey {
    /// The stable launch-state tab identity for this destination (task 2.1).
    /// Every [`LibraryKind`] maps through — the tab identity carries only
    /// the Service kind plus library ID, so the browse kind selects the
    /// destination-tagged pill/item interpretation, never the tab.
    // Consumed by selected-tab teardown assembly in task 2.3.
    pub fn tab_identity(&self) -> TabIdentity {
        match self {
            Self::Home => TabIdentity::Home,
            Self::Feeds => TabIdentity::Feeds,
            Self::Service {
                service,
                library_id,
                ..
            } => TabIdentity::ServiceLibrary {
                kind: *service,
                library_id: library_id.clone(),
            },
        }
    }
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
    /// Activate the selected Hero parent through the destination's typed
    /// intent. This is the semantic equivalent of the keyboard Enter path;
    /// pointer delivery must not fabricate a raw key event.
    HeroActivate,
}

/// The resolved selector effect needed to restore a destination before its
/// item. The shell applies this against App-owned browse state, then pushes the
/// resulting content back through the normal projection seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) enum LaunchSelector {
    Emby { index: usize },
    EmbyLatest,
    AudiobookshelfShow(String),
    AudiobookshelfState,
    AudiobookshelfLatest,
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
            .map_or(LeafKeyResult::Unhandled, |message| {
                LeafKeyResult::Consumed(Some(Box::new(message)))
            })
    }

    fn hero_overlay_available(&mut self) -> bool {
        self.content().hero.is_some()
    }

    /// Whether the selected parent can own a Hero overlay before its Hero
    /// snapshot has materialized (for example while provider detail is
    /// loading). Owners with Hero-bearing browser rows override this so Enter
    /// never disappears during that hand-off.
    fn hero_overlay_target_available(&mut self) -> bool {
        false
    }

    /// Whether the panel's Enter-in-non-Wide path may open the overlay for the
    /// current browser selection. Defaults to the Hero-availability pair, so
    /// every existing owner keeps its Enter behavior; Grouped Music overrides
    /// it so unfiltered artist roots enter the same Hero path as Right while
    /// filtered roots stay local.
    fn hero_overlay_enter_available(&mut self) -> bool {
        self.hero_overlay_available() || self.hero_overlay_target_available()
    }

    /// Whether the owner asks the Library panel to show its selected item in
    /// the compact mini-view hero overlay. This is separate from ordinary
    /// browser activation: a mini-view episode hero must not turn Enter into
    /// an overlay request.
    fn mini_view_hero_available(&mut self) -> bool {
        false
    }

    /// A typed request the owner resolved from the frame it just painted, or
    /// `None`. The panel delivers it after `view` through its deferred-message
    /// seam (Grouped Music's completed-paint neighbour artwork window, task
    /// 6.5). Owners that resolve nothing post-paint keep the default.
    fn post_paint_message(&mut self) -> Option<Msg> {
        None
    }

    /// Whether this destination's browser rows are hero-bearing in non-Wide
    /// geometry, so Enter/double-click may open the Library Hero overlay
    /// (default). Owners whose rows are themselves the leaf content — the
    /// podcast tab's episode rows — return `false`: there is no overlay flow,
    /// and activation happens directly.
    fn browser_rows_are_hero_bearing(&mut self) -> bool {
        true
    }

    /// Whether a narrow browser double-click is intercepted by the panel to
    /// open the Hero overlay. Destinations with their own pointer semantics
    /// opt out while retaining the ordinary row policy.
    fn double_click_opens_hero_overlay(&mut self) -> bool {
        self.browser_rows_are_hero_bearing()
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

    /// Whether the Library Hero overlay is open over this owner. The panel
    /// sets it at open/dismiss and re-asserts it each sync pass. Owners whose
    /// pre-overlay narrow surfaces never focused a Workspace (Music's sync
    /// pass cleared inline track focus, TV's narrow keys ignored the pane
    /// bit) use it to keep the overlay's Workspace focus and key routing
    /// alive across ordinary refresh.
    fn set_hero_overlay_open(&mut self, _open: bool) {}

    /// Activate the currently selected Hero target through the owner's typed
    /// destination intent. Pointer gestures call this semantic operation, not
    /// a fabricated keyboard event.
    fn activate_hero_selection(&mut self) -> LeafKeyResult {
        match self.on_slot_event(LibrarySlotEvent::HeroActivate) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None => LeafKeyResult::Unhandled,
        }
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

    /// The owner's embedded Inline Search session, when it embeds one. The
    /// shell's inline-search host path (open/load/push/dismiss) resolves the
    /// panel's active owner through this; owners without a session return
    /// `None` (the default).
    fn inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        None
    }

    /// The shared-borrow twin of [`LibraryContentOwner::inline_search_session`]
    /// for the shell's pure reads (is-open, selected result).
    fn inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        None
    }

    /// Bounded read-only launch-state identities for orderly-teardown
    /// snapshot assembly (task 2.1, design D2): the current main-Selector
    /// pill and selected library-item identities. The shell invokes this
    /// ONLY on the active owner (via `Model::active_library_key`); never
    /// for unselected destinations. No live mirroring: local pill/item
    /// movement emits no persistence message for this, and no sync/render
    /// pass copies these values into `App` (the framework spec's
    /// exit-snapshot rule). Owners without narrowed identities yet (their
    /// unit keeps the default) report absence.
    // Consumed by selected-tab teardown assembly in task 2.3.
    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        (None, None)
    }

    /// Resolve the current destination's main Selector into an App-owned
    /// effect. The shell applies it before the item-level re-anchor, so a
    /// projection cannot overwrite a component-local downward mirror.
    fn launch_selector(&self, _state: &TuiLaunchState) -> Option<LaunchSelector> {
        None
    }

    /// Apply the item-level part of one discrete startup re-anchor after the
    /// destination's current content has been projected. Returning `true`
    /// consumes the destination-level pending state; ordinary refreshes never
    /// call this operation.
    fn reanchor_launch_state(&mut self, _state: &TuiLaunchState) -> bool {
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
