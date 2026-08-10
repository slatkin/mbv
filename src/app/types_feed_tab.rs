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
            Self::Watched => "Watched",
            Self::Unwatched => "Unwatched",
        }
    }

    fn matches(self, played: bool) -> bool {
        match self {
            Self::All => true,
            Self::Watched => played,
            Self::Unwatched => !played,
        }
    }
}

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
    /// descending with `None` dates last. Individual subscription entries use
    /// the same ordering.
    pub all_entries: Vec<FeedEntry>,
    /// Active watched-state filter.
    pub watched_filter: WatchedFilter,
    /// Filtered view of the currently selected group's entries, rebuilt
    /// whenever the group, filter, or source entries change.
    filtered_entries: Vec<FeedEntry>,
    /// Which group is selected: 0 = "All", 1+ = subscription index 0+.
    pub selected_group: usize,
    /// Cursor into the currently-visible entry list.
    pub cursor: usize,
    /// Scroll offset for the currently-visible display rows.
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
            watched_filter: WatchedFilter::default(),
            filtered_entries: Vec::new(),
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
    /// Returns the currently visible entries after applying the active
    /// watched filter to the selected group's source entries.
    pub fn visible_entries(&self) -> &[FeedEntry] {
        &self.filtered_entries
    }

    /// Rebuild the filtered entries view from the selected group's source
    /// entries and the active watched filter.
    pub fn rebuild_filtered_entries(&mut self) {
        let source = if self.selected_group == 0 {
            &self.all_entries
        } else {
            let idx = self.selected_group - 1;
            self.entries.get(idx).map(|e| e.as_slice()).unwrap_or(&[])
        };
        self.filtered_entries.clear();
        if self.watched_filter == WatchedFilter::All {
            self.filtered_entries.extend(source.iter().cloned());
        } else {
            self.filtered_entries.extend(
                source
                    .iter()
                    .filter(|e| self.watched_filter.matches(e.played))
                    .cloned(),
            );
        }
    }

    /// Cycle the watched filter and reset cursor/scroll.
    pub fn cycle_watched_filter(&mut self) {
        self.watched_filter = self.watched_filter.cycle();
        self.cursor = 0;
        self.scroll = 0;
        self.rebuild_filtered_entries();
    }

    /// Rebuild `all_entries` from per-subscription `entries`, sorting each
    /// subscription and the combined list by `pub_date_secs` descending
    /// (None dates last). Also rebuilds the filtered view.
    pub fn rebuild_all_entries(&mut self) {
        self.all_entries.clear();
        for sub_entries in &mut self.entries {
            sort_entries_newest_first(sub_entries);
            self.all_entries.extend(sub_entries.iter().cloned());
        }
        sort_entries_newest_first(&mut self.all_entries);
        self.rebuild_filtered_entries();
    }

    /// Clamp the cursor to a valid entry for the current group. Display-row
    /// scroll is clamped by the renderer because its bounds depend on the
    /// terminal viewport and age headings.
    pub fn clamp_state(&mut self) {
        let n = self.visible_entries().len();
        if n == 0 {
            self.cursor = 0;
            self.scroll = 0;
        } else {
            self.cursor = self.cursor.min(n - 1);
            // This is only a coarse bound; the renderer applies the precise
            // display-row bound once it knows the viewport height.
            self.scroll = self.scroll.min(n - 1);
        }
    }

    /// Number of groups including "All".
    pub fn group_count(&self) -> usize {
        1 + self.subscriptions.len()
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
    fn visible_entries_all_group() {
        let mut state = FeedTabState {
            selected_group: 0,
            all_entries: vec![entry("a", None), entry("b", None)],
            ..Default::default()
        };
        state.rebuild_filtered_entries();
        assert_eq!(state.visible_entries().len(), 2);
    }

    #[test]
    fn visible_entries_subscription_group() {
        let mut state = FeedTabState {
            entries: vec![vec![entry("x", None)], vec![entry("y", None)]],
            selected_group: 1,
            ..Default::default()
        };
        state.rebuild_filtered_entries();
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

    fn played_entry(title: &str, pub_date_secs: Option<u64>) -> FeedEntry {
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
            played: true,
        }
    }

    #[test]
    fn watched_filter_cycle_order() {
        let mut state = FeedTabState::default();
        assert_eq!(state.watched_filter, WatchedFilter::All);
        state.cycle_watched_filter();
        assert_eq!(state.watched_filter, WatchedFilter::Watched);
        state.cycle_watched_filter();
        assert_eq!(state.watched_filter, WatchedFilter::Unwatched);
        state.cycle_watched_filter();
        assert_eq!(state.watched_filter, WatchedFilter::All);
    }

    #[test]
    fn watched_filter_shows_only_played() {
        let mut state = FeedTabState {
            all_entries: vec![
                entry("unplayed", Some(100)),
                played_entry("played", Some(200)),
                entry("also-unplayed", Some(300)),
            ],
            ..Default::default()
        };
        state.rebuild_filtered_entries(); // builds with All filter
        assert_eq!(state.visible_entries().len(), 3);

        state.cycle_watched_filter(); // -> Watched
        let titles: Vec<&str> = state
            .visible_entries()
            .iter()
            .map(|e| e.title.as_str())
            .collect();
        assert_eq!(titles, vec!["played"]);
    }

    #[test]
    fn unwatched_filter_shows_only_unplayed() {
        let mut state = FeedTabState {
            all_entries: vec![
                entry("unplayed", Some(100)),
                played_entry("played", Some(200)),
                entry("also-unplayed", Some(300)),
            ],
            ..Default::default()
        };
        state.rebuild_filtered_entries();
        state.cycle_watched_filter(); // -> Watched
        state.cycle_watched_filter(); // -> Unwatched
        let titles: Vec<&str> = state
            .visible_entries()
            .iter()
            .map(|e| e.title.as_str())
            .collect();
        assert_eq!(titles, vec!["unplayed", "also-unplayed"]);
    }

    #[test]
    fn watched_filter_empty_result() {
        let mut state = FeedTabState {
            all_entries: vec![
                entry("unplayed-a", Some(100)),
                entry("unplayed-b", Some(200)),
            ],
            ..Default::default()
        };
        state.rebuild_filtered_entries();
        state.cycle_watched_filter(); // -> Watched (no played entries)
        assert_eq!(state.visible_entries().len(), 0);
    }

    #[test]
    fn filter_cycle_resets_cursor_and_scroll() {
        let mut state = FeedTabState {
            entries: vec![vec![
                entry("a", Some(100)),
                entry("b", Some(200)),
                entry("c", Some(300)),
            ]],
            cursor: 2,
            scroll: 1,
            ..Default::default()
        };
        state.rebuild_all_entries();
        assert_eq!(state.cursor, 2);
        state.cycle_watched_filter();
        assert_eq!(state.cursor, 0);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn filter_applies_to_subscription_group() {
        let mut state = FeedTabState {
            entries: vec![vec![
                entry("unplayed", Some(100)),
                played_entry("played", Some(200)),
            ]],
            selected_group: 1,
            ..Default::default()
        };
        state.rebuild_all_entries();
        assert_eq!(state.visible_entries().len(), 2);

        state.cycle_watched_filter(); // -> Watched
        assert_eq!(state.visible_entries().len(), 1);
        assert_eq!(state.visible_entries()[0].title, "played");
    }

    #[test]
    fn group_change_reflects_active_filter() {
        let mut state = FeedTabState {
            entries: vec![
                vec![
                    entry("a-unplayed", Some(100)),
                    played_entry("a-played", Some(200)),
                ],
                vec![
                    entry("b-unplayed", Some(300)),
                    played_entry("b-played", Some(400)),
                ],
            ],
            all_entries: vec![
                entry("a-unplayed", Some(100)),
                played_entry("a-played", Some(200)),
                entry("b-unplayed", Some(300)),
                played_entry("b-played", Some(400)),
            ],
            watched_filter: WatchedFilter::Watched,
            ..Default::default()
        };
        state.rebuild_filtered_entries();
        // All group, Watched filter
        assert_eq!(state.visible_entries().len(), 2);

        state.selected_group = 1;
        state.rebuild_filtered_entries();
        assert_eq!(state.visible_entries().len(), 1);
        assert_eq!(state.visible_entries()[0].title, "a-played");
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
        for entry in entries.iter_mut() {
            if let Some(state) = lookup.get(entry.guid.as_str()) {
                entry.position_ticks = state.position_ticks;
                entry.played = state.played;
            }
        }

        assert_eq!(entries[0].position_ticks, 5000);
        assert!(!entries[0].played);
        assert_eq!(entries[1].position_ticks, 0);
        assert!(!entries[1].played); // not in scanned
        assert_eq!(entries[2].position_ticks, 0);
        assert!(entries[2].played);
    }
}
