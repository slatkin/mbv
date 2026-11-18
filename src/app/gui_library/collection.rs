//! The GUI Library's collection-selection core (gui-collection-dropdown
//! design D1, D4): one set type for every collection destination — Music
//! groups, Podcast shows, Feeds subscriptions, and Emby feed-group folders —
//! whose selection is All or a non-empty set of checked collections, plus the
//! date order the feed-based lists sort by. The TUI-shared stores never hold
//! a set, so the TUI does not change.

use std::cmp::Ordering;

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

impl GuiCollectionSet {
    /// Whether the selection is All (design D1).
    #[must_use]
    pub fn is_all(&self) -> bool {
        matches!(self, Self::All)
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
                    if keys.is_empty() {
                        Self::All
                    } else {
                        Self::Checked(keys)
                    }
                } else {
                    keys.push(key.to_string());
                    Self::Checked(keys)
                }
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
    /// the catalog no longer holds drop, and when none survive the selection
    /// becomes All. All stays All.
    #[must_use]
    pub fn retain_present(self, present: &[String]) -> Self {
        match self {
            Self::All => Self::All,
            Self::Checked(mut keys) => {
                keys.retain(|key| present.contains(key));
                if keys.is_empty() {
                    Self::All
                } else {
                    Self::Checked(keys)
                }
            }
        }
    }
}

/// The list order of a feed-based destination (design D4): Newest is the
/// default, and undated rows stay last in both orders.
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
/// checked flag, the destination's noun, and the date order a feed-based
/// destination sorts by. Music has no date order, so its `order` is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiCollections {
    /// The destination the toggle, All, and date-order commands name.
    pub target: GuiCollectionTarget,
    /// Whether the selection is All.
    pub all: bool,
    /// The catalog's collections in its own order.
    pub entries: Vec<GuiCollectionEntry>,
    /// The noun the trigger label and the Clear control read.
    pub noun: GuiCollectionNoun,
    /// The list's date order on a feed-based destination; `None` on Music,
    /// which sorts by name.
    pub order: Option<GuiDateOrder>,
}

impl GuiCollections {
    /// The selector for `target` (design D6) over `set`, with `scopes` — each
    /// collection's stable key and name, in catalog order — as its entries,
    /// and the destination's list `order`.
    #[must_use]
    pub fn new(
        target: GuiCollectionTarget,
        set: &GuiCollectionSet,
        noun: GuiCollectionNoun,
        scopes: impl IntoIterator<Item = (String, String)>,
        order: Option<GuiDateOrder>,
    ) -> Self {
        Self {
            target,
            all: set.is_all(),
            entries: scopes
                .into_iter()
                .map(|(key, name)| GuiCollectionEntry {
                    checked: set.checked_keys().contains(&key),
                    key,
                    name,
                })
                .collect(),
            noun,
            order,
        }
    }
}

/// Compare two epoch-second dates in `order` (design D4): Newest puts the
/// later date first, Oldest the earlier first, and a row with no date stays
/// last in both orders. The one owner of the "undated last" rule, so the
/// Podcasts, Feeds, and Emby feed-group sorts cannot drift.
#[must_use]
pub fn compare_gui_date_order(a: Option<i64>, b: Option<i64>, order: GuiDateOrder) -> Ordering {
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
