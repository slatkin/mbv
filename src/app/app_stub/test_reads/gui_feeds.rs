//! The Feeds destination's test seeds and pushes (add-gui-library-panel row
//! 11.1): the configured subscriptions the tab holds and one background
//! fetch's completion on the App's own result channel, so a Client's test
//! owns every entry its assertions see and no worker thread or network runs
//! behind it.
//!
//! The group is cohesive — the one destination whose data arrives from a
//! worker this App drains — so it sits in its own child module of
//! `test_reads` and keeps that file within its line ceiling.

use super::super::App;

impl App {
    /// Seed the Feeds tab's configured subscriptions (add-gui-library-panel
    /// row 11.1), as the construction's `sync_feed_subscriptions` copy of the
    /// config leaves them: `(name, url)` pairs, with one empty entry list per
    /// subscription. The fetch itself never runs, so a Client's test owns
    /// every entry its assertions see.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_feeds_for_test(&mut self, subscriptions: &[(&str, &str)]) {
        self.feed_tab.subscriptions = subscriptions
            .iter()
            .map(|(name, url)| mbv_config::FeedSubscription {
                name: (*name).to_string(),
                url: (*url).to_string(),
                kind: mbv_queue::FeedKind::Audio,
            })
            .collect();
        self.feed_tab.entries = vec![Vec::new(); self.feed_tab.subscriptions.len()];
        self.feed_tab.all_entries.clear();
    }

    /// Push one subscription fetch's completion onto the App's own result
    /// channel (add-gui-library-panel row 11.1), as a completed background
    /// fetch sends it: `(guid, publish instant in unix seconds)` rows whose
    /// title is the guid and whose enclosure is a playable URL. The tick's
    /// `drain_feed_tab_results` is the App code that applies it, so no worker
    /// thread and no network run behind the assertion.
    #[cfg(any(test, feature = "test"))]
    pub fn send_gui_feed_fetch_result_for_test(
        &mut self,
        subscription_index: usize,
        entries: &[(&str, Option<u64>)],
    ) {
        let Some(subscription) = self.feed_tab.subscriptions.get(subscription_index) else {
            return;
        };
        let feed_id = subscription.url.clone();
        let result = mbv_ui_model::feed_tab::FeedTabRefreshResult {
            feed_id: feed_id.clone(),
            subscription_index,
            entries: Ok(entries
                .iter()
                .map(|(guid, pub_date_secs)| mbv_queue::FeedEntry {
                    guid: (*guid).to_string(),
                    title: (*guid).to_string(),
                    enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
                    link: None,
                    mime_type: Some("audio/mpeg".to_string()),
                    duration_ticks: None,
                    pub_date_secs: *pub_date_secs,
                    feed_kind: Some(mbv_queue::FeedKind::Audio),
                    feed_id: Some(feed_id.clone()),
                    position_ticks: 0,
                    played: false,
                })
                .collect()),
        };
        let _ = self.feed_tab.refresh_tx.send(result);
    }
}
