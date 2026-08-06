use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use mbv_core::api::MediaItem;
use std::cmp::Reverse;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SearchMode {
    Fuzzy,
    Global,
}

pub(crate) struct SearchModal {
    pub(super) mode: SearchMode,
    pub(super) query: String,
    pub(super) results: Vec<MediaItem>,
    pub(super) corpus: Vec<MediaItem>,
    pub(super) cursor: usize,
    pub(super) scroll: usize,
    pub(super) loading: bool,
    pub(super) type_filter: usize,
    pub(super) last_drain_error: Option<String>,
}

pub(crate) struct SearchModalDrainOutcome {
    pub(crate) received: usize,
    pub(crate) errors: Vec<String>,
}

fn is_navigable_type(item_type: &str) -> bool {
    matches!(
        item_type,
        "Series" | "Episode" | "Season" | "Movie" | "Audio" | "MusicAlbum" | "MusicArtist"
    )
}

fn score_corpus_against(corpus: &[MediaItem], query: &str) -> Vec<MediaItem> {
    if query.is_empty() {
        return corpus.to_vec();
    }
    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<(i64, &MediaItem)> = corpus
        .iter()
        .filter_map(|item| {
            matcher
                .fuzzy_match(&item.display_name(), query)
                .map(|score| (score, item))
        })
        .collect();
    scored.sort_unstable_by_key(|(score, _)| Reverse(*score));
    scored.into_iter().map(|(_, item)| item.clone()).collect()
}

impl SearchModal {
    pub(super) fn new(mode: SearchMode) -> Self {
        Self {
            mode,
            query: String::new(),
            results: Vec::new(),
            corpus: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
            type_filter: 0,
            last_drain_error: None,
        }
    }

    pub(super) fn on_query_changed(&mut self) {
        self.loading = true;
        self.results.clear();
        self.cursor = 0;
        self.scroll = 0;
        self.type_filter = 0;
        self.last_drain_error = None;
        if matches!(self.mode, SearchMode::Fuzzy) {
            self.results = score_corpus_against(&self.corpus, &self.query);
            if !self.corpus.is_empty() {
                self.loading = false;
            }
        }
    }

    pub(super) fn apply_drain(
        &mut self,
        query: &str,
        result: Result<Vec<MediaItem>, String>,
        errors: &mut Vec<String>,
    ) {
        if !matches!(self.mode, SearchMode::Global) {
            return;
        }
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
    use super::{SearchModal, SearchMode};
    use crate::app::render::LetterFilter;
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{App, BrowseLevel, LibraryTab, PanelFocus};
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;
    use std::cmp::Reverse;

    fn stub_library_with_root(
        all_items: Option<Vec<mbv_core::api::MediaItem>>,
        letter_filter: Option<LetterFilter>,
    ) -> App {
        let mut app = make_app_stub();
        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "movies-lib".into();
        library.collection_type = "movies".into();
        library.is_folder = true;
        let items_len = all_items.as_ref().map_or(0, |v| v.len());
        let root_level = BrowseLevel {
            parent_id: "movies-lib".into(),
            title: "Movies".into(),
            items: all_items.clone().unwrap_or_default(),
            total_count: items_len,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items,
            letter_filter,
            music_grouping: None,
        };
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![root_level],
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
        app
    }

    fn set_id(mut item: mbv_core::api::MediaItem, id: &str) -> mbv_core::api::MediaItem {
        item.id = id.into();
        item
    }

    #[test]
    fn fuzzy_results_sorted_by_score_desc() {
        let mut modal = SearchModal::new(SearchMode::Fuzzy);
        modal.corpus = vec![
            set_id(make_item("alpha", "Movie"), "1"),
            set_id(make_item("alphabet", "Movie"), "2"),
            set_id(make_item("zeta", "Movie"), "3"),
        ];
        modal.query = "alph".into();
        modal.on_query_changed();

        assert!(!modal.loading);
        assert_eq!(modal.results.len(), 2);
        assert!(modal.results.iter().all(|r| r.id != "3"));

        let matcher = SkimMatcherV2::default();
        let scores: Vec<i64> = modal
            .results
            .iter()
            .map(|r| matcher.fuzzy_match(&r.display_name(), "alph").unwrap())
            .collect();
        let mut sorted = scores.clone();
        sorted.sort_unstable_by_key(|s| Reverse(*s));
        assert_eq!(scores, sorted);
    }

    #[test]
    fn fuzzy_empty_query_returns_full_corpus() {
        let mut modal = SearchModal::new(SearchMode::Fuzzy);
        modal.corpus = vec![
            set_id(make_item("alpha", "Movie"), "1"),
            set_id(make_item("beta", "Movie"), "2"),
        ];
        modal.on_query_changed();

        assert!(!modal.loading);
        assert_eq!(modal.results.len(), 2);
    }

    #[test]
    fn fuzzy_empty_corpus_keeps_loading_true() {
        let mut modal = SearchModal::new(SearchMode::Fuzzy);
        modal.query = "anything".into();
        modal.on_query_changed();

        assert!(modal.loading);
        assert!(modal.results.is_empty());
    }

    #[test]
    fn global_drain_replaces_results_and_resets_state() {
        let mut modal = SearchModal::new(SearchMode::Global);
        modal.cursor = 5;
        modal.scroll = 4;
        modal.type_filter = 2;
        modal.loading = true;

        let items = vec![
            make_item("Movie 1", "Movie"),
            make_item("Series 1", "Series"),
        ];
        let mut errors = Vec::new();
        modal.apply_drain("", Ok(items), &mut errors);

        assert!(errors.is_empty());
        assert!(!modal.loading);
        assert_eq!(modal.cursor, 0);
        assert_eq!(modal.scroll, 0);
        assert_eq!(modal.type_filter, 0);
        assert_eq!(modal.results.len(), 2);
    }

    #[test]
    fn global_drain_error_clears_loading_preserves_results() {
        let mut modal = SearchModal::new(SearchMode::Global);
        modal.loading = true;
        let prior = vec![make_item("Previous", "Movie")];
        modal.results = prior.clone();

        let mut errors = Vec::new();
        modal.apply_drain("", Err("API timeout".into()), &mut errors);

        assert!(!modal.loading);
        assert_eq!(errors, vec!["API timeout".to_string()]);
        assert_eq!(modal.last_drain_error.as_deref(), Some("API timeout"));
        let prior_names: Vec<&str> = prior.iter().map(|r| r.name.as_str()).collect();
        let actual_names: Vec<&str> = modal.results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(actual_names, prior_names);
    }

    #[test]
    fn successful_drain_clears_last_drain_error() {
        let mut modal = SearchModal::new(SearchMode::Global);
        modal.loading = true;
        modal.last_drain_error = Some("API timeout".into());

        let mut errors = Vec::new();
        modal.apply_drain("", Ok(vec![make_item("Movie 1", "Movie")]), &mut errors);

        assert!(errors.is_empty());
        assert!(modal.last_drain_error.is_none());
    }

    #[test]
    fn on_query_changed_clears_last_drain_error() {
        let mut modal = SearchModal::new(SearchMode::Global);
        modal.last_drain_error = Some("API timeout".into());
        modal.query = "new".into();

        modal.on_query_changed();

        assert!(modal.last_drain_error.is_none());
    }

    #[test]
    fn global_drain_filters_unnavigable_types() {
        let mut modal = SearchModal::new(SearchMode::Global);
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
        modal.apply_drain("", Ok(items), &mut errors);

        assert!(errors.is_empty());
        assert_eq!(modal.results.len(), 4);
        let types: Vec<&str> = modal.results.iter().map(|r| r.item_type.as_str()).collect();
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
        let mut modal = SearchModal::new(SearchMode::Global);
        let items = vec![
            make_item("BoxSet 1", "BoxSet"),
            make_item("Book 1", "Book"),
            make_item("Photo 1", "Photo"),
        ];

        let mut errors = Vec::new();
        modal.apply_drain("", Ok(items), &mut errors);

        assert!(errors.is_empty());
        assert!(modal.results.is_empty());
    }

    #[test]
    fn global_drain_ignored_in_fuzzy_mode() {
        let mut modal = SearchModal::new(SearchMode::Fuzzy);
        modal.loading = true;
        modal.cursor = 3;

        let mut errors = Vec::new();
        modal.apply_drain("", Ok(vec![make_item("Anything", "Movie")]), &mut errors);

        assert!(modal.loading);
        assert_eq!(modal.cursor, 3);
        assert!(modal.results.is_empty());
    }

    #[test]
    fn stale_response_is_discarded_current_response_applies() {
        let mut modal = SearchModal::new(SearchMode::Global);
        modal.query = "a".into();
        modal.loading = true;
        modal.cursor = 5;
        // A newer keystroke arrives before the "a" response does.
        modal.query = "ab".into();

        let mut errors = Vec::new();
        modal.apply_drain("a", Ok(vec![make_item("Stale", "Movie")]), &mut errors);

        assert!(errors.is_empty());
        assert!(modal.loading, "stale response must not touch loading");
        assert_eq!(modal.cursor, 5);
        assert!(modal.results.is_empty());

        modal.apply_drain("ab", Ok(vec![make_item("Fresh", "Movie")]), &mut errors);

        assert!(!modal.loading);
        assert_eq!(modal.cursor, 0);
        assert_eq!(modal.results.len(), 1);
        assert_eq!(modal.results[0].name, "Fresh");
    }

    #[test]
    fn available_types_sorted_by_display_order() {
        let mut modal = SearchModal::new(SearchMode::Global);
        modal.results = vec![
            make_item("Series 1", "Series"),
            make_item("Movie 1", "Movie"),
            make_item("Episode 1", "Episode"),
        ];

        assert_eq!(modal.available_types(), vec!["Movie", "Series", "Episode"]);
    }

    #[test]
    fn open_search_modal_fuzzy_uses_root_corpus_not_current_level_items() {
        let root_items = make_items(3);
        let mut app = stub_library_with_root(Some(root_items.clone()), None);
        let mut nested = make_item("Nested", "Series");
        nested.id = "nested-1".into();
        let nested_level = BrowseLevel {
            parent_id: "series-1".into(),
            title: "Series 1".into(),
            items: vec![nested],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        };
        app.libs[0].nav_stack.push(nested_level);

        app.open_search_modal_fuzzy(0);

        let modal = app.search_modal.as_ref().unwrap();
        let corpus_ids: Vec<&str> = modal.corpus.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(corpus_ids.len(), 3);
        for root in &root_items {
            assert!(
                corpus_ids.contains(&root.id.as_str()),
                "missing root item {}",
                root.id
            );
        }
        assert!(!corpus_ids.contains(&"nested-1"));
    }

    #[test]
    fn open_search_modal_fuzzy_ignores_active_letter_filter() {
        let root_items = make_items(3);
        let mut app = stub_library_with_root(
            Some(root_items.clone()),
            Some(LetterFilter::default_filter()),
        );

        app.open_search_modal_fuzzy(0);

        let modal = app.search_modal.as_ref().unwrap();
        let corpus_ids: Vec<&str> = modal.corpus.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(corpus_ids.len(), 3);
        for root in &root_items {
            assert!(
                corpus_ids.contains(&root.id.as_str()),
                "missing root item {}",
                root.id
            );
        }
    }

    #[test]
    fn open_search_modal_fuzzy_with_no_corpus_marks_loading_and_yields_no_matches() {
        let mut app = stub_library_with_root(None, None);

        app.open_search_modal_fuzzy(0);

        let modal = app.search_modal.as_ref().unwrap();
        assert!(modal.corpus.is_empty());
        assert!(modal.loading);

        let mut modal = app.search_modal.take().unwrap();
        modal.query = "anything".into();
        modal.on_query_changed();
        assert!(modal.results.is_empty());
        assert!(modal.loading);
    }

    fn slash_press(app: &mut App) {
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('/'),
            crossterm::event::KeyModifiers::NONE,
        );
        let _ = app.handle_key_search_modal(key);
    }

    fn key(code: crossterm::event::KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn second_slash_within_window_promotes_fuzzy_to_global() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_fuzzy(0);
        app.last_slash_at = Some(std::time::Instant::now());

        slash_press(&mut app);

        let modal = app.search_modal.as_ref().expect("modal stays open");
        assert!(matches!(modal.mode, SearchMode::Global));
        assert!(modal.corpus.is_empty());
        assert!(modal.loading);
    }

    #[test]
    fn second_slash_outside_window_is_literal_in_fuzzy_with_empty_query() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_fuzzy(0);
        app.last_slash_at = Some(std::time::Instant::now() - std::time::Duration::from_millis(500));

        slash_press(&mut app);

        let modal = app.search_modal.as_ref().expect("modal stays open");
        assert!(
            matches!(modal.mode, SearchMode::Fuzzy),
            "promotion must only happen within SEARCH_PROMOTION_WINDOW"
        );
        assert_eq!(modal.query, "/", "the `/` is appended as a literal");
    }

    #[test]
    fn slash_after_typed_character_is_literal() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_fuzzy(0);
        app.search_modal.as_mut().unwrap().query = "abc".into();
        app.last_slash_at = Some(std::time::Instant::now());

        slash_press(&mut app);

        let modal = app.search_modal.as_ref().expect("modal stays open");
        assert!(
            matches!(modal.mode, SearchMode::Fuzzy),
            "non-empty query must block promotion"
        );
        assert_eq!(modal.query, "abc/");
    }

    #[test]
    fn promotion_preserves_empty_query() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_fuzzy(0);
        app.last_slash_at = Some(std::time::Instant::now());

        slash_press(&mut app);

        let modal = app.search_modal.as_ref().expect("modal stays open");
        assert!(matches!(modal.mode, SearchMode::Global));
        assert_eq!(modal.query, "");
    }

    #[test]
    fn home_tab_slash_opens_global_directly() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.library_tab = 0;

        let key = key(crossterm::event::KeyCode::Char('/'));
        let _ = app.handle_key(key);

        let modal = app
            .search_modal
            .as_ref()
            .expect("search modal opens from the home tab");
        assert!(matches!(modal.mode, SearchMode::Global));
    }

    #[test]
    fn activation_with_empty_results_is_inert_and_keeps_query() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_fuzzy(0);
        app.search_modal.as_mut().unwrap().query = "abc".into();
        app.search_modal.as_mut().unwrap().results.clear();

        let _ = app.handle_key_search_modal(key(crossterm::event::KeyCode::Enter));

        let modal = app
            .search_modal
            .as_ref()
            .expect("modal remains open when there is nothing to activate");
        assert_eq!(modal.query, "abc");
    }

    #[test]
    fn esc_dismisses_global_mode_in_one_press() {
        let mut app = make_app_stub();
        app.open_search_modal_global();

        let _ = app.handle_key_search_modal(key(crossterm::event::KeyCode::Esc));

        assert!(app.search_modal.is_none());
    }

    #[test]
    fn activation_with_a_selected_result_closes_the_modal() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_fuzzy(0);
        let mut item = make_item("Target", "Movie");
        item.id = "target-1".into();
        app.search_modal.as_mut().unwrap().results = vec![item];

        let _ = app.handle_key_search_modal(key(crossterm::event::KeyCode::Enter));

        assert!(
            app.search_modal.is_none(),
            "Enter on a selectable result closes the modal"
        );
    }

    #[test]
    fn typing_in_global_mode_dispatches_after_two_chars_and_debounce() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.open_search_modal_global();
        // Opening global mode no longer dispatches an empty query.
        assert!(app.search_rx.try_recv().is_err());

        // One character: query stays pending, nothing dispatched yet.
        let _ = app.handle_key_search_modal(key(crossterm::event::KeyCode::Char('a')));
        assert!(app.search_debounce_deadline.is_none());
        assert!(app.search_rx.try_recv().is_err());

        // Two characters: pending query is set, but not yet sent.
        let _ = app.handle_key_search_modal(key(crossterm::event::KeyCode::Char('b')));
        assert!(app.search_debounce_pending.is_some());
        assert!(app.search_rx.try_recv().is_err());

        // Advance past the debounce deadline and flush.
        app.search_debounce_deadline = Some(std::time::Instant::now() - std::time::Duration::from_millis(400));
        assert!(app.maybe_flush_search_debounce());
        assert_eq!(app.search_debounce_pending, None);
        assert_eq!(app.search_debounce_deadline, None);

        assert!(
            app.search_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok(),
            "debounced query must dispatch after flush"
        );
    }

    #[test]
    fn esc_restores_prior_panel_focus() {
        let mut app = stub_library_with_root(Some(make_items(3)), None);
        app.panel_focus = PanelFocus::Queue;
        app.open_search_modal_fuzzy(0);
        assert_eq!(app.panel_focus, PanelFocus::Queue);

        app.dismiss_search_modal();

        assert_eq!(
            app.panel_focus,
            PanelFocus::Queue,
            "Esc must restore the focus that was active when the modal opened"
        );
        assert!(app.search_modal_prior_focus.is_none());
    }
}
