use std::sync::mpsc;

use mbv_core::api::{EmbyClient, MediaItem};

pub(crate) struct HomeSearch {
    pub(super) query: String,
    pub(super) last_query: String,
    pub(super) results: Vec<MediaItem>,
    pub(super) cursor: usize,
    pub(super) loading: bool,
    pub(super) scroll: usize,
    pub(super) type_filter: usize,
    pub(super) input_focused: bool,
}

impl HomeSearch {
    fn type_sort_key(t: &str) -> u8 {
        match t {
            "Movie" => 0,
            "Series" => 1,
            "Episode" => 2,
            "Audio" => 3,
            "MusicAlbum" => 4,
            "MusicArtist" => 5,
            _ => 6,
        }
    }

    pub(super) fn new(input_focused: bool) -> Self {
        Self {
            query: String::new(),
            last_query: String::new(),
            results: Vec::new(),
            cursor: 0,
            loading: false,
            scroll: 0,
            type_filter: 0,
            input_focused,
        }
    }

    pub(super) fn available_types(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut types: Vec<&str> = self
            .results
            .iter()
            .filter_map(|r| {
                let t = r.item_type.as_str();
                if seen.insert(t) {
                    Some(t)
                } else {
                    None
                }
            })
            .collect();
        types.sort_by_key(|t| Self::type_sort_key(t));
        types
    }

    pub(super) fn filtered_results(&self) -> Vec<&MediaItem> {
        let types = self.available_types();
        let filter = if self.type_filter == 0 {
            None
        } else {
            types.get(self.type_filter - 1).copied()
        };
        self.results
            .iter()
            .filter(|r| filter.is_none_or(|t| r.item_type == t))
            .collect()
    }

    pub(super) fn filtered_count(&self) -> usize {
        self.filtered_results().len()
    }
}

pub(super) struct SearchDrainOutcome {
    pub(super) received: usize,
    pub(super) errors: Vec<String>,
}

pub(super) struct SearchSubsystem {
    home_search: Option<HomeSearch>,
    search_tx: mpsc::Sender<Result<Vec<MediaItem>, String>>,
    search_rx: mpsc::Receiver<Result<Vec<MediaItem>, String>>,
}

impl SearchSubsystem {
    pub(super) fn new(
        search_tx: mpsc::Sender<Result<Vec<MediaItem>, String>>,
        search_rx: mpsc::Receiver<Result<Vec<MediaItem>, String>>,
    ) -> Self {
        Self {
            home_search: None,
            search_tx,
            search_rx,
        }
    }

    pub(super) fn state(&self) -> Option<&HomeSearch> {
        self.home_search.as_ref()
    }

    pub(super) fn state_mut(&mut self) -> Option<&mut HomeSearch> {
        self.home_search.as_mut()
    }

    pub(super) fn is_open(&self) -> bool {
        self.home_search.is_some()
    }

    pub(super) fn open(&mut self, input_focused: bool) {
        self.home_search = Some(HomeSearch::new(input_focused));
    }

    pub(super) fn close(&mut self) {
        self.home_search = None;
    }

    #[cfg(test)]
    pub(super) fn set_state_for_test(&mut self, state: Option<HomeSearch>) {
        self.home_search = state;
    }

    pub(super) fn prepare_query(&mut self, query: &str) {
        if let Some(home_search) = self.home_search.as_mut() {
            home_search.last_query = query.to_string();
            home_search.loading = true;
            home_search.results.clear();
            home_search.cursor = 0;
            home_search.scroll = 0;
        }
    }

    pub(super) fn spawn_global_search(&self, client: EmbyClient, query: String) {
        let tx = self.search_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(client.search_items(&query, 100));
        });
    }

    pub(super) fn drain_results(&mut self) -> SearchDrainOutcome {
        let mut outcome = SearchDrainOutcome {
            received: 0,
            errors: Vec::new(),
        };
        while let Ok(result) = self.search_rx.try_recv() {
            outcome.received += 1;
            if let Some(home_search) = self.home_search.as_mut() {
                home_search.loading = false;
                home_search.cursor = 0;
                home_search.scroll = 0;
                home_search.type_filter = 0;
                match result {
                    Ok(items) => {
                        home_search.results = items;
                    }
                    Err(error) => {
                        home_search.results.clear();
                        outcome.errors.push(error);
                    }
                }
            }
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::{HomeSearch, SearchSubsystem};
    use crate::app::tests::make_item;

    #[test]
    fn drain_results_updates_state_and_resets_filter_cursor() {
        let (search_tx, search_rx) = std::sync::mpsc::channel();
        let mut search = SearchSubsystem::new(search_tx.clone(), search_rx);
        search.open(false);
        let state = search.state_mut().unwrap();
        state.cursor = 5;
        state.scroll = 4;
        state.type_filter = 2;
        state.loading = true;
        search_tx
            .send(Ok(vec![
                make_item("Movie", "Movie"),
                make_item("Series", "Series"),
            ]))
            .unwrap();

        let outcome = search.drain_results();

        assert_eq!(outcome.received, 1);
        assert!(outcome.errors.is_empty());
        let state = search.state().unwrap();
        assert!(!state.loading);
        assert_eq!(state.cursor, 0);
        assert_eq!(state.scroll, 0);
        assert_eq!(state.type_filter, 0);
        assert_eq!(state.results.len(), 2);
    }

    #[test]
    fn home_search_available_types_keep_expected_sort_order() {
        let mut search = HomeSearch::new(true);
        search.results = vec![
            make_item("Series", "Series"),
            make_item("Movie", "Movie"),
            make_item("Episode", "Episode"),
        ];

        assert_eq!(search.available_types(), vec!["Movie", "Series", "Episode"]);
    }
}
