use mbv_core::api::EmbyItem;

pub(crate) struct SearchSidebar {
    pub(super) query: String,
    pub(super) results: Vec<EmbyItem>,
    pub(super) cursor: usize,
    pub(super) scroll: usize,
    pub(super) loading: bool,
    pub(super) type_filter: usize,
    pub(super) last_drain_error: Option<String>,
    /// Visible list rows; written by the renderer, read by cursor movement.
    pub(super) list_height: usize,
}

pub(crate) struct SearchDrainOutcome {
    pub(crate) received: usize,
    pub(crate) errors: Vec<String>,
}

fn is_navigable_type(item_type: &str) -> bool {
    matches!(
        item_type,
        "Series" | "Episode" | "Season" | "Movie" | "Audio" | "MusicAlbum" | "MusicArtist"
    )
}

impl SearchSidebar {
    pub(super) fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
            type_filter: 0,
            last_drain_error: None,
            list_height: 0,
        }
    }

    pub(super) fn on_query_changed(&mut self) {
        self.loading = true;
        self.results.clear();
        self.cursor = 0;
        self.scroll = 0;
        self.type_filter = 0;
        self.last_drain_error = None;
    }

    pub(super) fn apply_drain(
        &mut self,
        query: &str,
        result: Result<Vec<EmbyItem>, String>,
        errors: &mut Vec<String>,
    ) {
        // A faster keystroke can dispatch a newer query while an older one
        // is still in flight; responses race on arrival order, not send
        // order. Discard anything that isn't answering the live query,
        // and leave `loading` untouched -- a request for the current query
        // may still be in flight.
        if query != self.query {
            return;
        }
        self.loading = false;
        self.cursor = 0;
        self.scroll = 0;
        self.type_filter = 0;
        match result {
            Ok(items) => {
                self.results = items
                    .into_iter()
                    .filter(|item| is_navigable_type(&item.item_type))
                    .collect();
                self.last_drain_error = None;
            }
            Err(error) => {
                self.last_drain_error = Some(error.clone());
                errors.push(error);
            }
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

    pub(super) fn filtered_results(&self) -> Vec<&EmbyItem> {
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

    fn type_sort_key(t: &str) -> u8 {
        match t {
            "Movie" => 0,
            "Series" => 1,
            "Season" => 2,
            "Episode" => 3,
            "Audio" => 4,
            "MusicAlbum" => 5,
            "MusicArtist" => 6,
            _ => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SearchSidebar;
    use crate::app::tests::make_item;

    #[test]
    fn global_drain_replaces_results_and_resets_state() {
        let mut sidebar = SearchSidebar::new();
        sidebar.cursor = 5;
        sidebar.scroll = 4;
        sidebar.type_filter = 2;
        sidebar.loading = true;

        let items = vec![
            make_item("Movie 1", "Movie"),
            make_item("Series 1", "Series"),
        ];
        let mut errors = Vec::new();
        sidebar.apply_drain("", Ok(items), &mut errors);

        assert!(errors.is_empty());
        assert!(!sidebar.loading);
        assert_eq!(sidebar.cursor, 0);
        assert_eq!(sidebar.scroll, 0);
        assert_eq!(sidebar.type_filter, 0);
        assert_eq!(sidebar.results.len(), 2);
    }

    #[test]
    fn global_drain_error_clears_loading_preserves_results() {
        let mut sidebar = SearchSidebar::new();
        sidebar.loading = true;
        let prior = vec![make_item("Previous", "Movie")];
        sidebar.results = prior.clone();

        let mut errors = Vec::new();
        sidebar.apply_drain("", Err("API timeout".into()), &mut errors);

        assert!(!sidebar.loading);
        assert_eq!(errors, vec!["API timeout".to_string()]);
        assert_eq!(sidebar.last_drain_error.as_deref(), Some("API timeout"));
        let prior_names: Vec<&str> = prior.iter().map(|r| r.name.as_str()).collect();
        let actual_names: Vec<&str> = sidebar.results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(actual_names, prior_names);
    }

    #[test]
    fn successful_drain_clears_last_drain_error() {
        let mut sidebar = SearchSidebar::new();
        sidebar.loading = true;
        sidebar.last_drain_error = Some("API timeout".into());

        let mut errors = Vec::new();
        sidebar.apply_drain("", Ok(vec![make_item("Movie 1", "Movie")]), &mut errors);

        assert!(errors.is_empty());
        assert!(sidebar.last_drain_error.is_none());
    }

    #[test]
    fn on_query_changed_clears_last_drain_error() {
        let mut sidebar = SearchSidebar::new();
        sidebar.last_drain_error = Some("API timeout".into());
        sidebar.query = "new".into();

        sidebar.on_query_changed();

        assert!(sidebar.last_drain_error.is_none());
    }

    #[test]
    fn global_drain_filters_unnavigable_types() {
        let mut sidebar = SearchSidebar::new();
        let items = vec![
            make_item("Movie 1", "Movie"),
            make_item("Series 1", "Series"),
            make_item("Audio 1", "Audio"),
            make_item("Album 1", "MusicAlbum"),
            make_item("BoxSet 1", "BoxSet"),
            make_item("Book 1", "Book"),
            make_item("Photo 1", "Photo"),
        ];

        let mut errors = Vec::new();
        sidebar.apply_drain("", Ok(items), &mut errors);

        assert!(errors.is_empty());
        assert_eq!(sidebar.results.len(), 4);
        let types: Vec<&str> = sidebar
            .results
            .iter()
            .map(|r| r.item_type.as_str())
            .collect();
        assert!(types.contains(&"Movie"));
        assert!(types.contains(&"Series"));
        assert!(types.contains(&"Audio"));
        assert!(types.contains(&"MusicAlbum"));
        assert!(!types.contains(&"BoxSet"));
        assert!(!types.contains(&"Book"));
        assert!(!types.contains(&"Photo"));
    }

    #[test]
    fn global_drain_all_unnavigable_yields_empty() {
        let mut sidebar = SearchSidebar::new();
        let items = vec![
            make_item("BoxSet 1", "BoxSet"),
            make_item("Book 1", "Book"),
            make_item("Photo 1", "Photo"),
        ];

        let mut errors = Vec::new();
        sidebar.apply_drain("", Ok(items), &mut errors);

        assert!(errors.is_empty());
        assert!(sidebar.results.is_empty());
    }

    #[test]
    fn stale_response_is_discarded_current_response_applies() {
        let mut sidebar = SearchSidebar::new();
        sidebar.query = "a".into();
        sidebar.loading = true;
        sidebar.cursor = 5;
        // A newer keystroke arrives before the "a" response does.
        sidebar.query = "ab".into();

        let mut errors = Vec::new();
        sidebar.apply_drain("a", Ok(vec![make_item("Stale", "Movie")]), &mut errors);

        assert!(errors.is_empty());
        assert!(sidebar.loading, "stale response must not touch loading");
        assert_eq!(sidebar.cursor, 5);
        assert!(sidebar.results.is_empty());

        sidebar.apply_drain("ab", Ok(vec![make_item("Fresh", "Movie")]), &mut errors);

        assert!(!sidebar.loading);
        assert_eq!(sidebar.cursor, 0);
        assert_eq!(sidebar.results.len(), 1);
        assert_eq!(sidebar.results[0].name, "Fresh");
    }

    #[test]
    fn available_types_sorted_by_display_order() {
        let mut sidebar = SearchSidebar::new();
        sidebar.results = vec![
            make_item("Series 1", "Series"),
            make_item("Movie 1", "Movie"),
            make_item("Episode 1", "Episode"),
        ];

        assert_eq!(
            sidebar.available_types(),
            vec!["Movie", "Series", "Episode"]
        );
    }
}
