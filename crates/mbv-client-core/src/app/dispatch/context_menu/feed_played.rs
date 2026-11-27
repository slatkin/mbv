//! Feed played-state marks: the local curation write, the immediate
//! browse-row apply, and the owner-queue propagation every accepted feed
//! mark shares.

use crate::app::App;

impl App {
    pub(in crate::app) fn set_feed_entries_played(
        &mut self,
        entries: &[mbv_queue::FeedEntry],
        played: bool,
    ) {
        let user_id = self
            .config
            .lock()
            .unwrap()
            .emby_setup
            .as_ref()
            .map_or_else(String::new, |setup| setup.user_id.clone());
        for entry in entries {
            if let Some(feed_id) = entry.feed_id.as_deref() {
                self.feed_entry_state
                    .set_played(&user_id, feed_id, &entry.guid, played);
            }
        }
        let _ = self.feed_entry_state.save();
        self.apply_stored_feed_entry_states(entries, &user_id);
        // Queue rows take the same mark through the owner: the stored state
        // is the local truth feed played-state curates, so the read-back pair
        // is what every matching non-active slot records.
        let updates = entries
            .iter()
            .filter_map(|entry| {
                let feed_id = entry.feed_id.as_deref()?;
                let state = self.feed_entry_state.get(&user_id, feed_id, &entry.guid)?;
                Some(mbv_ctrl::ProgressUpdate {
                    content_id: mbv_queue::QueueItemContentId::Feed(entry.guid.clone()),
                    position_ticks: state.position_ticks,
                    finished: state.played,
                })
            })
            .collect();
        self.propagate_played_to_queue(updates);
    }

    /// Copy each entry's stored state onto the loaded Feeds rows, so a mark
    /// shows at once without a refetch (design D5, spec `feed-queue-item`,
    /// #1005). Rows are matched by `(feed_id, guid)`; Home rows are not
    /// touched, and Queue rows take the mark through the owner propagation
    /// `set_feed_entries_played` sends after this apply.
    fn apply_stored_feed_entry_states(&mut self, entries: &[mbv_queue::FeedEntry], user_id: &str) {
        for entry in entries {
            let Some(feed_id) = entry.feed_id.as_deref() else {
                continue;
            };
            let Some(state) = self.feed_entry_state.get(user_id, feed_id, &entry.guid) else {
                continue;
            };
            for sub_entries in &mut self.feed_tab.entries {
                apply_feed_entry_state(sub_entries, feed_id, &entry.guid, state);
            }
            apply_feed_entry_state(&mut self.feed_tab.all_entries, feed_id, &entry.guid, state);
        }
    }
}

/// Copy one stored feed-entry state onto every loaded row with that
/// `(feed_id, guid)` identity (design D5, spec `feed-queue-item`, #1005).
fn apply_feed_entry_state(
    rows: &mut [mbv_queue::FeedEntry],
    feed_id: &str,
    entry_guid: &str,
    state: mbv_feed::FeedEntryState,
) {
    for row in rows {
        if row.feed_id.as_deref() == Some(feed_id) && row.guid == entry_guid {
            row.position_ticks = state.position_ticks;
            row.played = state.played;
        }
    }
}
