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
use super::{SeededAbsBook, SeededAbsEpisode};

impl App {
    /// Make the Audiobookshelf catalog still pending (add-gui-library-panel
    /// design D9): the startup is running, so the saved launch location's
    /// resolution must wait for the catalog it can still publish. A
    /// launch-location test seeds the waiting window this way, because the
    /// runtime's state is App-internal.
    #[cfg(feature = "test")]
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

    /// The book the GUI Library's Hero shows for the Audiobookshelf book
    /// library `lib_idx` (add-gui-library-panel row 9.1).
    #[must_use]
    pub fn gui_abs_picked_book_for_test(&self, lib_idx: usize) -> Option<&str> {
        self.gui_abs_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_book.as_deref())
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
}
