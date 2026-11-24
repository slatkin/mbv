//! The Audiobookshelf destinations' test seeds and their Owner admission
//! (add-gui-library-panel rows 9.1 and 10.1): the catalog the two libraries
//! present, the book and episode rows with their cached detail and progress,
//! and the admission the play and queue commands gate on. A Client's test
//! injects each one, so no fetch and no worker run behind its assertions.
//!
//! The group is cohesive — the one Service whose catalog, detail, and
//! progress all arrive on the App's own browse states — so it sits in its own
//! child module of `test_reads` and keeps that file within its line ceiling.

use super::super::App;
use super::super::{SeededAbsBook, SeededAbsEpisode};

impl App {
    /// Make the Audiobookshelf catalog still pending (add-gui-library-panel
    /// design D9): the startup is running, so the saved launch location's
    /// resolution must wait for the catalog it can still publish. A
    /// launch-location test seeds the waiting window this way, because the
    /// runtime's state is App-internal.
    #[cfg(any(test, feature = "test"))]
    pub fn mark_gui_abs_catalog_pending_for_test(&mut self) {
        self.audiobookshelf_runtime =
            crate::app::state::service_runtime::AudiobookshelfRuntime::new(true);
    }

    /// Seed one Audiobookshelf book library and one podcast library with
    /// their parallel browse states (add-gui-library-panel row 9.1): the
    /// catalog a landed Audiobookshelf startup leaves, with no books yet and
    /// the runtime settled out of its startup, so a saved location's restore
    /// resolves against it. A Client's test seeds it after its core thread is
    /// built.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_abs_libraries_for_test(&mut self) {
        use mbv_ui_model::audiobookshelf_browse::{
            AudiobookshelfBookBrowseState, AudiobookshelfBrowseState,
        };
        let book = mbv_audiobookshelf::AudiobookshelfLibrary {
            id: "lib-abs-books".into(),
            name: "Audiobooks".into(),
            media_type: "book".into(),
        };
        let podcast = mbv_audiobookshelf::AudiobookshelfLibrary {
            id: "lib-abs-podcasts".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        };
        self.audiobookshelf_libraries = vec![book, podcast];
        self.audiobookshelf_browse = self
            .audiobookshelf_libraries
            .iter()
            .cloned()
            .map(AudiobookshelfBrowseState::new)
            .collect();
        self.audiobookshelf_book_browse = self
            .audiobookshelf_libraries
            .iter()
            .cloned()
            .map(AudiobookshelfBookBrowseState::new)
            .collect();
    }

    /// Name `server_url` as the configured Audiobookshelf server, as a saved
    /// setup leaves it: the row-art keys and the row sources read it.
    ///
    /// # Panics
    /// Panics if the settings mutex is poisoned: a previous holder panicked.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_server_url_for_test(&mut self, server_url: &str) {
        self.config.lock().unwrap().audiobookshelf_setup = Some(mbv_config::AudiobookshelfSetup {
            server_url: server_url.to_owned(),
            revision: 1,
        });
    }

    /// Seed the GUI's Audiobooks book catalog for `lib_idx` with `books`, as
    /// a landed fetch would — already in the TUI's author-sort order
    /// (add-gui-library-panel row 9.1): `(id, title, author credit, author
    /// sort key, duration seconds, cover path)`. A Client's command test
    /// injects the rows and observes the frame with no fetch behind it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_abs_books_for_test(&mut self, lib_idx: usize, books: &[SeededAbsBook<'_>]) {
        let Some(state) = self.audiobookshelf_book_browse.get_mut(lib_idx) else {
            return;
        };
        for (id, title, author_display, author_sort_key, duration, cover) in books {
            state.books.push(mbv_audiobookshelf::AudiobookshelfBook {
                library_item_id: (*id).into(),
                updated_at_ms: 0,
                added_at_ms: None,
                title: (*title).into(),
                author_display: author_display.map(str::to_string),
                author_sort_key: (*author_sort_key).into(),
                cover_path: cover.map(str::to_string),
                duration_seconds: *duration,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            });
        }
        state.total = state.books.len();
    }

    /// Seed the selected book's chapter detail for the GUI's Audiobooks
    /// projection, as a landed detail fetch would (add-gui-library-panel row
    /// 9.1): `(chapter id, start seconds, end seconds, title)` chapters and
    /// `(index, duration seconds)` audio files.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_abs_book_detail_for_test(
        &mut self,
        lib_idx: usize,
        book_id: &str,
        chapters: &[(usize, f64, f64, &str)],
        files: &[(usize, f64)],
    ) {
        let Some(state) = self.audiobookshelf_book_browse.get_mut(lib_idx) else {
            return;
        };
        state.detail_cache.insert(
            book_id.to_string(),
            (
                chapters
                    .iter()
                    .map(
                        |(id, start, end, title)| mbv_audiobookshelf::AudiobookshelfChapter {
                            id: *id,
                            start: *start,
                            end: *end,
                            title: (*title).into(),
                        },
                    )
                    .collect(),
                files
                    .iter()
                    .map(
                        |(index, duration)| mbv_audiobookshelf::AudiobookshelfAudioFile {
                            index: *index,
                            ino: String::new(),
                            duration: *duration,
                        },
                    )
                    .collect(),
            ),
        );
    }

    /// Seed the cached book progress the Hero's `Finished`/`N%` row and the
    /// play path's saved position read (add-gui-library-panel row 9.1).
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_abs_book_progress_for_test(
        &mut self,
        lib_idx: usize,
        book_id: &str,
        current_time_seconds: f64,
        is_finished: bool,
    ) {
        if let Some(state) = self.audiobookshelf_book_browse.get_mut(lib_idx) {
            state.progress.insert(
                book_id.to_string(),
                mbv_audiobookshelf::AudiobookshelfBookProgress {
                    library_item_id: book_id.to_string(),
                    current_time_seconds,
                    is_finished,
                },
            );
        }
    }

    /// Seed the GUI's Podcasts show list for `lib_idx` with `shows`, as a
    /// landed show page would — the list's title order is the projection's
    /// pill order (add-gui-library-panel row 10.1): `(show id, title)` pairs.
    /// A Client's command test injects the catalog and observes the frame
    /// with no fetch behind it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_podcast_shows_for_test(&mut self, lib_idx: usize, shows: &[(&str, &str)]) {
        let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx) else {
            return;
        };
        for (id, title) in shows {
            state.shows.push(mbv_audiobookshelf::AudiobookshelfShow {
                library_item_id: (*id).into(),
                updated_at_ms: 0,
                title: (*title).into(),
                author: None,
                description: None,
                cover_path: None,
            });
        }
        state.total = state.shows.len();
    }

    /// Seed one show's episode cache for the GUI's Podcasts projection, as a
    /// landed per-show fetch would (add-gui-library-panel row 10.1):
    /// `(episode id, title, description, publish instant in unix seconds,
    /// duration seconds)` rows, with the show's own cover path when `cover` is
    /// some so the Hero's cover chain resolves.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_podcast_episodes_for_test(
        &mut self,
        lib_idx: usize,
        show_id: &str,
        cover: Option<&str>,
        episodes: &[SeededAbsEpisode<'_>],
    ) {
        let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx) else {
            return;
        };
        if let Some(cover) = cover
            && let Some(show) = state
                .shows
                .iter_mut()
                .find(|show| show.library_item_id == show_id)
        {
            show.cover_path = Some(cover.to_string());
        }
        state.cache_detail(
            show_id.to_string(),
            episodes
                .iter()
                .map(
                    |(episode_id, title, description, published_at, duration_seconds)| {
                        mbv_audiobookshelf::AudiobookshelfDownloadedEpisode {
                            library_item_id: show_id.to_string(),
                            episode_id: (*episode_id).into(),
                            title: (*title).into(),
                            description: description.map(str::to_string),
                            published_at: *published_at,
                            duration_seconds: *duration_seconds,
                        }
                    },
                )
                .collect(),
        );
    }

    /// Seed one episode's progress for the GUI's Podcasts row state and the
    /// Hero's resume facts (add-gui-library-panel row 10.1).
    #[cfg(any(test, feature = "test"))]
    pub fn seed_gui_podcast_progress_for_test(
        &mut self,
        lib_idx: usize,
        show_id: &str,
        episode_id: &str,
        current_time_seconds: f64,
        is_finished: bool,
    ) {
        if let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx) {
            state.progress.insert(
                (show_id.to_string(), episode_id.to_string()),
                mbv_audiobookshelf::AudiobookshelfProgress {
                    library_item_id: show_id.to_string(),
                    episode_id: episode_id.to_string(),
                    current_time_seconds,
                    is_finished,
                },
            );
        }
    }

    /// Set the Owner's Audiobookshelf queue admission (add-gui-library-panel
    /// row 9.1): the play and enqueue commands gate on it, and a Client's
    /// test has no live Owner. The stub owner's ctrl compatibility carries
    /// the gate, so the player proxy is rebuilt over a stub remote with the
    /// requested capability.
    #[cfg(any(test, feature = "test"))]
    pub fn set_abs_admission_for_test(&mut self, admitted: bool) {
        let (mut remote, player_rx, _cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
        compatibility.audiobookshelf.queue = admitted;
        compatibility.audiobookshelf.book_queue = admitted;
        remote.set_ctrl_compatibility_for_test(compatibility);
        let always_play_next = self.player.always_play_next;
        self.player = mbv_player::PlayerProxy::from_remote(remote, always_play_next);
        self.player_rx = player_rx;
    }

    /// Hold a setup receiver whose worker exited without answering, as a
    /// crashed validation worker leaves the attempt.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_setup_worker_gone_for_test(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        self.setup.audiobookshelf_setup_rx = Some(rx);
    }

    /// Whether the Audiobookshelf setup attempt still holds a result receiver:
    /// whether the drain put a live worker's receiver back.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn audiobookshelf_setup_receiver_present_for_test(&self) -> bool {
        self.setup.audiobookshelf_setup_rx.is_some()
    }

    /// Start one more Audiobookshelf validation round, as a retry does, so the
    /// generation the runtime now accepts differs from the one it held.
    #[cfg(any(test, feature = "test"))]
    pub fn begin_audiobookshelf_validation_for_test(&mut self) {
        self.audiobookshelf_runtime.begin_validation();
    }

    /// Hold the catalog worker's answered `result` as the catalog load's
    /// receiver, as a completed catalog fetch leaves it.
    ///
    /// # Panics
    ///
    /// Panics when the freshly built catalog channel refuses its one
    /// completion, which needs the receiver dropped.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_catalog_result_for_test(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        result: crate::app::dispatch::session::service_startup::AudiobookshelfCatalogOutcome,
    ) {
        use crate::app::dispatch::session::service_startup::{
            AudiobookshelfCatalogCompletion, AudiobookshelfCatalogReceiver,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(AudiobookshelfCatalogCompletion { generation, result })
            .expect("the catalog channel accepts its one completion");
        self.setup.audiobookshelf_catalog_rx =
            Some(AudiobookshelfCatalogReceiver { generation, rx });
    }

    /// Hold a catalog receiver at `generation` whose worker exited without
    /// answering, as a crashed catalog worker leaves the load.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_catalog_worker_gone_for_test(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        use crate::app::dispatch::session::service_startup::AudiobookshelfCatalogReceiver;
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        self.setup.audiobookshelf_catalog_rx =
            Some(AudiobookshelfCatalogReceiver { generation, rx });
    }

    /// Apply the validation worker's `result` for the current setup generation,
    /// as the worker's answer drains into the runtime.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_audiobookshelf_result_for_test(
        &mut self,
        result: Result<
            mbv_audiobookshelf::AudiobookshelfUser,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    ) {
        use crate::app::dispatch::session::service_startup::{
            AudiobookshelfCompletion, AudiobookshelfCompletionKind,
        };
        let generation = self.audiobookshelf_runtime.generation();
        self.apply_audiobookshelf_completion(AudiobookshelfCompletion {
            generation,
            kind: AudiobookshelfCompletionKind::Startup,
            result,
        });
    }

    /// Report the validation worker's exit at `generation`, as the drain does
    /// when its channel closes.
    #[cfg(any(test, feature = "test"))]
    pub fn report_audiobookshelf_worker_disconnect_for_test(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        self.handle_audiobookshelf_worker_disconnect(generation);
    }

    /// Drain the Audiobookshelf setup hand-offs once, answering whether a
    /// completion or a disconnect was applied.
    #[cfg(any(test, feature = "test"))]
    pub fn drain_audiobookshelf_events_for_test(&mut self) -> bool {
        self.drain_audiobookshelf_events()
    }
}

impl App {
    /// Rewrite the Podcasts library's browse state at `index` through `edit`,
    /// as a landed page, a committed scope, or an armed replacement batch
    /// leaves it.
    ///
    /// # Panics
    ///
    /// Panics when `index` names no browse state.
    #[cfg(any(test, feature = "test"))]
    pub fn edit_audiobookshelf_browse_state_for_test<R>(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState) -> R,
    ) -> R {
        let state = self
            .audiobookshelf_browse
            .get_mut(index)
            .expect("the browse state index is in range");
        edit(state)
    }

    /// Rewrite the Audiobooks library's book browse state at `index` through
    /// `edit`, as a landed catalog page or an armed replacement batch leaves
    /// it.
    ///
    /// # Panics
    ///
    /// Panics when `index` names no browse state.
    #[cfg(any(test, feature = "test"))]
    pub fn edit_audiobookshelf_book_state_for_test<R>(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState) -> R,
    ) -> R {
        let state = self
            .audiobookshelf_book_browse
            .get_mut(index)
            .expect("the book browse state index is in range");
        edit(state)
    }

    /// Append `library` as a Podcasts library, with its own browse state, as a
    /// landed catalog that grew one library leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn push_audiobookshelf_podcast_library_for_test(
        &mut self,
        library: mbv_audiobookshelf::AudiobookshelfLibrary,
    ) {
        self.audiobookshelf_libraries.push(library.clone());
        self.audiobookshelf_browse
            .push(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState::new(library));
    }

    /// Append `library` as an Audiobooks library, with its own book browse
    /// state, as a landed catalog that grew one library leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn push_audiobookshelf_book_library_for_test(
        &mut self,
        library: mbv_audiobookshelf::AudiobookshelfLibrary,
    ) {
        self.audiobookshelf_libraries.push(library.clone());
        self.audiobookshelf_book_browse
            .push(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState::new(library));
    }

    /// Whether the library `library_id` has an unabandoned Latest shelf
    /// request in flight.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn audiobookshelf_shelf_requested_for_test(&self, library_id: &str) -> bool {
        self.audiobookshelf_shelf_requested.contains(library_id)
    }
}
