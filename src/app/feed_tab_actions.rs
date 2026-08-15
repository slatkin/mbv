use super::feed_parse::fetch_and_parse_entries;
use super::notify_actions::ToastSeverity;
use super::types_feed_tab::FeedTabRefreshResult;
use super::App;
use mbv_core::playback_queue::{PlaybackQueue, QueueItem};

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
            self.feed_tab.clamp_state();
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
        self.flash("Refreshing feeds...".into(), ToastSeverity::Neutral);
    }

    /// Move the feed tab cursor by `delta` rows (clamped).
    pub(super) fn feed_tab_move_cursor(&mut self, delta: i64) {
        let n = self.feed_tab.visible_entries().len();
        if n == 0 {
            return;
        }
        let cur = self.feed_tab.cursor as i64;
        self.feed_tab.cursor = (cur + delta).clamp(0, n as i64 - 1) as usize;
    }

    /// Jump the feed tab cursor to the start or end.
    pub(super) fn feed_tab_jump_cursor(&mut self, to_end: bool) {
        let n = self.feed_tab.visible_entries().len();
        if n == 0 {
            return;
        }
        self.feed_tab.cursor = if to_end { n - 1 } else { 0 };
    }

    /// Cycle the feed tab group selection by `delta` (wrapping).
    pub(super) fn feed_tab_cycle_group(&mut self, delta: i64) {
        let n = self.feed_tab.group_count();
        if n == 0 {
            return;
        }
        let cur = self.feed_tab.selected_group as i64;
        self.feed_tab.selected_group = (cur + delta).rem_euclid(n as i64) as usize;
        self.feed_tab.cursor = 0;
        self.feed_tab.scroll = 0;
        self.feed_tab.rebuild_filtered_entries();
        self.feed_tab.clamp_state();
    }

    /// Select a specific group by index.
    pub(super) fn feed_tab_select_group(&mut self, group_idx: usize) {
        if group_idx < self.feed_tab.group_count() {
            self.feed_tab.selected_group = group_idx;
            self.feed_tab.cursor = 0;
            self.feed_tab.scroll = 0;
            self.feed_tab.rebuild_filtered_entries();
            self.feed_tab.clamp_state();
        }
    }

    /// Page the feed tab cursor up or down.
    pub(super) fn feed_tab_page_cursor(&mut self, page_size: usize, forward: bool) {
        let n = self.feed_tab.visible_entries().len();
        if n == 0 {
            return;
        }
        let cur = self.feed_tab.cursor;
        if forward {
            self.feed_tab.cursor = (cur + page_size).min(n - 1);
        } else {
            self.feed_tab.cursor = cur.saturating_sub(page_size);
        }
    }

    /// Play the entry at the current cursor (§5.5/§5.6). A no-op for an
    /// empty list, an out-of-range cursor, or an entry with no playable
    /// enclosure/link -- the source validation below avoids spawning mpv
    /// for a doomed play and gives the user an explanatory toast instead.
    ///
    /// Routes through the same `PlayerProxy::submit_queue` boundary as
    /// library-item Play: the entry is appended to the canonical
    /// `PlaybackQueue`, the cursor is set, and the full queue is submitted
    /// to the player.  For remote players this sends `UnifiedQueueReplace`
    /// when the daemon supports the capability; for local players it
    /// replaces the mpv playlist in place or cold-starts a fresh process.
    pub(super) fn feed_tab_play_selected(&mut self) {
        let Some(entry) = self
            .feed_tab
            .visible_entries()
            .get(self.feed_tab.cursor)
            .cloned()
        else {
            return;
        };
        if entry.primary_source().is_none() {
            self.flash(
                "Feed entry has no playable source".into(),
                ToastSeverity::Error,
            );
            return;
        }
        // Hydrate stored position/played before appending to the queue.
        let entry = self.hydrate_feed_entry_state(entry);
        // Append to the canonical queue (local or remote mirror) so the
        // queue panel, cursor, and now-playing title all resolve the Feed
        // entry correctly.  On a cold (inactive) local player the player
        // will spawn a fresh thread with only this Feed entry, so clear
        // any stale items first.
        let active = self.player.status.lock().unwrap().active;
        let is_remote = self.player.is_remote();
        let scope = self.playback_target_queue_scope();
        let previous_queue = self.queue_for_scope(scope).clone();
        if !active && !is_remote {
            self.playback_queue_mut().queue = PlaybackQueue::from_items(Vec::new(), None);
        }
        let queue = self.playback_queue_mut();
        let existing_idx = queue.queue.slots().iter().position(
            |slot| matches!(&slot.item, QueueItem::Feed(existing) if existing.guid == entry.guid),
        );
        let unified_idx = match existing_idx {
            Some(index) => index,
            None => {
                queue.queue.append(QueueItem::Feed(entry.clone()));
                queue.queue.slots().len() - 1
            }
        };
        queue.queue_cursor = unified_idx;
        let _ = queue
            .queue
            .set_active_slot(queue.queue.slots()[unified_idx].slot_id);
        // Submit through the same unified path as library Play: the full
        // canonical queue (including any pre-existing items) is handed to
        // the player so its internal playlist matches the PlayerTab.
        let all_items = self.playback_queue().all_queue_items();
        let start_idx = self.playback_queue().queue_cursor;
        let headless = all_items.iter().all(QueueItem::is_audio);
        if existing_idx.is_some() && self.player.is_remote() {
            let slot_id = self
                .playback_queue()
                .slot_id_at(unified_idx)
                .expect("existing playback queue slot disappeared");
            if !self
                .player
                .queue_play_slot(mbv_core::ctrl::slot_id_to_u64(slot_id))
            {
                *self.queue_for_scope_mut(scope) = previous_queue;
                self.flash(
                    "Playback owner rejected the Feed selection".into(),
                    ToastSeverity::Error,
                );
            }
            return;
        }
        if !self
            .player
            .submit_queue(all_items, start_idx, None, headless, self.ui_volume)
        {
            *self.queue_for_scope_mut(scope) = previous_queue;
            self.flash(
                "Playback owner rejected the Feed entry".into(),
                ToastSeverity::Error,
            );
        }
    }

    /// Enqueue the entry at the current cursor into the canonical queue
    /// without starting playback, using the same append path as library
    /// enqueue.
    pub(super) fn feed_tab_enqueue_selected(&mut self) {
        let Some(entry) = self
            .feed_tab
            .visible_entries()
            .get(self.feed_tab.cursor)
            .cloned()
        else {
            return;
        };
        if entry.primary_source().is_none() {
            self.flash(
                "Feed entry has no playable source".into(),
                ToastSeverity::Error,
            );
            return;
        }
        let scope = self.visible_queue_scope();
        let previous_dirty = self.queue_dirty;
        let previous_queue = self.queue_for_scope(scope).clone();
        self.queue_for_scope_mut(scope)
            .queue
            .append(QueueItem::Feed(entry.clone()));
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        if self.sync_playback_queue_items_after_append(scope, vec![QueueItem::Feed(entry)]) {
            self.persist_local_queue_state_if_needed(scope);
            self.retire_remote_tracking(true);
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
    }
}
