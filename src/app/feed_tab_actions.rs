use super::feed_parse::fetch_and_parse_entries;
use super::notify_actions::ToastSeverity;
use super::types_feed_tab::FeedTabRefreshResult;
use super::{App, LibEvent};
use mbv_core::feed_entry_state::FeedEntryState;
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use std::collections::HashMap;

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

    /// Play the exact entry selected by the Feeds Interactive Component.
    pub(super) fn play_feed_entry(&mut self, entry: FeedEntry) {
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

    /// Enqueue the exact entry selected by the Feeds Interactive Component.
    pub(super) fn enqueue_feed_entry(&mut self, entry: FeedEntry) {
        if entry.primary_source().is_none() {
            self.flash(
                "Feed entry has no playable source".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.submit_queue_item(QueueItem::Feed(entry), false);
    }

    /// The Emby user id that keys local feed-entry state, or an empty sentinel
    /// for a feed-only client with no Emby Service. The store is machine-local,
    /// so the sentinel only has to be stable, not globally unique.
    fn feed_state_user_id(&self) -> String {
        self.config
            .lock()
            .unwrap()
            .emby_setup
            .as_ref()
            .map(|setup| setup.user_id.clone())
            .unwrap_or_default()
    }

    /// Copy stored playback state into one entry before it is played or queued.
    /// A missing feed identity or a missing stored row leaves the entry exactly
    /// as fetched.
    pub(super) fn hydrate_feed_entry_state(&mut self, mut entry: FeedEntry) -> FeedEntry {
        let Some(feed_id) = entry.feed_id.clone() else {
            return entry;
        };
        let user_id = self.feed_state_user_id();
        if let Some(state) = self.feed_entry_state.get(&user_id, &feed_id, &entry.guid) {
            entry.position_ticks = state.position_ticks;
            entry.played = state.played;
            log::info!(
                target: "feed_state",
                "hydrated feed entry guid={} feed_id={} pos={}s played={}",
                entry.guid,
                feed_id,
                state.position_ticks / mbv_core::api::TICKS_PER_SECOND,
                state.played,
            );
        }
        entry
    }

    /// Merge stored state into one subscription's freshly fetched entries with a
    /// single store read rather than one read per entry. Entries with no stored
    /// row stay as fetched (zero position, unplayed).
    pub(super) fn hydrate_feed_entries_for_subscription(
        &mut self,
        feed_id: &str,
        entries: &mut [FeedEntry],
    ) {
        let user_id = self.feed_state_user_id();
        let rows = self.feed_entry_state.scan(&user_id, feed_id);
        if rows.is_empty() {
            return;
        }
        let lookup: HashMap<&str, FeedEntryState> = rows
            .iter()
            .map(|(guid, state)| (guid.as_str(), *state))
            .collect();
        let mut hydrated = 0usize;
        for entry in entries.iter_mut() {
            if let Some(state) = lookup.get(entry.guid.as_str()) {
                entry.position_ticks = state.position_ticks;
                entry.played = state.played;
                hydrated += 1;
            }
        }
        if hydrated > 0 {
            log::info!(
                target: "feed_state",
                "bulk-hydrated {hydrated} entries for feed_id={feed_id}",
            );
        }
    }

    /// Store one entry's playback state and rewrite the state file. A failed
    /// write is logged and discarded: it never stops playback, and the
    /// previously written state stays intact.
    pub(super) fn write_feed_entry_state(
        &mut self,
        feed_id: &str,
        entry_guid: &str,
        position_ticks: i64,
        played: bool,
    ) {
        let user_id = self.feed_state_user_id();
        self.feed_entry_state.put(
            &user_id,
            feed_id,
            entry_guid,
            FeedEntryState {
                position_ticks,
                played,
            },
        );
        match self.feed_entry_state.save() {
            Ok(()) => log::info!(
                target: "feed_state",
                "wrote feed entry state guid={} feed_id={} pos={}s played={}",
                entry_guid,
                feed_id,
                position_ticks / mbv_core::api::TICKS_PER_SECOND,
                played,
            ),
            Err(error) => log::warn!(
                target: "feed_state",
                "feed state write failed guid={} feed_id={}: {error}",
                entry_guid,
                feed_id,
            ),
        }
    }

    /// Resolve an addressable Feed queue slot, derive its lifecycle state,
    /// update queue progress, and write the resulting `FeedEntryState` without
    /// invoking Emby progress reporting. `completed` is true for known-runtime
    /// EOF or stop at/above 95% -- played entries store position zero.
    /// Unknown-runtime EOF keeps `played` false.
    pub(super) fn persist_feed_slot_lifecycle(
        &mut self,
        slot_id: mbv_core::playback_queue::QueueSlotId,
        position_ticks: i64,
        completed: bool,
    ) {
        // Extract identity from the slot before any mutable borrow.
        let (feed_id, entry_guid) = {
            let queue = self.playback_queue();
            let Some(slot) = queue.queue.slot(slot_id) else {
                return;
            };
            let QueueItem::Feed(ref entry) = slot.item else {
                return;
            };
            let Some(ref feed_id) = entry.feed_id else {
                return;
            };
            (feed_id.clone(), entry.guid.clone())
        };
        let runtime = {
            let queue = self.playback_queue();
            queue
                .queue
                .slot(slot_id)
                .map(|s| s.item.runtime_ticks())
                .unwrap_or(0)
        };
        let (store_position, store_played) = if completed && runtime > 0 {
            (0, true)
        } else {
            (position_ticks, false)
        };
        let queue_mut = self.playback_queue_mut();
        let _ = queue_mut
            .queue
            .apply_progress(slot_id, store_position, store_played);
        self.write_feed_entry_state(&feed_id, &entry_guid, store_position, store_played);
    }
}
