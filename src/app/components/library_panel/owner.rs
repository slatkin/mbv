//! The Library panel's embedded content-owner contract (tasks 5.7–5.9,
//! design D2/D3). A content owner is a plain type — never mounted, focused,
//! subscribed, or given a `ComponentId` — that keeps its Service content,
//! media lists, cursor/scroll/focus state, and typed intent translation. The
//! mounted [`super::panel::LibraryPanel`] is the library area's one event
//! boundary: it resolves pointer input against its own painted slot geometry
//! and hands the active owner semantic slot events, which the owner
//! translates into its existing typed `Msg`s.

use std::collections::HashMap;

use crate::app::components::component_id::BrowserKey;
use crate::app::components::media_list::RowLocalInput;
use crate::app::components::msg::Msg;

use super::content::LibraryPanelContent;

/// The identity of one library destination, keying the panel's owner map
/// (design D2: `Home | Feeds | Service(BrowserKey)`). Stable across
/// re-renders so an inactive owner keeps its private state across tab
/// changes; `Service` keys are the `BrowserKey` the shell already derives
/// for every mounted destination.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::app) enum LibraryKey {
    Home,
    Feeds,
    Service(BrowserKey),
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
    List(RowLocalInput),
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
