//! The GUI Library's collection-selection core (gui-collection-dropdown
//! design D1, D4): one set type for every collection destination — Music
//! groups, Podcast shows, Feeds subscriptions, and Emby feed-group folders —
//! whose selection is All or a non-empty set of checked collections, plus the
//! date comparator the date sorts share. A list's sort is not a collection
//! fact, so it lives in the list-sort store (`list_sort.rs`). The TUI-shared
//! stores never hold a set, so the TUI does not change.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;

/// All, or the checked collections of one destination (design D1). `Checked`
/// is never empty: the toggle and the retention rules fall back to [`Self::All`]
/// when no collection stays checked, so every reader can match on the two
/// shapes alone. The keys are the stable Service ids (the Music group id, the
/// podcast show's `library_item_id`, the subscription URL, the Emby folder
/// id), and they keep press order; the projections paint in the collection's
/// own order, so the order inside the set has no effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiCollectionSet {
    /// Every collection of the destination: the default and the fallback.
    All,
    /// The checked collections' stable Service ids, in press order.
    Checked(Vec<String>),
}

impl Default for GuiCollectionSet {
    /// All: the selection every destination starts on (design D1).
    fn default() -> Self {
        Self::All
    }
}

impl GuiCollectionSet {
    /// All: the shared borrowed default (design D1), so a reader without a
    /// stored entry borrows one value instead of cloning a fresh one.
    pub const ALL: Self = Self::All;

    /// Whether the selection is All (design D1).
    #[must_use]
    pub fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }

    /// The selection the key list `keys` names (design D1): an empty list is
    /// All, any other list is its keys. The one owner of the "empty becomes
    /// All" normalisation, so the toggle and the retention cannot drift.
    #[must_use]
    pub fn normalized(keys: Vec<String>) -> Self {
        if keys.is_empty() {
            Self::All
        } else {
            Self::Checked(keys)
        }
    }

    /// The selection after a press on the collection `key` (design D1): a
    /// press on an unchecked collection checks it and clears All; a press on
    /// a checked one unchecks it, and when no collection stays checked the
    /// selection becomes All.
    #[must_use]
    pub fn toggle(self, key: &str) -> Self {
        match self {
            Self::All => Self::Checked(vec![key.to_string()]),
            Self::Checked(mut keys) => {
                if let Some(position) = keys.iter().position(|checked| checked == key) {
                    keys.remove(position);
                } else {
                    keys.push(key.to_string());
                }
                Self::normalized(keys)
            }
        }
    }

    /// The checked keys; empty for All (design D1): the save writes them to
    /// the launch file, where an empty list means All.
    #[must_use]
    pub fn checked_keys(&self) -> &[String] {
        match self {
            Self::All => &[],
            Self::Checked(keys) => keys,
        }
    }

    /// The selection after the catalog changed (design D1): the checked keys
    /// `present` does not hold drop, and when none survive the selection
    /// becomes All. All stays All. The stored set is cloned only when a key
    /// drops; an unchanged set is returned borrowed, so a per-frame reader
    /// allocates nothing when the catalog holds every checked key. An empty
    /// key list is All before the all-present read, whose `all` is vacuously
    /// true for one — the stored shape a saved All carries, so the launch
    /// restore normalizes it instead of installing the empty shape.
    #[must_use]
    pub fn retain_present(&self, present: impl Fn(&str) -> bool) -> Cow<'_, Self> {
        let Self::Checked(keys) = self else {
            return Cow::Borrowed(self);
        };
        if keys.is_empty() {
            return Cow::Owned(Self::All);
        }
        if keys.iter().all(|key| present(key)) {
            return Cow::Borrowed(self);
        }
        let mut keys = keys.clone();
        keys.retain(|key| present(key));
        Cow::Owned(Self::normalized(keys))
    }

    /// The selection after the catalog settled (design D1): `Some(present)`
    /// is the landed catalog, whose retention applies — every checked key it
    /// no longer holds drops, and when none survive the selection becomes
    /// All; `None` is a catalog that has not landed yet, the list that would
    /// name a key stale, which keeps the set as it is. All stays All either
    /// way, and a set nothing drops is returned borrowed — except an empty
    /// key list, which is All before the catalog lands too: nothing can be
    /// stale in an empty list, and the stored shape a saved All carries must
    /// not survive as the empty Checked shape.
    #[must_use]
    pub fn effective(&self, present: Option<impl Fn(&str) -> bool>) -> Cow<'_, Self> {
        match present {
            Some(present) => self.retain_present(present),
            None if matches!(self, Self::Checked(keys) if keys.is_empty()) => Cow::Owned(Self::All),
            None => Cow::Borrowed(self),
        }
    }
}

/// One destination's whole stored selection (design D2, D4): the checked set
/// the dropdown paints. The one shape all four stores hold — the Music
/// store's set, the Podcasts and Feeds selections' halves, and the Emby
/// feed-group map's value. The list's sort is not a collection fact, so it
/// lives in the list-sort store beside the destination's own state
/// (`App::list_sorts`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiCollectionSelection {
    /// The checked collections, or All. All is the default.
    pub set: GuiCollectionSet,
}

/// The comparator's order (design D4): Newest is the default, and undated
/// rows stay last in both orders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiDateOrder {
    /// The most recent row first.
    #[default]
    Newest,
    /// The earliest row first.
    Oldest,
}

/// The collection destination a dropdown command names (design D5): the
/// index is the library's index in the core's catalog, Emby's `libs` or the
/// Audiobookshelf libraries', as the destination kinds key them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiCollectionTarget {
    /// The Grouped Music library at the Emby library index.
    Music(usize),
    /// The Audiobookshelf podcast library at its index.
    Podcasts(usize),
    /// The Feeds tab.
    Feeds,
    /// The Emby library in feed-group mode at the Emby library index.
    EmbyFeed(usize),
}

/// The noun a destination's collections take (design D6): the trigger label
/// reads "2 groups", "2 shows", "2 subscriptions", or "2 folders".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiCollectionNoun {
    Groups,
    Shows,
    Subscriptions,
    Folders,
}

impl GuiCollectionNoun {
    /// The noun the selector's count label reads (design D6): "2 groups",
    /// "2 shows", "2 subscriptions", or "2 folders".
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Groups => "groups",
            Self::Shows => "shows",
            Self::Subscriptions => "subscriptions",
            Self::Folders => "folders",
        }
    }
}

/// One collection entry of a destination's dropdown (design D6): its stable
/// Service id, its label, and whether the selection checks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiCollectionEntry {
    /// The collection's stable Service id: the toggle command carries it.
    pub key: String,
    /// The collection's display name, the row's label.
    pub name: String,
    /// Whether the selection checks this collection.
    pub checked: bool,
}

/// One destination's whole collection selector (design D6): the target its
/// commands name, the All state, every collection in catalog order with its
/// checked flag, and the destination's noun. The list's sort is not a
/// collection fact, so the frame carries it beside the selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiCollections {
    /// The destination the toggle and All commands name.
    pub target: GuiCollectionTarget,
    /// Whether the selection is All.
    pub all: bool,
    /// The catalog's collections in its own order.
    pub entries: Vec<GuiCollectionEntry>,
    /// The noun the trigger label and the Clear control read.
    pub noun: GuiCollectionNoun,
}

impl GuiCollections {
    /// The selector for `target` (design D6) over `set`, with `scopes` — each
    /// collection's stable key and name, in catalog order — as its entries.
    /// The scopes' pairs convert as they are kept, so a site can stream
    /// borrowed catalog entries without a freshly cloned vector between them.
    #[must_use]
    pub fn new<S: Into<String>, N: Into<String>>(
        target: GuiCollectionTarget,
        set: &GuiCollectionSet,
        noun: GuiCollectionNoun,
        scopes: impl IntoIterator<Item = (S, N)>,
    ) -> Self {
        let checked: HashSet<&str> = set.checked_keys().iter().map(String::as_str).collect();
        Self {
            target,
            all: set.is_all(),
            entries: scopes
                .into_iter()
                .map(|(key, name)| {
                    let key = key.into();
                    GuiCollectionEntry {
                        checked: checked.contains(key.as_str()),
                        key,
                        name: name.into(),
                    }
                })
                .collect(),
            noun,
        }
    }

    /// The names the destination's location and the Music Hero header read
    /// (design D6): All names itself, and a checked set names its checked
    /// collections' names in catalog order. The one derivation of the names
    /// the Status bar's middle segment and the Music Hero header carry, so
    /// every painter that names the selected collections reads the same fact.
    /// The Emby feed-group location reads its own derivation over the checked
    /// set's press order instead (`feed_collection_names`), whose stale-folder
    /// fallback to the stable id the entries cannot carry: a folder the
    /// listing no longer names holds no entry here.
    #[must_use]
    pub fn selected_names(&self) -> Vec<String> {
        if self.all {
            return vec!["All".to_string()];
        }
        self.entries
            .iter()
            .filter(|entry| entry.checked)
            .map(|entry| entry.name.clone())
            .collect()
    }
}

/// Compare two epoch-second dates in `order` (design D4): Newest puts the
/// later date first, Oldest the earlier first, and a row with no date stays
/// last in both orders. The one owner of the "undated last" rule, so the
/// Podcasts, Feeds, and Emby feed-group sorts cannot drift. The providers
/// carry `u64` seconds; a count past `i64::MAX` names no real date, so it
/// saturates.
#[must_use]
pub fn compare_gui_date_order(a: Option<u64>, b: Option<u64>, order: GuiDateOrder) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => match order {
            GuiDateOrder::Newest => b.cmp(&a),
            GuiDateOrder::Oldest => a.cmp(&b),
        },
    }
}

#[cfg(test)]
mod tests;
