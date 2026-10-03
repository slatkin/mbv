use crate::app::App;
use crate::app::state::events::AudiobookshelfEvent;

/// Applies one podcast catalog page to `state`: a pending replacement stages
/// and is published only when its traversal completes (design D2), otherwise
/// the published chain appends. Returns the next page the active chain needs.
fn apply_show_page(
    state: &mut mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState,
    request: u64,
    page: mbv_audiobookshelf::AudiobookshelfShowPage,
) -> Option<usize> {
    if state.replacement.is_none() {
        state.append_page(page.page, page.limit, page.total, page.items);
        return state.needs_page();
    }
    state.append_replacement_page(request, page.page, page.limit, page.total, page.items);
    let next_page = state.replacement_needs_page(request);
    if next_page.is_none() {
        state.commit_catalog_replacement(request);
    }
    next_page
}

/// Book-shaped sibling of `apply_show_page`.
fn apply_book_page(
    state: &mut mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState,
    request: u64,
    page: mbv_audiobookshelf::AudiobookshelfBookPage,
) -> Option<usize> {
    if state.replacement.is_none() {
        state.append_page_books(page.page, page.total, page.items);
        return state.needs_page();
    }
    state.append_replacement_page(request, page.page, page.total, page.items);
    let next_page = state.replacement_needs_page(request);
    if next_page.is_none() {
        state.commit_catalog_replacement(request);
    }
    next_page
}

/// Discards a failed podcast replacement and, when the retained published
/// catalog was still mid-load, resumes its own traversal now that the
/// superseded mark is gone (#745). Returns the next published page to request.
fn abort_show_replacement(
    state: &mut mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState,
    request: u64,
) -> Option<usize> {
    if !state.abort_catalog_replacement(request) {
        return None;
    }
    state.needs_page()
}

/// Book-shaped sibling of `abort_show_replacement`.
fn abort_book_replacement(
    state: &mut mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState,
    request: u64,
) -> Option<usize> {
    if !state.abort_catalog_replacement(request) {
        return None;
    }
    state.needs_page()
}

impl App {
    pub(super) fn handle_audiobookshelf_event(&mut self, ev: AudiobookshelfEvent) {
        match ev {
            AudiobookshelfEvent::DetailFetched {
                generation,
                request,
                library_item_id,
                result,
            } => self.handle_audiobookshelf_podcast_detail_fetched(
                generation,
                request,
                library_item_id,
                result,
            ),
            AudiobookshelfEvent::ShowsFetched {
                generation,
                request,
                library_id,
                result,
            } => self.handle_audiobookshelf_shows_fetched(generation, request, library_id, result),
            AudiobookshelfEvent::BooksFetched {
                generation,
                request,
                library_id,
                result,
            } => self.handle_audiobookshelf_books_fetched(generation, request, library_id, result),
            AudiobookshelfEvent::ShelfFetched {
                generation,
                library_id,
                result,
            } => self.handle_audiobookshelf_shelf_fetched(generation, library_id, result),
            AudiobookshelfEvent::BookDetailLoaded {
                generation,
                request,
                library_item_id,
                result,
            } => {
                self.handle_audiobookshelf_book_detail_fetched(
                    generation,
                    request,
                    &library_item_id,
                    result,
                );
            }
        }
    }

    /// The `AudiobookshelfEvent::BooksFetched` body: append the fetched page to the
    /// library's book browse state, fetch the selected book's detail, and
    /// chain the next page when one is needed.
    pub(super) fn handle_audiobookshelf_books_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        request: u64,
        library_id: String,
        result: Result<
            mbv_audiobookshelf::AudiobookshelfBookPage,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        if let Some(index) = self
            .audiobookshelf_libraries
            .iter()
            .position(|library| library.id == library_id)
        {
            let mut next_page = None;
            let mut selected_detail = None;
            if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
                // A page from a superseded pre-refresh chain is discarded
                // whole: it must neither append to nor clear newer content.
                if state.catalog_request != request {
                    return;
                }
                match result {
                    Ok(page) => {
                        next_page = apply_book_page(state, request, page);
                        selected_detail =
                            state.selected_id.clone().filter(|_| !state.detail_loading);
                    }
                    Err(error) => {
                        state.error = Some(error.to_string());
                        next_page = abort_book_replacement(state, request);
                    }
                }
            }
            if let Some(selected_detail) = selected_detail {
                self.start_audiobookshelf_book_detail(selected_detail);
            }
            if let Some(next_page) = next_page {
                crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
                    self.config.lock().unwrap().clone(),
                    generation,
                    request,
                    library_id,
                    next_page,
                    self.channels.lib_tx.clone(),
                );
            }
        }
    }

    /// The `AudiobookshelfEvent::BookDetailFetched` body: retire the in-flight mark
    /// on the owning browse state and cache the detail on success.
    pub(super) fn handle_audiobookshelf_book_detail_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        request: u64,
        library_item_id: &str,
        result: Result<
            (
                Vec<mbv_audiobookshelf::AudiobookshelfChapter>,
                Vec<mbv_audiobookshelf::AudiobookshelfAudioFile>,
            ),
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
            state
                .books
                .iter()
                .any(|book| book.library_item_id == library_item_id)
        }) else {
            return;
        };
        // The response belongs to this book only when its in-flight mark
        // still carries this serial: a superseded response (a newer request
        // for the same book was issued, e.g. by a refresh) is discarded whole
        // — it must neither retire the newer request's mark nor overwrite a
        // newer entry.
        if state.detail_loading_ids.get(library_item_id) != Some(&request) {
            return;
        }
        state.detail_loading_ids.remove(library_item_id);
        state.detail_loading = state
            .selected_id
            .as_ref()
            .is_some_and(|id| state.detail_loading_ids.contains_key(id));
        if let Ok(detail) = result {
            state
                .detail_cache
                .insert(library_item_id.to_owned(), detail);
        }
    }

    pub(super) fn handle_audiobookshelf_podcast_detail_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        request: u64,
        library_item_id: String,
        result: Result<
            Vec<mbv_audiobookshelf::AudiobookshelfDownloadedEpisode>,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    ) {
        let index = self.audiobookshelf_browse.iter().position(|state| {
            state
                .shows
                .iter()
                .any(|show| show.library_item_id == library_item_id)
        });
        let Some(state) = index.and_then(|index| self.audiobookshelf_browse.get_mut(index)) else {
            return;
        };
        // The response belongs to this state only when the show's in-flight
        // mark still carries its request serial: an orphaned response (its
        // mark cleared by a refresh) or a superseded one (a newer request for
        // the show was issued) is discarded whole — it must neither retire
        // the newer request's mark nor write the cache over a newer entry.
        if state.detail_loading_ids.get(&library_item_id) != Some(&request) {
            return;
        }
        state.detail_loading_ids.remove(&library_item_id);
        // The mark is retired and the batch re-armed on the rejected-generation
        // path too: a generation bump between spawn and arrival must not leak
        // the in-flight slot and permanently stall the remaining shows behind
        // the bounded cap. A rejected payload is still never cached.
        if self.audiobookshelf_runtime.accepts(generation) {
            match result {
                Ok(episodes) => state.cache_detail(library_item_id, episodes),
                Err(_error) => {
                    // A failed refresh keeps the show's prior episodes
                    // (design D2). A show with no prior content caches an
                    // empty result so the bounded fan-out does not re-issue
                    // it forever (design D5).
                    if !state.detail_cache.contains_key(&library_item_id) {
                        state.cache_detail(library_item_id, Vec::new());
                    }
                }
            }
        }
        // The fan-out continues its bounded batch: the next required show's
        // request starts as this one retires (design D5).
        if let Some(index) = index {
            self.start_audiobookshelf_podcast_fan_out(index);
        }
    }

    pub(super) fn handle_audiobookshelf_shows_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        request: u64,
        library_id: String,
        result: Result<
            mbv_audiobookshelf::AudiobookshelfShowPage,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        if let Some(index) = self
            .audiobookshelf_libraries
            .iter()
            .position(|library| library.id == library_id)
        {
            let mut next_page = None;
            if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
                // A page from a superseded pre-refresh chain is discarded
                // whole: it must neither append to nor clear newer content.
                if state.catalog_request != request {
                    return;
                }
                match result {
                    Ok(page) => next_page = apply_show_page(state, request, page),
                    Err(error) => {
                        state.error = Some(error.to_string());
                        next_page = abort_show_replacement(state, request);
                    }
                }
            }
            // A landed page may list shows the active pill's fan-out has not
            // requested yet (a state pill requires every show); the scheduler
            // re-arms idempotently and stays bounded (design D5).
            self.start_audiobookshelf_podcast_fan_out(index);
            if let Some(next_page) = next_page {
                crate::app::dispatch::session::service_startup::start_audiobookshelf_shows(
                    self.config.lock().unwrap().clone(),
                    generation,
                    request,
                    library_id,
                    next_page,
                    self.channels.lib_tx.clone(),
                );
            }
        }
    }

    pub(super) fn handle_audiobookshelf_shelf_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            Vec<mbv_audiobookshelf::AudiobookshelfShelf>,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        if let Ok(shelves) = result {
            let items = App::newest_episodes_items(shelves);
            self.audiobookshelf_shelf_cache.insert(library_id, items);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::state::events::{AudiobookshelfEvent, LibEvent};
    use mbv_audiobookshelf::{
        AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookPage, AudiobookshelfChapter,
        AudiobookshelfError, AudiobookshelfFailureClass, AudiobookshelfLibrary,
    };
    use mbv_core::service_runtime::SetupGeneration;
    use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use rstest::rstest;

    fn book(id: &str) -> AudiobookshelfBook {
        AudiobookshelfBook {
            library_item_id: id.into(),
            title: id.into(),
            author_display: None,
            author_sort_key: "Author".into(),
            cover_path: None,
            duration_seconds: 1.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        }
    }

    fn app_with_book_state() -> crate::app::App {
        let mut app = crate::app::tests::make_app_stub();
        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        app.audiobookshelf_libraries.push(library.clone());
        app.audiobookshelf_book_browse
            .push(AudiobookshelfBookBrowseState::new(library));
        app
    }

    #[test]
    fn books_fetched_appends_page_and_selects_first_book() {
        let mut app = app_with_book_state();
        app.handle_lib_event(LibEvent::Audiobookshelf(
            AudiobookshelfEvent::BooksFetched {
                generation: SetupGeneration::default(),
                request: 0,
                library_id: "books".into(),
                result: Ok(AudiobookshelfBookPage {
                    page: 0,
                    limit: 20,
                    total: 21,
                    items: vec![book("book-a")],
                }),
            },
        ));

        let state = &app.audiobookshelf_book_browse[0];
        assert_eq!(state.books.len(), 1);
        assert_eq!(state.selected_id.as_deref(), Some("book-a"));
        assert_eq!(state.total, 21);
        assert_eq!(state.next_page, 1);
        assert!(state.error.is_none());
    }

    #[test]
    fn books_fetched_stages_a_replacement_and_commits_on_completion() {
        let mut app = app_with_book_state();
        app.audiobookshelf_book_browse[0].books.push(book("book-a"));
        let request = app.audiobookshelf_book_browse[0].begin_catalog_replacement();
        app.handle_lib_event(LibEvent::Audiobookshelf(
            AudiobookshelfEvent::BooksFetched {
                generation: SetupGeneration::default(),
                request,
                library_id: "books".into(),
                result: Ok(AudiobookshelfBookPage {
                    page: 0,
                    limit: 20,
                    total: 1,
                    items: vec![book("book-b")],
                }),
            },
        ));

        let state = &app.audiobookshelf_book_browse[0];
        assert!(state.replacement.is_none(), "the completed batch commits");
        assert_eq!(
            state
                .books
                .iter()
                .map(|b| b.library_item_id.as_str())
                .collect::<Vec<_>>(),
            ["book-b"],
            "the authoritative replacement publishes"
        );
    }

    fn books_fetched_does_not_apply(
        app: &mut crate::app::App,
        library_id: &str,
        stale: bool,
        request: u64,
    ) {
        if stale {
            app.audiobookshelf_runtime.begin_setup();
        }
        app.handle_lib_event(LibEvent::Audiobookshelf(
            AudiobookshelfEvent::BooksFetched {
                generation: SetupGeneration::default(),
                request,
                library_id: library_id.into(),
                result: Ok(AudiobookshelfBookPage {
                    page: 0,
                    limit: 20,
                    total: 1,
                    items: vec![book("book-a")],
                }),
            },
        ));
    }

    #[rstest]
    #[case::stale_generation("books", true, 0)]
    #[case::superseded_request("books", false, 0)]
    fn books_fetched_ignores_stale_generation_or_superseded_request(
        #[case] library_id: &str,
        #[case] stale_generation: bool,
        #[case] request: u64,
    ) {
        let mut app = app_with_book_state();
        if !stale_generation {
            // A newer catalog chain is current (as a refresh would establish);
            // the older page must be discarded whole.
            app.audiobookshelf_book_browse[0].begin_catalog_replacement();
        }

        books_fetched_does_not_apply(&mut app, library_id, stale_generation, request);

        assert_eq!(
            app.audiobookshelf_book_browse[0].books,
            [] as [mbv_audiobookshelf::AudiobookshelfBook; 0]
        );
    }

    fn detail_result(
        succeeds: bool,
    ) -> Result<(Vec<AudiobookshelfChapter>, Vec<AudiobookshelfAudioFile>), AudiobookshelfError>
    {
        if succeeds {
            Ok((
                vec![AudiobookshelfChapter {
                    id: 2,
                    start: 0.0,
                    end: 1.0,
                    title: "Chapter".into(),
                }],
                vec![AudiobookshelfAudioFile {
                    index: 0,
                    ino: "file".into(),
                    duration: 1.0,
                }],
            ))
        } else {
            Err(AudiobookshelfError::from_class(
                AudiobookshelfFailureClass::Connectivity,
            ))
        }
    }

    #[rstest]
    #[case::success(true)]
    #[case::failure(false)]
    fn book_detail_completion_retires_loading_and_caches_only_success(#[case] succeeds: bool) {
        let mut app = app_with_book_state();
        let state = &mut app.audiobookshelf_book_browse[0];
        state.books.push(book("book-a"));
        state.selected_id = Some("book-a".into());
        state.detail_loading_ids.insert("book-a".into(), 1);
        state.detail_loading = true;
        app.handle_lib_event(LibEvent::Audiobookshelf(
            AudiobookshelfEvent::BookDetailLoaded {
                generation: SetupGeneration::default(),
                request: 1,
                library_item_id: "book-a".into(),
                result: detail_result(succeeds),
            },
        ));

        let state = &app.audiobookshelf_book_browse[0];
        assert!(!state.detail_loading);
        assert!(!state.detail_loading_ids.contains_key("book-a"));
        assert_eq!(state.detail_cache.contains_key("book-a"), succeeds);
    }

    fn detail_completion_does_not_apply(
        app: &mut crate::app::App,
        library_item_id: &str,
        request: u64,
    ) {
        app.handle_lib_event(LibEvent::Audiobookshelf(
            AudiobookshelfEvent::BookDetailLoaded {
                generation: SetupGeneration::default(),
                request,
                library_item_id: library_item_id.into(),
                result: Ok((Vec::new(), Vec::new())),
            },
        ));
    }

    #[rstest]
    #[case::stale_generation("book-a", true, 0, false)]
    #[case::unowned("missing", false, 0, false)]
    #[case::superseded_request("book-a", false, 1, true)]
    fn book_detail_completion_ignores_stale_unowned_or_superseded_result(
        #[case] library_item_id: &str,
        #[case] stale_generation: bool,
        #[case] request: u64,
        #[case] superseded: bool,
    ) {
        let mut app = app_with_book_state();
        app.audiobookshelf_book_browse[0].books.push(book("book-a"));
        if superseded {
            // A newer request for the same book is in flight: the older
            // same-setup response must not retire it or write the cache.
            app.audiobookshelf_book_browse[0]
                .detail_loading_ids
                .insert("book-a".into(), 2);
        }
        if stale_generation {
            app.audiobookshelf_runtime.begin_setup();
        }

        detail_completion_does_not_apply(&mut app, library_item_id, request);

        let state = &app.audiobookshelf_book_browse[0];
        assert!(state.detail_cache.is_empty());
        if superseded {
            assert_eq!(state.detail_loading_ids.get("book-a"), Some(&2));
        }
    }
}
