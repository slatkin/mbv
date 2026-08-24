use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;
use std::sync::mpsc;

/// Watched-state filter for the Feeds tab. Cycles
/// `All -> Watched -> Unwatched -> All` on unmodified `w`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum WatchedFilter {
    #[default]
    All,
    Watched,
    Unwatched,
}

impl WatchedFilter {
    /// Next filter in the cycle.
    pub fn cycle(self) -> Self {
        match self {
            Self::All => Self::Watched,
            Self::Watched => Self::Unwatched,
            Self::Unwatched => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Watched => "Played",
            Self::Unwatched => "Unplayed",
        }
    }

    pub(super) fn matches(self, played: bool) -> bool {
        match self {
            Self::All => true,
            Self::Watched => played,
            Self::Unwatched => !played,
        }
    }
}

/// Result sent from a background fetch thread back to the Feeds tab.
pub(super) struct FeedTabRefreshResult {
    pub feed_id: String,
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
    /// descending with `None` dates last. Individual subscription entries use
    /// the same ordering.
    pub all_entries: Vec<FeedEntry>,
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
            loading: false,
            pending_results: 0,
            refresh_rx: Some(rx),
            refresh_tx: tx,
        }
    }
}

impl FeedTabState {
    /// Rebuild `all_entries` from per-subscription `entries`, sorting each
    /// subscription and the combined list by `pub_date_secs` descending
    /// (None dates last).
    pub fn rebuild_all_entries(&mut self) {
        self.all_entries.clear();
        for sub_entries in &mut self.entries {
            sort_entries_newest_first(sub_entries);
            self.all_entries.extend(sub_entries.iter().cloned());
        }
        sort_entries_newest_first(&mut self.all_entries);
    }
}

fn sort_entries_newest_first(entries: &mut [FeedEntry]) {
    entries.sort_by(|a, b| match (a.pub_date_secs, b.pub_date_secs) {
        (Some(a), Some(b)) => b.cmp(&a), // newest first
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
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
            feed_kind: Some(mbv_core::config::FeedKind::Video),
            feed_id: None,
            position_ticks: 0,
            played: false,
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
    fn subscription_groups_are_sorted_newest_first_with_none_last() {
        let mut state = FeedTabState {
            entries: vec![vec![
                entry("old", Some(100)),
                entry("new", Some(300)),
                entry("nodate", None),
            ]],
            ..Default::default()
        };
        state.rebuild_all_entries();
        let titles: Vec<&str> = state.entries[0]
            .iter()
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(titles, vec!["new", "old", "nodate"]);
    }

    #[test]
    fn hydration_merges_by_guid_and_ignores_unknown() {
        use mbv_core::shared_store::FeedEntryState;

        let mut entries = [
            entry("ep-a", Some(100)),
            entry("ep-b", Some(200)),
            entry("ep-c", Some(300)),
        ];
        let scanned = [
            (
                "ep-a".to_string(),
                FeedEntryState {
                    position_ticks: 5000,
                    played: false,
                },
            ),
            (
                "ep-c".to_string(),
                FeedEntryState {
                    position_ticks: 0,
                    played: true,
                },
            ),
            (
                "unknown-guid".to_string(),
                FeedEntryState {
                    position_ticks: 9999,
                    played: true,
                },
            ),
        ];
        let lookup: std::collections::HashMap<&str, &FeedEntryState> = scanned
            .iter()
            .map(|(guid, state)| (guid.as_str(), state))
            .collect();
        for entry in &mut entries {
            if let Some(state) = lookup.get(entry.guid.as_str()) {
                entry.position_ticks = state.position_ticks;
                entry.played = state.played;
            }
        }

        assert_eq!(entries[0].position_ticks, 5000);
        assert!(!entries[0].played);
        assert_eq!(entries[1].position_ticks, 0);
        assert!(!entries[1].played);
        assert_eq!(entries[2].position_ticks, 0);
        assert!(entries[2].played);
    }
}
