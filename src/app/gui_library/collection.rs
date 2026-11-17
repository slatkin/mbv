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
    /// The selection every destination starts on (design D1).
    #[must_use]
    pub fn all() -> Self {
        Self::All
    }

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

    /// Whether the selection covers `key`: All covers every collection
    /// (design D1).
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        match self {
            Self::All => true,
            Self::Checked(keys) => keys.iter().any(|checked| checked == key),
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
