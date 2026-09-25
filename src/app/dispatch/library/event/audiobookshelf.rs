use crate::app::{App, LibEvent};

impl App {
    /// The `AudiobookshelfBooksFetched` body: append the fetched page to the
    /// library's book browse state, fetch the selected book's detail, and
    /// chain the next page when one is needed.
    fn handle_audiobookshelf_books_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            mbv_core::audiobookshelf::AudiobookshelfBookPage,
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    ) {
        if let Some(index) = self
            .audiobookshelf_libraries
            .iter()
            .position(|library| library.id == library_id)
        {
            let mut next_page = None;
            let mut selected_detail = None;
            if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
                match result {
                    Ok(page) => {
                        state.append_page_books(page.page, page.total, page.items);
                        next_page = state.needs_page();
                        if !state.detail_loading {
                            selected_detail = state.selected_id.clone();
                        }
                    }
                    Err(error) => state.error = Some(error.to_string()),
                }
            }
            if let Some(selected_detail) = selected_detail {
                self.start_audiobookshelf_book_detail(selected_detail);
            }
            if let Some(next_page) = next_page {
                crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
                    self.config.lock().unwrap().clone(),
                    generation,
                    library_id,
                    next_page,
                    self.lib_tx.clone(),
                );
            }
        }
    }

    /// The `AudiobookshelfBookDetailFetched` body: retire the in-flight mark
    /// on the owning browse state and cache the detail on success.
    fn handle_audiobookshelf_book_detail_fetched(
        &mut self,
        library_item_id: String,
        result: Result<
            (
                Vec<mbv_core::audiobookshelf::AudiobookshelfChapter>,
                Vec<mbv_core::audiobookshelf::AudiobookshelfAudioFile>,
            ),
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    ) {
        match result {
            Ok(detail) => {
                if let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
                    state
                        .books
                        .iter()
                        .any(|book| book.library_item_id == library_item_id)
                }) {
                    state.detail_loading_ids.remove(&library_item_id);
                    state.detail_loading = state
                        .selected_id
                        .as_ref()
                        .is_some_and(|id| state.detail_loading_ids.contains(id));
                    state.detail_cache.insert(library_item_id.clone(), detail);
                }
            }
            Err(_error) => {
                if let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
                    state
                        .books
                        .iter()
                        .any(|book| book.library_item_id == library_item_id)
                }) {
                    state.detail_loading_ids.remove(&library_item_id);
                    state.detail_loading = state
                        .selected_id
                        .as_ref()
                        .is_some_and(|id| state.detail_loading_ids.contains(id));
                }
            }
        }
    }

    fn handle_audiobookshelf_podcast_detail_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        request: u64,
        library_item_id: String,
        result: Result<
            Vec<mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode>,
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    ) -> Option<LibEvent> {
        let index = self.audiobookshelf_browse.iter().position(|state| {
            state
                .shows
                .iter()
                .any(|show| show.library_item_id == library_item_id)
        });
        let state = index.and_then(|index| self.audiobookshelf_browse.get_mut(index))?;
        // The response belongs to this state only when the show's in-flight
        // mark still carries its request serial: an orphaned response (its
        // mark cleared by a refresh) or a superseded one (a newer request for
        // the show was issued) is discarded whole — it must neither retire
        // the newer request's mark nor write the cache over a newer entry.
        if state.detail_loading_ids.get(&library_item_id) != Some(&request) {
            return None;
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
                    // A failed fetch consumed the show's once-per-session
                    // request: caching an empty result keeps the bounded
                    // fan-out from re-issuing it forever (design D5); the
                    // refresh key re-requests everything.
                    state.cache_detail(library_item_id, Vec::new());
                }
            }
        }
        // The fan-out continues its bounded batch: the next required show's
        // request starts as this one retires (design D5).
        if let Some(index) = index {
            self.start_audiobookshelf_podcast_fan_out(index);
        }
        None
    }

    fn handle_audiobookshelf_shows_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            mbv_core::audiobookshelf::AudiobookshelfShowPage,
            mbv_core::audiobookshelf::AudiobookshelfError,
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
                match result {
                    Ok(page) => {
                        state.append_page(page.page, page.limit, page.total, page.items);
                        next_page = state.needs_page();
                    }
                    Err(error) => state.error = Some(error.to_string()),
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
                    library_id,
                    next_page,
                    self.lib_tx.clone(),
                );
            }
        }
    }

    fn handle_audiobookshelf_shelf_fetched(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            Vec<mbv_core::audiobookshelf::AudiobookshelfShelf>,
            mbv_core::audiobookshelf::AudiobookshelfError,
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

    pub(super) fn handle_audiobookshelf_event(&mut self, ev: LibEvent) -> Option<LibEvent> {
        match ev {
            LibEvent::AudiobookshelfProgressAcknowledged(update) => {
                if self.audiobookshelf_runtime.accepts(update.generation) {
                    let position_ticks =
                        crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks(
                            update.current_time_seconds,
                        );
                    self.reconcile_audiobookshelf_progress(
                        &update.library_item_id,
                        &update.episode_id,
                        position_ticks,
                        update.current_time_seconds,
                        update.is_finished,
                    );
                }
                None
            }
            LibEvent::AudiobookshelfBookProgressAcknowledged(update) => {
                if self.audiobookshelf_runtime.accepts(update.generation) {
                    let position_ticks =
                        crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks(
                            update.current_time_seconds,
                        );
                    self.reconcile_audiobookshelf_book_progress(
                        &update.library_item_id,
                        position_ticks,
                        update.is_finished,
                    );
                }
                None
            }
            LibEvent::AudiobookshelfBooksFetched {
                generation,
                library_id,
                result,
            } => {
                if self.audiobookshelf_runtime.accepts(generation) {
                    self.handle_audiobookshelf_books_fetched(generation, library_id, result);
                }
                None
            }
            LibEvent::AudiobookshelfBookDetailFetched {
                generation,
                library_item_id,
                result,
            } => {
                if self.audiobookshelf_runtime.accepts(generation) {
                    self.handle_audiobookshelf_book_detail_fetched(library_item_id, result);
                }
                None
            }
            LibEvent::AudiobookshelfDetailFetched {
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
            LibEvent::AudiobookshelfShowsFetched {
                generation,
                library_id,
                result,
            } => {
                self.handle_audiobookshelf_shows_fetched(generation, library_id, result);
                None
            }
            LibEvent::AudiobookshelfShelfFetched {
                generation,
                library_id,
                result,
            } => {
                self.handle_audiobookshelf_shelf_fetched(generation, library_id, result);
                None
            }
            _ => Some(ev),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::state::types::audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use crate::app::state::types::events::LibEvent;
    use mbv_core::audiobookshelf::{
        AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookPage, AudiobookshelfChapter,
        AudiobookshelfError, AudiobookshelfFailureClass, AudiobookshelfLibrary,
    };
    use mbv_core::service_runtime::SetupGeneration;
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
        app.handle_audiobookshelf_event(LibEvent::AudiobookshelfBooksFetched {
            generation: SetupGeneration::default(),
            library_id: "books".into(),
            result: Ok(AudiobookshelfBookPage {
                page: 0,
                limit: 20,
                total: 21,
                items: vec![book("book-a")],
            }),
        });

        let state = &app.audiobookshelf_book_browse[0];
        assert_eq!(state.books.len(), 1);
        assert_eq!(state.selected_id.as_deref(), Some("book-a"));
        assert_eq!(state.total, 21);
        assert_eq!(state.next_page, 1);
        assert!(state.error.is_none());
    }

    fn books_fetched_does_not_apply(app: &mut crate::app::App, library_id: &str, stale: bool) {
        if stale {
            app.audiobookshelf_runtime.begin_setup();
        }
        app.handle_audiobookshelf_event(LibEvent::AudiobookshelfBooksFetched {
            generation: SetupGeneration::default(),
            library_id: library_id.into(),
            result: Ok(AudiobookshelfBookPage {
                page: 0,
                limit: 20,
                total: 1,
                items: vec![book("book-a")],
            }),
        });
    }

    #[rstest]
    #[case::unknown_library("missing", false)]
    #[case::stale_generation("books", true)]
    fn books_fetched_ignores_unknown_library_or_stale_generation(
        #[case] library_id: &str,
        #[case] stale_generation: bool,
    ) {
        let mut app = app_with_book_state();

        books_fetched_does_not_apply(&mut app, library_id, stale_generation);

        assert!(app.audiobookshelf_book_browse[0].books.is_empty());
    }

    #[test]
    fn books_fetch_failure_is_recorded_on_matching_browse_state() {
        let mut app = app_with_book_state();
        app.handle_audiobookshelf_event(LibEvent::AudiobookshelfBooksFetched {
            generation: SetupGeneration::default(),
            library_id: "books".into(),
            result: Err(AudiobookshelfError {
                class: AudiobookshelfFailureClass::Connectivity,
            }),
        });

        assert!(app.audiobookshelf_book_browse[0].error.is_some());
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
            Err(AudiobookshelfError {
                class: AudiobookshelfFailureClass::Connectivity,
            })
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
        state.detail_loading_ids.insert("book-a".into());
        state.detail_loading = true;
        app.handle_audiobookshelf_event(LibEvent::AudiobookshelfBookDetailFetched {
            generation: SetupGeneration::default(),
            library_item_id: "book-a".into(),
            result: detail_result(succeeds),
        });

        let state = &app.audiobookshelf_book_browse[0];
        assert!(!state.detail_loading);
        assert!(!state.detail_loading_ids.contains("book-a"));
        assert_eq!(state.detail_cache.contains_key("book-a"), succeeds);
    }

    fn detail_completion_does_not_apply(app: &mut crate::app::App, stale_generation: bool) {
        if stale_generation {
            app.audiobookshelf_runtime.begin_setup();
        }
        app.handle_audiobookshelf_event(LibEvent::AudiobookshelfBookDetailFetched {
            generation: SetupGeneration::default(),
            library_item_id: if stale_generation {
                "book-a"
            } else {
                "missing"
            }
            .into(),
            result: Ok((Vec::new(), Vec::new())),
        });
    }

    #[rstest]
    #[case::stale_generation(true)]
    #[case::unowned_book(false)]
    fn book_detail_completion_ignores_stale_or_unowned_result(#[case] stale_generation: bool) {
        let mut app = app_with_book_state();
        app.audiobookshelf_book_browse[0].books.push(book("book-a"));

        detail_completion_does_not_apply(&mut app, stale_generation);

        assert!(app.audiobookshelf_book_browse[0].detail_cache.is_empty());
    }

    #[test]
    fn books_page_does_not_restart_selected_detail_while_loading() {
        let mut app = app_with_book_state();
        let state = &mut app.audiobookshelf_book_browse[0];
        state.detail_loading = true;
        state.selected_id = Some("book-a".into());
        app.handle_audiobookshelf_event(LibEvent::AudiobookshelfBooksFetched {
            generation: SetupGeneration::default(),
            library_id: "books".into(),
            result: Ok(AudiobookshelfBookPage {
                page: 0,
                limit: 20,
                total: 1,
                items: vec![book("book-a")],
            }),
        });

        assert!(app.audiobookshelf_book_browse[0]
            .detail_loading_ids
            .is_empty());
        assert_eq!(
            app.audiobookshelf_book_browse[0].selected_id.as_deref(),
            Some("book-a")
        );
    }
}
