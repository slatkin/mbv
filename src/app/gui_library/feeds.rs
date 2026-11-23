//! The GUI Library's Feeds data path (add-gui-library-panel design D12,
//! row 11.1): the selection the core owns — the checked subscriptions and the
//! picked entry — and the open, set-pill, select, and ensure actions the
//! later core surface's commands apply. The selection is
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
//!
//! Every step is a free function over the `App` the GUI core owns, with the
//! `GuiCore` entries as one-line calls (design D4), so the core's own helpers
//! reach them through the `App` they already hold.

use std::borrow::Cow;

use mbv_queue::FeedEntry;

use super::collection::{GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::feeds_projection::{GuiFeedEntryId, feed_entry_matches};
use super::tabs::GuiDestinationKind;
use crate::app::App;
use crate::app::gui_client_state::GuiClientState;
use crate::gui::GuiCore;

/// The GUI Library's Feeds selection (row 11.1).
///
/// The stored collection selection — the checked subscriptions
/// (gui-collection-dropdown design D1, D2) — and the entry the Hero shows.
/// All is the set default, so the selection starts on All at every launch and
/// saves nothing — the store is session memory only. The list's sort lives in
/// the list-sort store. The checked set's keys are the
/// subscriptions' URLs. One feed tab exists per Client, so one store holds
/// it. The picked entry is named by its composite
/// `(subscription url, guid)` identity, encoded as one opaque string — two
/// subscriptions can publish the same `guid`, so the entry's own `guid` alone
/// names no row — and a pick the checked subscriptions' list no longer holds
/// falls back to that list's first row in the projection, as every
/// selection's stale arm does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiFeedsSelection {
    /// The checked subscriptions.
    pub selection: GuiCollectionSelection,
    /// The entry the Hero shows, as [`GuiFeedEntryId::encode`] writes it.
    /// `None` before a select command names one.
    pub picked_entry: Option<String>,
}

impl Default for GuiFeedsSelection {
    /// All with no pick: the selection every launch starts on (the spec's
    /// persistence rule — session memory only, nothing saved).
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
        super::feeds_projection::feeds_subscriptions(app)
    }

    fn stored_set<'a>(&self, gui: &'a GuiClientState) -> &'a GuiCollectionSet {
        &gui.feeds_selection.selection.set
    }

    fn has_stored_selection(&self, gui: &GuiClientState) -> bool {
        gui.feeds_selection.selection != GuiCollectionSelection::default()
    }

    fn reject_destination(&self, app: &App) -> Option<CollectionRejection> {
        if feeds_present(app) {
            None
        } else {
            Some(CollectionRejection::Ignored(
                "the feed tab is absent".to_string(),
            ))
        }
    }

    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection> {
        if feeds_subscription_known(app, key) {
            None
        } else {
            Some(CollectionRejection::Stale(key.to_string()))
        }
    }

    fn install_set(&self, _app: &mut App, gui: &mut GuiClientState, set: GuiCollectionSet) {
        gui.feeds_selection.selection.set = set;
    }
}

impl GuiCore {
    /// Open the Feeds destination (row 11.1; gui-one-destination-ensure design
    /// D4). The feed entries arrive from the shared startup fetch and the
    /// shared tick's `drain_feed_tab_results`, so an open starts no load and
    /// touches no network: the effective set's retention settles the stored
    /// checked set against the subscription set the tab holds wherever it is
    /// read, and the destination's load pass is a no-op for every step.
    pub fn gui_feeds_open(&mut self) {
        open_feeds(&mut self.app, &mut self.gui);
    }

    /// Select the entry the Hero shows (row 11.1).
    ///
    /// The selection the core owns, so the frame carries it back. The entry's
    /// own fields are already in the fetched list, so no load starts. The pick
    /// is the frame's opaque composite identity; a string that is no encoding
    /// of it, and an entry the checked subscriptions' list does not carry,
    /// are both a stale frame's pick and change nothing.
    pub fn gui_feeds_select(&mut self, entry: &str) {
        select_feed(&mut self.app, &mut self.gui, entry);
    }

    /// Play the entry a row id names (row 11.1): Enter on the row, or the
    /// double-click a window sends with this same command. The TUI's own
    /// play path replaces the queue with the entry at its saved position, and
    /// its no-playable-source guard flashes the TUI's toast. A string that is
    /// no encoding of a row id, and an entry the App's data no longer holds,
    /// are a stale frame's row and play nothing.
    pub(in crate::app::gui_library) fn play_gui_feed_row(&mut self, item_id: &str) {
        play_feed_row(&mut self.app, &self.gui, item_id);
    }

    /// Add the entry a row id names to the queue (row 11.1): `a` on the row,
    /// through the TUI's own enqueue path, whose no-playable-source guard
    /// flashes the TUI's toast. A stale row id queues nothing.
    pub(in crate::app::gui_library) fn enqueue_gui_feed_row(&mut self, item_id: &str) {
        enqueue_feed_row(&mut self.app, &self.gui, item_id);
    }

    /// The entries the selection lists (row 11.1), in App order.
    #[must_use]
    pub fn gui_feeds_entries(&self) -> Vec<&FeedEntry> {
        feeds_entries(&self.app, &self.gui)
    }
}

/// Open the Feeds destination (row 11.1; gui-one-destination-ensure design
/// D4): an open starts no load and touches no network, so it only reports an
/// absent tab, and the destination's load pass runs for it as a no-op.
pub(in crate::app::gui_library) fn open_feeds(app: &mut App, gui: &mut GuiClientState) {
    if !feeds_present(app) {
        super::log_ignored_library_command("feedsOpen", format_args!("the feed tab is absent"));
    }
    // The ordered load pass is the GUI core's, so the open step hands the
    // core the App it holds.
    let mut core = GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    *core.gui_state_mut() = std::mem::take(gui);
    core.ensure_gui_destination(GuiDestinationKind::Feeds);
    *gui = std::mem::take(core.gui_state_mut());
    *app = core.into_app();
}

/// Select the entry the Hero shows (row 11.1): the pick the frame carries
/// back, which the entry named by its composite identity must still be in the
/// checked subscriptions' list to change.
pub(in crate::app::gui_library) fn select_feed(
    app: &mut App,
    gui: &mut GuiClientState,
    entry: &str,
) {
    if !feeds_present(app) {
        super::log_ignored_library_command("feedsSelect", format_args!("the feed tab is absent"));
        return;
    }
    let Some(entry) = GuiFeedEntryId::decode(entry) else {
        super::log_stale_library_item("feedsSelect", entry);
        return;
    };
    if !feeds_entry_known(app, gui, &entry) {
        super::log_stale_library_item("feedsSelect", &entry.encode());
        return;
    }
    gui.feeds_selection.picked_entry = Some(entry.encode());
}

/// Play the entry a row id names (row 11.1).
pub(in crate::app::gui_library) fn play_feed_row(
    app: &mut App,
    gui: &GuiClientState,
    item_id: &str,
) {
    let Some(entry) = feed_entry(app, gui, item_id) else {
        super::log_stale_library_item("feedsPlay", item_id);
        return;
    };
    app.play_feed_entry(entry);
}

/// Add the entry a row id names to the queue (row 11.1).
pub(in crate::app::gui_library) fn enqueue_feed_row(
    app: &mut App,
    gui: &GuiClientState,
    item_id: &str,
) {
    let Some(entry) = feed_entry(app, gui, item_id) else {
        super::log_stale_library_item("feedsQueue", item_id);
        return;
    };
    app.enqueue_feed_entry(entry);
}

/// The feed tab's presence rule (row 11.1): configured subscriptions make
/// the tab.
pub(in crate::app) fn feeds_present(app: &App) -> bool {
    app.has_feeds_subscriptions()
}

/// The effective selection the list and the pill row paint (row 11.1): the
/// stored set with every checked subscription the tab no longer holds
/// dropped. A set nothing drops is returned borrowed, so a per-frame reader
/// clones nothing.
pub(in crate::app) fn feeds_effective_set<'a>(
    app: &'a App,
    gui: &'a GuiClientState,
) -> Cow<'a, GuiCollectionSet> {
    GuiFeedsCollectionSite.effective_set(app, gui)
}

/// The entry the Hero shows (row 11.1): the stored pick.
#[must_use]
pub(in crate::app) fn feeds_picked_entry(gui: &GuiClientState) -> Option<&str> {
    gui.feeds_selection.picked_entry.as_deref()
}

/// The entries the selection lists (row 11.1), in App order.
pub(in crate::app) fn feeds_entries<'a>(
    app: &'a App,
    gui: &'a GuiClientState,
) -> Vec<&'a FeedEntry> {
    match feeds_effective_set(app, gui).as_ref() {
        GuiCollectionSet::All => app.feed_tab.all_entries.iter().collect(),
        GuiCollectionSet::Checked(keys) => {
            let mut merged = Vec::new();
            for (index, subscription) in app.feed_tab.subscriptions.iter().enumerate() {
                if !keys.contains(&subscription.url) {
                    continue;
                }
                if let Some(entries) = app.feed_tab.entries.get(index) {
                    merged.extend(entries.iter());
                }
            }
            merged
        }
    }
}

/// The entry a row id names (row 11.1): the composite identity the frame's
/// row carries, resolved against the entries the App holds — the selection's
/// list first, because that is the list the frame painted, then the other
/// subscriptions' lists, so a play whose selection moved under a stale frame
/// still addresses the entry it painted.
pub(in crate::app) fn feed_entry(
    app: &App,
    gui: &GuiClientState,
    item_id: &str,
) -> Option<FeedEntry> {
    let id = GuiFeedEntryId::decode(item_id)?;
    let listed = feeds_entries(app, gui)
        .into_iter()
        .find(|entry| feed_entry_matches(entry, &id));
    listed
        .or_else(|| {
            app.feed_tab
                .all_entries
                .iter()
                .find(|entry| feed_entry_matches(entry, &id))
        })
        .cloned()
}

/// Whether the tab still holds the subscription `feed_id` names (row 11.1):
/// the URL the fetch stamps on each entry and the launch-neutral identity the
/// checked set carries.
fn feeds_subscription_known(app: &App, feed_id: &str) -> bool {
    feeds_subscription_index(app, feed_id).is_some()
}

/// The index of the subscription whose URL is `feed_id` (row 11.1): the
/// index the per-subscription entry lists use.
fn feeds_subscription_index(app: &App, feed_id: &str) -> Option<usize> {
    app.feed_tab
        .subscriptions
        .iter()
        .position(|subscription| subscription.url == feed_id)
}

/// Whether the checked subscriptions' list carries the entry `entry` names
/// (row 11.1): a landed list admits only an entry it holds, and an empty
/// list admits nothing, because no row is painted to pick.
fn feeds_entry_known(app: &App, gui: &GuiClientState, entry: &GuiFeedEntryId) -> bool {
    feeds_entries(app, gui)
        .iter()
        .any(|known| feed_entry_matches(known, entry))
}
