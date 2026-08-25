use super::feed_parse::fetch_and_parse_entries;
use super::notify_actions::ToastSeverity;
use super::types_feed_tab::FeedTabRefreshResult;
use super::{App, LibEvent};
use mbv_core::playback_queue::QueueItem;

impl App {
    /// Whether feed subscriptions are configured and the Feeds tab should
    /// be visible.
    pub(super) fn has_feeds_subscriptions(&self) -> bool {
        !self.feed_tab.subscriptions.is_empty()
    }

    /// The 1-based tab position of the Feeds tab, or `None` when no
    /// subscriptions exist.
    pub(super) fn feeds_tab_pos(&self) -> Option<usize> {
        if self.has_feeds_subscriptions() {
            Some(1 + self.libs.len() + self.audiobookshelf_libraries.len())
        } else {
            None
        }
    }

    /// Copy configured subscriptions from the client config into the
    /// feed tab state. Called once at startup and when the config is
    /// reloaded.
    pub(super) fn sync_feed_subscriptions(&mut self) {
        let subs = self.config.lock().unwrap().feeds.clone();
        self.feed_tab.subscriptions = subs;
        // Ensure per-subscription entries vec is the right length.
        let n = self.feed_tab.subscriptions.len();
        self.feed_tab.entries.resize_with(n, Vec::new);
    }

    pub(super) fn feed_latest_items(&self) -> Vec<QueueItem> {
        self.feed_tab
            .all_entries
            .iter()
            .cloned()
            .map(QueueItem::Feed)
            .collect()
    }

    /// Drain completed background fetch results from the channel.
    pub(super) fn drain_feed_tab_results(&mut self) -> bool {
        let mut had_events = false;
        // Take the receiver once; we'll put it back after draining.
        let rx = match self.feed_tab.refresh_rx.take() {
            Some(rx) => rx,
            None => return false,
        };
        // Drain all available results.
        while let Ok(result) = rx.try_recv() {
            had_events = true;
            let feed_id = result.feed_id;
            let idx = result.subscription_index;
            let is_current_subscription = self
                .feed_tab
                .subscriptions
                .get(idx)
                .is_some_and(|subscription| subscription.url == feed_id);
            if is_current_subscription {
                match result.entries {
                    Ok(mut entries) => {
                        self.hydrate_feed_entries_for_subscription(&feed_id, &mut entries);
                        if let Some(slot) = self.feed_tab.entries.get_mut(idx) {
                            *slot = entries;
                        }
                    }
                    Err(e) => {
                        self.flash(
                            format!("Feed '{}' refresh failed: {e}", {
                                self.feed_tab
                                    .subscriptions
                                    .get(idx)
                                    .map(|s| s.name.as_str())
                                    .unwrap_or("?")
                            }),
                            ToastSeverity::Error,
                        );
                    }
                }
            }
            // Decrement outstanding count; loading stays true until every
            // spawned request has produced a result.
            self.feed_tab.pending_results = self.feed_tab.pending_results.saturating_sub(1);
        }
        // Re-insert the receiver so future results can arrive.
        self.feed_tab.refresh_rx = Some(rx);

        self.feed_tab.loading = self.feed_tab.pending_results > 0;

        if had_events {
            self.feed_tab.rebuild_all_entries();
            // Reflect the freshly loaded entries in Home's "Feeds" pill: the
            // App computes the section (honoring `hidden_latest`), then the
            // shell merges it into Model-owned `latest` at the lib_rx drain
            // (task 5.3d). The feed drain runs after that drain, so the pill
            // lands on the next loop pass — a bounded one-iteration latency.
            let _ = self.lib_tx.send(LibEvent::FeedsLatestRebuilt(
                self.feeds_latest_section().into_iter().collect(),
            ));
        }
        had_events
    }

    /// Start a manual refresh of all configured feed subscriptions.
    /// Does nothing if already loading.
    pub(super) fn refresh_feeds(&mut self) {
        if self.feed_tab.loading {
            self.flash(
                "Feeds refresh already in progress".into(),
                ToastSeverity::Neutral,
            );
            return;
        }
        if self.feed_tab.subscriptions.is_empty() {
            self.flash(
                "No feed subscriptions configured".into(),
                ToastSeverity::Neutral,
            );
            return;
        }
        self.start_feed_fetch();
        self.flash("Refreshing feeds...".into(), ToastSeverity::Neutral);
    }

    /// Spawn one background fetch per configured feed subscription, marking the
    /// Feeds tab loading until every result drains. Shared by the manual
    /// `refresh_feeds` (which adds a user-facing flash) and the async startup
    /// auto-fetch (which stays silent). Does nothing if already loading or if
    /// no subscriptions are configured.
    pub(super) fn start_feed_fetch(&mut self) {
        if self.feed_tab.loading || self.feed_tab.subscriptions.is_empty() {
            return;
        }
        self.feed_tab.loading = true;
        self.feed_tab.pending_results = self.feed_tab.subscriptions.len();
        let tx = self.feed_tab.refresh_tx.clone();
        for (idx, sub) in self.feed_tab.subscriptions.iter().enumerate() {
            let url = sub.url.clone();
            let feed_id = url.clone();
            let tx = tx.clone();
            let kind = sub.kind;
            std::thread::spawn(move || {
                let result = fetch_and_parse_entries(&url, kind, &feed_id);
                let _ = tx.send(FeedTabRefreshResult {
                    feed_id,
                    subscription_index: idx,
                    entries: result,
                });
            });
        }
    }

    /// Resolve a component request against the shell-owned, newest-first
    /// combined Feed snapshot.
    pub(super) fn feed_tab_play_guid(&mut self, guid: &str) {
        let Some(entry) = self
            .feed_tab
            .all_entries
            .iter()
            .find(|entry| entry.guid == guid)
            .cloned()
        else {
            return;
        };
        self.play_feed_entry(entry);
    }

    fn play_feed_entry(&mut self, entry: mbv_core::playback_queue::FeedEntry) {
        if entry.primary_source().is_none() {
            self.flash(
                "Feed entry has no playable source".into(),
                ToastSeverity::Error,
            );
            return;
        }
        // Hydrate stored position/played before appending to the queue.
        let entry = self.hydrate_feed_entry_state(entry);
        self.submit_queue_item(QueueItem::Feed(entry), true);
    }

    /// Resolve a component request against the shell-owned, newest-first
    /// combined Feed snapshot.
    pub(super) fn feed_tab_enqueue_guid(&mut self, guid: &str) {
        let Some(entry) = self
            .feed_tab
            .all_entries
            .iter()
            .find(|entry| entry.guid == guid)
            .cloned()
        else {
            return;
        };
        self.enqueue_feed_entry(entry);
    }

    fn enqueue_feed_entry(&mut self, entry: mbv_core::playback_queue::FeedEntry) {
        if entry.primary_source().is_none() {
            self.flash(
                "Feed entry has no playable source".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.submit_queue_item(QueueItem::Feed(entry), false);
    }
}
