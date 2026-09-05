use super::notify_actions::ToastSeverity;
use super::types_tab_selection::TabSelection;
use super::App;
use mbv_core::config::FeedSubscription;

impl App {
    /// Effect for `ConfirmAction::RemoveFeedSubscription`'s "yes" answer
    /// (§6.3): rewrites `config.feeds` without the removed entry.
    pub(super) fn remove_feed_confirmed(&mut self, index: usize) {
        let feeds: Vec<FeedSubscription> = {
            let c = self.config.lock().unwrap();
            if index >= c.feeds.len() {
                return;
            }
            c.feeds
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, s)| s.clone())
                .collect()
        };
        self.persist_feeds(feeds);
    }

    /// Writes `feeds` into App's general config and persists it via the
    /// existing read-modify-write toml merge (§6.3), then runs the §6.4
    /// post-mutation resync.
    pub(super) fn persist_feeds(&mut self, feeds: Vec<FeedSubscription>) {
        let cfg = {
            let mut c = self.config.lock().unwrap();
            c.feeds = feeds;
            c.clone()
        };
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "config save failed: {e}");
            self.flash(
                format!("Feed change saved but config save failed ({e})"),
                ToastSeverity::Error,
            );
        }
        self.after_feeds_mutation();
    }

    /// After every subscription mutation (§6.4): resync shell-owned Feed
    /// data, clear fetched entries (no auto-fetch), and fall back to Home if
    /// the last subscription was removed while Feeds is selected. The mounted
    /// FeedsComponent resets its local selection when the subscription
    /// identity changes during the next shell sync.
    pub(super) fn after_feeds_mutation(&mut self) {
        self.sync_feed_subscriptions();
        let n = self.feed_tab.subscriptions.len();
        self.feed_tab.entries = vec![Vec::new(); n];
        self.feed_tab.all_entries.clear();
        if n == 0 && self.tab.is_feeds() {
            self.tab = TabSelection::Home;
        }
    }
}
