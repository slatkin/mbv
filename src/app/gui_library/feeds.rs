//! The GUI Library's Feeds data path (add-gui-library-panel design D12,
//! row 11.1): the selection the core owns — the checked subscriptions, the
//! date order, and the picked entry — and the open, set-pill, select, and
//! ensure actions the later core surface's commands apply. The selection is
//! All or a set of checked subscriptions (gui-collection-dropdown design
//! D1, D2), so the dropdown replaces the pill row in a later unit.
//!
//! The feed data itself is already Client-neutral. The configured
//! subscriptions land on the `App` at construction
//! (`App::sync_feed_subscriptions`), the startup fetch is the shared
//! `App::start_client_services` step, and the results drain in the shared
//! `ClientCore::tick` through `App::drain_feed_tab_results`, so the GUI tick
//! receives them like the TUI shell does. Only the TUI's projection push
//! (`Model::sync_feeds`) and the TUI owner's own group and Watched-filter
//! selection are shell-only, and they write `Model` state. So nothing that
//! produces `App` feed state moved here, and the TUI never reads any of it.

use std::borrow::Cow;

use mbv_queue::FeedEntry;

use super::collection::{
    GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet, GuiDateOrder,
};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::feeds_projection::{GuiFeedEntryId, feed_entry_matches};
use crate::app::App;

/// The GUI Library's Feeds selection (row 11.1).
///
/// The stored collection selection — the checked subscriptions and the list's
/// date order (gui-collection-dropdown design D1, D2, D4) — and the entry the
/// Hero shows. All is the set default and Newest the order default, so the
/// selection starts on All and Newest at every launch and saves nothing — the
/// store is session memory only. The checked set's keys are the
/// subscriptions' URLs. One feed tab exists per Client, so one store holds
/// it. The picked entry is named by its composite
/// `(subscription url, guid)` identity, encoded as one opaque string — two
/// subscriptions can publish the same `guid`, so the entry's own `guid` alone
/// names no row — and a pick the checked subscriptions' list no longer holds
/// falls back to that list's first row in the projection, as every
/// selection's stale arm does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiFeedsSelection {
    /// The checked subscriptions and the list's date order.
    pub selection: GuiCollectionSelection,
    /// The entry the Hero shows, as [`GuiFeedEntryId::encode`] writes it.
    /// `None` before a select command names one.
    pub picked_entry: Option<String>,
}

impl Default for GuiFeedsSelection {
    /// All on Newest with no pick: the selection every launch starts on (the
    /// spec's persistence rule — session memory only, nothing saved).
    fn default() -> Self {
        Self {
            selection: GuiCollectionSelection::default(),
            picked_entry: None,
        }
    }
}

/// The Feeds destination's collection-command site (gui-collection-dropdown
/// design D5): the feed-tab guard, the subscription staleness rule, the
/// subscription catalog, and the selection install the shared collection path
/// calls.
pub(super) struct GuiFeedsCollectionSite;

impl GuiCollectionSite for GuiFeedsCollectionSite {
    fn noun(&self) -> GuiCollectionNoun {
        GuiCollectionNoun::Subscriptions
    }

    fn has_order(&self) -> bool {
        true
    }

    fn present(&self, app: &App) -> Option<impl Fn(&str) -> bool> {
        // The tab is present whenever the destination serves a command, so
        // the retention always applies (row 11.1).
        Some(move |key: &str| {
            app.feed_tab
                .subscriptions
                .iter()
                .any(|subscription| subscription.url == key)
        })
    }

    fn scopes(&self, app: &App) -> impl IntoIterator<Item = (String, String)> {
        app.gui_feeds_subscriptions()
    }

    fn stored_set<'a>(&self, app: &'a App) -> &'a GuiCollectionSet {
        &app.gui_feeds_selection.selection.set
    }

    fn stored_order(&self, app: &App) -> GuiDateOrder {
        app.gui_feeds_selection.selection.order
    }

    fn reject_destination(&self, app: &App) -> Option<CollectionRejection> {
        if app.gui_feeds_present() {
            None
        } else {
            Some(CollectionRejection::Ignored(
                "the feed tab is absent".to_string(),
            ))
        }
    }

    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection> {
        if app.gui_feeds_subscription_known(key) {
            None
        } else {
            Some(CollectionRejection::Stale(key.to_string()))
        }
    }

    fn install_set(&self, app: &mut App, set: GuiCollectionSet) {
        app.gui_feeds_selection.selection.set = set;
    }

    fn install_order(&self, app: &mut App, order: GuiDateOrder) {
        app.gui_feeds_selection.selection.order = order;
    }
}

impl App {
    /// The feed tab's presence rule (row 11.1).
    ///
    /// Configured subscriptions make the tab, exactly as the TUI's own
    /// tab-strip rule does (`App::has_feeds_subscriptions`).
    pub(in crate::app::gui_library) fn gui_feeds_present(&self) -> bool {
        self.has_feeds_subscriptions()
    }

    /// Open the Feeds destination (row 11.1). The feed entries arrive from the
    /// shared startup fetch and the shared tick's `drain_feed_tab_results`, so
    /// an open starts no load and touches no network: the effective set's
    /// retention settles the stored checked set against the subscription set
    /// the tab holds wherever it is read.
    pub fn gui_feeds_open(&mut self) {
        if !self.gui_feeds_present() {
            Self::log_ignored_library_command("feedsOpen", format_args!("the feed tab is absent"));
        }
    }

    /// Select the entry the Hero shows (row 11.1).
    ///
    /// The selection the core owns, so the frame carries it back. The entry's
    /// own fields are already in the fetched list, so no load starts. The pick
    /// is the frame's opaque composite identity; a string that is no encoding
    /// of it, and an entry the checked subscriptions' list does not carry,
    /// are both a stale frame's pick and change nothing.
    /// stale frame's pick and change nothing.
    pub fn gui_feeds_select(&mut self, entry: &str) {
        if !self.gui_feeds_present() {
            Self::log_ignored_library_command(
                "feedsSelect",
                format_args!("the feed tab is absent"),
            );
            return;
        }
        let Some(entry) = GuiFeedEntryId::decode(entry) else {
            Self::log_stale_library_item("feedsSelect", entry);
            return;
        };
        if !self.gui_feeds_entry_known(&entry) {
            Self::log_stale_library_item("feedsSelect", &entry.encode());
            return;
        }
        self.gui_feeds_selection.picked_entry = Some(entry.encode());
    }

    /// Play the entry a row id names (row 11.1): Enter on the row, or the
    /// double-click a window sends with this same command. The TUI's own
    /// play path replaces the queue with the entry at its saved position, and
    /// its no-playable-source guard flashes the TUI's toast. A string that is
    /// no encoding of a row id, and an entry the App's data no longer holds,
    /// are a stale frame's row and play nothing.
    pub(in crate::app::gui_library) fn play_gui_feed_row(&mut self, item_id: &str) {
        let Some(entry) = self.gui_feed_entry(item_id) else {
            Self::log_stale_library_item("feedsPlay", item_id);
            return;
        };
        self.play_feed_entry(entry);
    }

    /// Add the entry a row id names to the queue (row 11.1): `a` on the row,
    /// through the TUI's own enqueue path, whose no-playable-source guard
    /// flashes the TUI's toast. A stale row id queues nothing.
    pub(in crate::app::gui_library) fn enqueue_gui_feed_row(&mut self, item_id: &str) {
        let Some(entry) = self.gui_feed_entry(item_id) else {
            Self::log_stale_library_item("feedsQueue", item_id);
            return;
        };
        self.enqueue_feed_entry(entry);
    }

    /// The effective selection the list and the pill row paint (row 11.1;
    /// gui-collection-dropdown design D1): the stored set with every checked
    /// subscription the tab no longer holds dropped — the spec's vanish rule,
    /// so when none of the checked subscriptions survive, the selection
    /// becomes All. All stays All. A set nothing drops is returned borrowed,
    /// so a per-frame reader clones nothing.
    pub(in crate::app) fn gui_feeds_effective_set(&self) -> Cow<'_, GuiCollectionSet> {
        GuiFeedsCollectionSite.effective_set(self)
    }

    /// The date order the list sorts by (row 11.1; gui-collection-dropdown
    /// design D4): the stored order — Newest is the launch default.
    #[must_use]
    pub fn gui_feeds_date_order(&self) -> GuiDateOrder {
        self.gui_feeds_selection.selection.order
    }

    /// The entry the Hero shows (row 11.1): the stored pick. The projection
    /// falls back to the checked subscriptions' first row when the pick left
    /// the list, so the Hero's default is never blank.
    #[must_use]
    pub fn gui_feeds_picked_entry(&self) -> Option<&str> {
        self.gui_feeds_selection.picked_entry.as_deref()
    }

    /// The entries the selection lists (row 11.1), in App order.
    ///
    /// All is the merged `all_entries`, and a checked set is the checked
    /// subscriptions' own lists, concatenated in the tab's order. Both sort
    /// by the stored date order when the projection paints them. The list
    /// borrows the App's stored entries, so a frame that lists a subscription
    /// copies no entry.
    #[must_use]
    pub fn gui_feeds_entries(&self) -> Vec<&FeedEntry> {
        match self.gui_feeds_effective_set().as_ref() {
            GuiCollectionSet::All => self.feed_tab.all_entries.iter().collect(),
            GuiCollectionSet::Checked(keys) => {
                let mut merged = Vec::new();
                for (index, subscription) in self.feed_tab.subscriptions.iter().enumerate() {
                    if !keys.contains(&subscription.url) {
                        continue;
                    }
                    if let Some(entries) = self.feed_tab.entries.get(index) {
                        merged.extend(entries.iter());
                    }
                }
                merged
            }
        }
    }

    /// Whether the tab still holds the subscription `feed_id` names (row
    /// 11.1): the URL the fetch stamps on each entry and the launch-neutral
    /// identity the checked set carries.
    fn gui_feeds_subscription_known(&self, feed_id: &str) -> bool {
        self.gui_feeds_subscription_index(feed_id).is_some()
    }

    /// The index of the subscription whose URL is `feed_id` (row 11.1): the
    /// index the per-subscription entry lists use.
    fn gui_feeds_subscription_index(&self, feed_id: &str) -> Option<usize> {
        self.feed_tab
            .subscriptions
            .iter()
            .position(|subscription| subscription.url == feed_id)
    }

    /// The entry a row id names (row 11.1): the composite identity the frame's
    /// row carries, resolved against the entries the App holds — the
    /// selection's list first, because that is the list the frame painted,
    /// then the other subscriptions' lists, so a play whose selection moved
    /// under a stale frame still addresses the entry it painted. `None` on a
    /// string that is no encoding of a row id and on an entry no list holds.
    fn gui_feed_entry(&self, item_id: &str) -> Option<FeedEntry> {
        let id = GuiFeedEntryId::decode(item_id)?;
        let listed = self
            .gui_feeds_entries()
            .into_iter()
            .find(|entry| feed_entry_matches(entry, &id));
        listed
            .or_else(|| {
                self.feed_tab
                    .all_entries
                    .iter()
                    .find(|entry| feed_entry_matches(entry, &id))
            })
            .cloned()
    }

    /// Whether the checked subscriptions' list carries the entry `entry`
    /// names (row 11.1): a landed list admits only an entry it holds, and an
    /// empty list admits nothing, because no row is painted to pick.
    fn gui_feeds_entry_known(&self, entry: &GuiFeedEntryId) -> bool {
        self.gui_feeds_entries()
            .iter()
            .any(|known| feed_entry_matches(known, entry))
    }
}
