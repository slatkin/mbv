//! The GUI Library's Feeds data path (add-gui-library-panel design D12,
//! row 11.1): the selection the core owns — the selected pill and the picked
//! entry — and the open, set-pill, select, and ensure actions the later core
//! surface's commands apply.
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

use mbv_queue::FeedEntry;

use crate::app::App;

/// The Feeds destination's pills (row 11.1).
///
/// The merged Latest view, or one subscription's entries by the subscription's
/// URL — the feed identity the fetch stamps on each entry and the identity the
/// state store keys on. The pill row is single-select, and Latest is the
/// default, as the TUI owner's one-valued group selection is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GuiFeedPill {
    /// Every subscription's entries, merged newest first with undated rows
    /// last. The TUI's default group and the GUI's too.
    #[default]
    Latest,
    /// The merged view scoped to one subscription's entries.
    Subscription(String),
}

/// The GUI Library's Feeds selection (row 11.1).
///
/// The selected pill and the entry the Hero shows. One feed tab exists per
/// Client, so one store holds it. The picked entry is named by its stable
/// `guid` — the identity the TUI owner's row target and the queue's feed
/// content id carry — and a pick the selected pill's list no longer holds
/// falls back to that list's first row in the projection, as every
/// selection's stale arm does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiFeedsSelection {
    /// The selected pill. Latest is the default.
    pub pill: GuiFeedPill,
    /// The entry the Hero shows, by its `guid`. `None` before a select
    /// command names one.
    pub picked_entry: Option<String>,
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
    /// an open starts no load and touches no network: it settles the stored
    /// pill against the subscription set the tab holds, which is the same
    /// trigger the frame path runs.
    pub fn gui_feeds_open(&mut self) {
        if !self.gui_feeds_present() {
            Self::log_ignored_library_command("feedsOpen", format_args!("the feed tab is absent"));
            return;
        }
        self.ensure_gui_feeds_selection();
    }

    /// Select the destination's pill (row 11.1).
    ///
    /// The selection the core owns, so the frame carries it back. No load
    /// starts, because every subscription's entries arrive from the one fetch.
    /// A subscription the tab no longer holds is a stale frame's pill and
    /// changes nothing; a set of entries still landing admits it, because the
    /// subscription list is what would name it stale.
    pub fn gui_feeds_set_pill(&mut self, pill: GuiFeedPill) {
        if !self.gui_feeds_present() {
            Self::log_ignored_library_command(
                "feedsSetPill",
                format_args!("the feed tab is absent"),
            );
            return;
        }
        if let GuiFeedPill::Subscription(feed_id) = &pill
            && !self.gui_feeds_subscription_known(feed_id)
        {
            Self::log_stale_library_item("feedsSetPill", feed_id);
            return;
        }
        self.gui_feeds_selection.pill = pill;
    }

    /// Select the entry the Hero shows (row 11.1).
    ///
    /// The selection the core owns, so the frame carries it back. The entry's
    /// own fields are already in the fetched list, so no load starts. An entry
    /// the selected pill's list does not carry is a stale frame's pick and
    /// changes nothing.
    pub fn gui_feeds_select(&mut self, entry: &str) {
        if !self.gui_feeds_present() {
            Self::log_ignored_library_command(
                "feedsSelect",
                format_args!("the feed tab is absent"),
            );
            return;
        }
        if !self.gui_feeds_entry_known(entry) {
            Self::log_stale_library_item("feedsSelect", entry);
            return;
        }
        self.gui_feeds_selection.picked_entry = Some(entry.to_string());
    }

    /// The frame-path trigger for the active Feeds destination (row 11.1).
    ///
    /// The stored pill converges on the subscription set the tab holds, so a
    /// removed subscription's pill falls back to Latest instead of naming a
    /// dead list. The trigger starts no load — the fetch is the shared startup
    /// step's — and it is idempotent, so the frame path runs it on every
    /// frame.
    pub fn ensure_gui_feeds_selection(&mut self) {
        if !self.gui_feeds_present() || !self.tab.is_feeds() {
            return;
        }
        if let GuiFeedPill::Subscription(feed_id) = &self.gui_feeds_selection.pill
            && !self.gui_feeds_subscription_known(feed_id)
        {
            self.gui_feeds_selection.pill = GuiFeedPill::Latest;
        }
    }

    /// The pill the frame paints (row 11.1).
    ///
    /// The stored pill, or Latest when the session never selected one or a
    /// removed subscription took it away. The read-side fallback keeps a dead
    /// pill off the frame even before the ensure converges the store.
    #[must_use]
    pub fn gui_feeds_pill(&self) -> GuiFeedPill {
        match &self.gui_feeds_selection.pill {
            GuiFeedPill::Subscription(feed_id) if !self.gui_feeds_subscription_known(feed_id) => {
                GuiFeedPill::Latest
            }
            pill => pill.clone(),
        }
    }

    /// The entry the Hero shows (row 11.1): the stored pick. The projection
    /// falls back to the selected pill's first row when the pick left the
    /// list, so the Hero's default is never blank.
    #[must_use]
    pub fn gui_feeds_picked_entry(&self) -> Option<&str> {
        self.gui_feeds_selection.picked_entry.as_deref()
    }

    /// The entries the selected pill lists (row 11.1), in App order.
    ///
    /// Latest is the merged `all_entries`, and a subscription pill is that
    /// subscription's own list. Both are newest first with undated entries
    /// last, because `FeedTabState::rebuild_all_entries` sorted them when the
    /// fetch landed. Empty while the subscription the pill names holds no
    /// fetched entries.
    #[must_use]
    pub fn gui_feeds_entries(&self) -> Vec<&FeedEntry> {
        match self.gui_feeds_pill() {
            GuiFeedPill::Latest => self.feed_tab.all_entries.iter().collect(),
            GuiFeedPill::Subscription(feed_id) => self
                .gui_feeds_subscription_index(&feed_id)
                .and_then(|index| self.feed_tab.entries.get(index))
                .map(|entries| entries.iter().collect())
                .unwrap_or_default(),
        }
    }

    /// Whether the tab still holds the subscription `feed_id` names (row
    /// 11.1): the URL the fetch stamps on each entry and the launch-neutral
    /// identity the pill carries.
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

    /// Whether the selected pill's list carries the entry whose `guid` is
    /// `entry` (row 11.1): a landed list admits only an entry it holds, and
    /// an empty list admits nothing, because no row is painted to pick.
    fn gui_feeds_entry_known(&self, entry: &str) -> bool {
        self.gui_feeds_entries()
            .iter()
            .any(|known| known.guid == entry)
    }
}
