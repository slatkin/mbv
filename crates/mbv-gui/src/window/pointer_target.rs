//! The pointer target (design D1 of `gui-pointer-target-interaction`): the
//! thing under the pointer that receives a GUI key.
//!
//! Blitz resolves the hovered node itself — `BaseDocument::resolve` calls
//! `refresh_hover`, which re-resolves it from the last pointer position after
//! every layout and scroll — so the hover fact stays right under a still
//! pointer, where a Dioxus `onpointerenter` handler would go stale. The launch
//! copy reads it after each window event ([`hover_chain`]), walks the ancestor
//! chain once ([`pointer_target`]), and writes the result into the root's
//! signal ([`PointerTargetStore`]) only when it changed.
//!
//! The DOM markers name the targets: [`ZONE_ATTRIBUTE`] on the Library column
//! and the Queue list box, [`LIST_ATTRIBUTE`] on a list's scroller, and
//! [`ROW_ATTRIBUTE`] on a list row. Every value is a fixed token except the
//! row's, which is the same row id the destination's commands name. The walk
//! is pure, over the ancestors' attribute values, so its contracts are unit
//! tests; the launch copy is its only caller.
//!
//! The signal's readers peek only, and no component subscribes, so a hover
//! change re-renders nothing: the hover paint stays the CSS `:hover` veil over
//! the same node Blitz resolved.

use std::cell::RefCell;
use std::rc::Rc;

use blitz_dom::{BaseDocument, LocalName, Node};
use dioxus_native::prelude::{Signal, VirtualDom, WritableExt};

/// The attribute that names the panel zone under the pointer: `library` on the
/// Library column's root, `queue` on the Queue list box (the header, the
/// filter, the rows, and the footer band — not the player block).
pub const ZONE_ATTRIBUTE: &str = "data-pointer-zone";

/// The attribute that names the list under the pointer: the destination list
/// on each list scroller, and `tracklist` or `workspace` inside the Hero.
pub const LIST_ATTRIBUTE: &str = "data-pointer-list";

/// The attribute that names the row under the pointer: the row's stable id,
/// the value the destination's own commands name.
pub const ROW_ATTRIBUTE: &str = "data-pointer-row";

/// The panel zone under the pointer (design D1): the Library column or the
/// Queue list box. A key's target zone picks which panel's keys act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerZone {
    /// The Library column: its lists, the pill rows, the Hero, and the
    /// Status bar.
    Library,
    /// The Queue list box: the header, the filter, the rows, and the footer
    /// band.
    Queue,
}

impl PointerZone {
    /// The value the zone's marker carries.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Queue => "queue",
        }
    }

    /// The zone a marker's value names, or `None` for a value no painter
    /// writes.
    #[must_use]
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "library" => Some(Self::Library),
            "queue" => Some(Self::Queue),
            _ => None,
        }
    }
}

/// The list under the pointer (design D1): a destination's list, the Hero's
/// tracklist, or a Hero's Workspace table. A key acts on the hovered row only
/// while the list is the one that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerList {
    /// The Music list.
    Music,
    /// The Home list.
    Home,
    /// The Shows list.
    Shows,
    /// The Favourites list (emby-favourites design D6): the list the Shows
    /// and plain Emby destinations paint while their view is a favourites
    /// view, with a scroll store of its own per library. Its rows carry
    /// stable Service ids, so the panel's keys resolve them uniformly.
    Favourites,
    /// A plain Emby library's list.
    Emby,
    /// The Audiobooks list.
    Audiobooks,
    /// The Podcasts list.
    Podcasts,
    /// The Feeds list.
    Feeds,
    /// The picked album's tracklist in the Music Hero.
    Tracklist,
    /// A Hero's Workspace table: the Shows episodes, the Audiobooks
    /// chapters, or the Feeds entries.
    Workspace,
}

impl PointerList {
    /// The value the list's marker carries.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Home => "home",
            Self::Shows => "shows",
            Self::Favourites => "favourites",
            Self::Emby => "emby",
            Self::Audiobooks => "audiobooks",
            Self::Podcasts => "podcasts",
            Self::Feeds => "feeds",
            Self::Tracklist => "tracklist",
            Self::Workspace => "workspace",
        }
    }

    /// The list a marker's value names, or `None` for a value no painter
    /// writes.
    #[must_use]
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "music" => Some(Self::Music),
            "home" => Some(Self::Home),
            "shows" => Some(Self::Shows),
            "favourites" => Some(Self::Favourites),
            "emby" => Some(Self::Emby),
            "audiobooks" => Some(Self::Audiobooks),
            "podcasts" => Some(Self::Podcasts),
            "feeds" => Some(Self::Feeds),
            "tracklist" => Some(Self::Tracklist),
            "workspace" => Some(Self::Workspace),
            _ => None,
        }
    }
}

/// The thing under the pointer (design D1): the panel zone, the list, and the
/// row its markers name. A node that carries no marker walks up to the nearest
/// ancestor of each kind, so the Library column reads as its zone alone and a
/// row's chrome child reads as its row. An empty target — the pointer over the
/// player block, over no marked node, or outside the window — is the default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PointerTarget {
    /// The panel zone under the pointer, or `None` while it is over neither
    /// panel's marked box.
    pub zone: Option<PointerZone>,
    /// The list under the pointer, or `None` while it is over no list.
    pub list: Option<PointerList>,
    /// The hovered row's stable id, or `None` while the pointer is over no
    /// row.
    pub row: Option<String>,
}

/// One node of the hovered node's ancestor chain, as the three marker values
/// the walk reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PointerAncestor<'a> {
    /// The node's [`ZONE_ATTRIBUTE`] value.
    pub zone: Option<&'a str>,
    /// The node's [`LIST_ATTRIBUTE`] value.
    pub list: Option<&'a str>,
    /// The node's [`ROW_ATTRIBUTE`] value.
    pub row: Option<&'a str>,
}

impl<'a> PointerAncestor<'a> {
    /// Read one node's marker attributes. A node that holds none — a text
    /// node, a `div`, a glyph — reads as an ancestor with no markers, so the
    /// walk keeps climbing.
    #[must_use]
    pub fn of(node: &'a Node) -> Self {
        Self {
            zone: node.attr(LocalName::from(ZONE_ATTRIBUTE)),
            list: node.attr(LocalName::from(LIST_ATTRIBUTE)),
            row: node.attr(LocalName::from(ROW_ATTRIBUTE)),
        }
    }
}

/// The hovered node's ancestor chain, nearest first, as the marker values the
/// walk reads (design D1). The hovered node is the one Blitz resolved, so a
/// scroll under a still pointer reads the row that now carries the `:hover`
/// look. A hovered node the document no longer holds reads as an empty chain,
/// and so as the empty target.
#[must_use]
pub fn hover_chain(document: &BaseDocument) -> Vec<PointerAncestor<'_>> {
    let mut chain = Vec::new();
    let mut candidate = document.get_hover_node_id();
    while let Some(id) = candidate {
        let Some(node) = document.get_node(id) else {
            break;
        };
        chain.push(PointerAncestor::of(node));
        candidate = node.parent;
    }
    chain
}

/// The pointer target an ancestor chain names (design D1), nearest ancestor
/// first: the nearest marker of each kind wins, so a row's chrome child
/// resolves to its row, and a marker value no painter writes counts as none.
#[must_use]
pub fn pointer_target(chain: &[PointerAncestor<'_>]) -> PointerTarget {
    let mut target = PointerTarget::default();
    for ancestor in chain {
        if target.zone.is_none() {
            target.zone = ancestor.zone.and_then(PointerZone::from_token);
        }
        if target.list.is_none() {
            target.list = ancestor.list.and_then(PointerList::from_token);
        }
        if target.row.is_none() {
            target.row = ancestor.row.map(str::to_owned);
        }
    }
    target
}

/// The root's pointer-target signal, held by the launch copy (design D1).
///
/// The root provides its signal through [`Self::provide`] while its tree
/// builds, and the launch copy writes it after each window event through
/// [`Self::set`] — from the vdom's runtime, and only when the value changed,
/// as the launch copy writes the viewport signal (D9 of
/// `replace-tauri-with-blitz`). The store keeps the value it wrote, so an
/// unchanged hover costs one comparison and marks no subscriber.
#[derive(Clone, Default)]
pub struct PointerTargetStore {
    /// The root's signal, `None` until its tree builds.
    signal: Rc<RefCell<Option<Signal<PointerTarget>>>>,
    /// The value the last write carried.
    written: Rc<RefCell<Option<PointerTarget>>>,
}

impl std::fmt::Debug for PointerTargetStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointerTargetStore")
            .field("provided", &self.signal.borrow().is_some())
            .field("written", &self.written.borrow())
            .finish()
    }
}

impl PointerTargetStore {
    /// A store no root has provided its signal through yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The root handed its signal over (design D1): called once, while its
    /// tree builds.
    pub fn provide(&self, target: Signal<PointerTarget>) {
        *self.signal.borrow_mut() = Some(target);
    }

    /// Write `target` into the root's signal, from the vdom's runtime, when it
    /// differs from the value last written (design D1). A signal write marks
    /// its subscribers, an equal value included, so the comparison is what
    /// keeps a hover change from re-rendering the window.
    pub fn set(&self, vdom: &VirtualDom, target: PointerTarget) {
        if self.written.borrow().as_ref() == Some(&target) {
            return;
        }
        let Some(mut signal) = *self.signal.borrow() else {
            return;
        };
        let write = target.clone();
        vdom.in_runtime(move || signal.set(write));
        *self.written.borrow_mut() = Some(target);
    }
}

#[cfg(test)]
mod tests;
