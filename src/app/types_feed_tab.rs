use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;
use std::sync::mpsc;

/// Result sent from a background fetch thread back to the Feeds tab.
pub(super) struct FeedTabRefreshResult {
    pub subscription_index: usize,
    pub entries: Result<Vec<FeedEntry>, String>,
}

/// Mutable state held by the Feeds tab.
pub(super) struct FeedTabState {
    /// Configured subscriptions copied from `Config.feeds` at startup.
    pub subscriptions: Vec<FeedSubscription>,
    /// All fetched entries, one vec per subscription (indexed by
    /// `subscription_index`).
    pub entries: Vec<Vec<FeedEntry>>,
    /// Combined entries for the "All" group, sorted by `pub_date_secs`
    /// descending with `None` dates last.
    pub all_entries: Vec<FeedEntry>,
    /// Which group is selected: 0 = "All", 1+ = subscription index 0+.
    pub selected_group: usize,
    /// Cursor into the currently-visible entry list.
    pub cursor: usize,
    /// Scroll offset for the currently-visible entry list.
    pub scroll: usize,
    /// True while a refresh is in progress.
    pub loading: bool,
    /// Number of background fetch results still expected. When this
    /// reaches zero, `loading` transitions to `false`.
    pub pending_results: usize,
    /// Channel receiver for background fetch results. `None` after first
    /// take (drained once per tick).
    pub refresh_rx: Option<mpsc::Receiver<FeedTabRefreshResult>>,
    /// Channel sender clone for spawning fetches.
    pub refresh_tx: mpsc::Sender<FeedTabRefreshResult>,
}

impl Default for FeedTabState {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            subscriptions: Vec::new(),
            entries: Vec::new(),
            all_entries: Vec::new(),
            selected_group: 0,
            cursor: 0,
            scroll: 0,
            loading: false,
            pending_results: 0,
            refresh_rx: Some(rx),
            refresh_tx: tx,
        }
    }
}

impl FeedTabState {
    /// Returns the currently visible entries based on `selected_group`.
    pub fn visible_entries(&self) -> &[FeedEntry] {
        if self.selected_group == 0 {
            &self.all_entries
        } else {
            let idx = self.selected_group - 1;
            self.entries.get(idx).map(|e| e.as_slice()).unwrap_or(&[])
        }
    }

    /// Rebuild `all_entries` from per-subscription `entries`, sorted by
    /// `pub_date_secs` descending (None dates last).
    pub fn rebuild_all_entries(&mut self) {
        self.all_entries.clear();
        for sub_entries in &self.entries {
            self.all_entries.extend(sub_entries.iter().cloned());
        }
        self.all_entries
            .sort_by(|a, b| match (a.pub_date_secs, b.pub_date_secs) {
                (Some(a), Some(b)) => b.cmp(&a), // newest first
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
    }

    /// Clamp `cursor` and `scroll` to valid bounds for the current group.
    pub fn clamp_state(&mut self) {
        let n = self.visible_entries().len();
        if n == 0 {
            self.cursor = 0;
            self.scroll = 0;
        } else {
            self.cursor = self.cursor.min(n - 1);
            self.scroll = self.scroll.min(self.cursor);
        }
    }

    /// Number of groups including "All".
    pub fn group_count(&self) -> usize {
        1 + self.subscriptions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, pub_date_secs: Option<u64>) -> FeedEntry {
        FeedEntry {
            guid: title.to_string(),
            title: title.to_string(),
            enclosure_url: None,
            link: Some(format!("https://example.test/{title}")),
            mime_type: None,
            duration_ticks: None,
            pub_date_secs,
            feed_kind: mbv_core::config::FeedKind::Video,
        }
    }

    #[test]
    fn all_group_sorted_newest_first_with_none_last() {
        let mut state = FeedTabState {
            entries: vec![
                vec![entry("old", Some(100)), entry("new", Some(300))],
                vec![entry("nodate", None), entry("mid", Some(200))],
            ],
            ..Default::default()
        };
        state.rebuild_all_entries();
        let titles: Vec<&str> = state.all_entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["new", "mid", "old", "nodate"]);
    }

    #[test]
    fn visible_entries_all_group() {
        let state = FeedTabState {
            selected_group: 0,
            all_entries: vec![entry("a", None), entry("b", None)],
            ..Default::default()
        };
        assert_eq!(state.visible_entries().len(), 2);
    }

    #[test]
    fn visible_entries_subscription_group() {
        let state = FeedTabState {
            entries: vec![vec![entry("x", None)], vec![entry("y", None)]],
            selected_group: 1,
            ..Default::default()
        };
        assert_eq!(state.visible_entries().len(), 1);
        assert_eq!(state.visible_entries()[0].title, "x");
    }

    #[test]
    fn clamp_state_works() {
        let mut state = FeedTabState {
            entries: vec![vec![entry("a", None)]],
            selected_group: 0,
            cursor: 99,
            scroll: 99,
            ..Default::default()
        };
        state.rebuild_all_entries();
        state.clamp_state();
        assert_eq!(state.cursor, 0);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn group_count_includes_all() {
        let mut state = FeedTabState {
            subscriptions: vec![],
            ..Default::default()
        };
        assert_eq!(state.group_count(), 1);
        state.subscriptions = vec![
            FeedSubscription {
                name: "a".into(),
                url: "http://a".into(),
                kind: mbv_core::config::FeedKind::Audio,
            },
            FeedSubscription {
                name: "b".into(),
                url: "http://b".into(),
                kind: mbv_core::config::FeedKind::Video,
            },
        ];
        assert_eq!(state.group_count(), 3);
    }
}
