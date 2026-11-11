//! The test-only reads and seeds a Client's test makes of an `App`'s state.
//! The gate spans the crate's own tests and a dependent Client's `test`
//! build, as `make_app_stub` does. The seed helpers compile only under the
//! `test` feature: the functions they use live in `app_stub` behind the
//! same gate.

#[cfg(any(test, feature = "test"))]
use super::App;

/// The Feeds destination's test seeds and pushes (row 11.1) live in their own
/// child module so this file stays within its line ceiling.
mod gui_feeds;

/// The GUI Search data path's test seam (row 12.1) sits in its own child
/// module for the same reason.
mod search;

#[cfg(feature = "test")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "test")]
use super::{
    GUI_MUSIC_LIBRARY_ID, GUI_TV_LIBRARY_ID, album_item, group_item, music_album_level,
    music_group_level,
};

#[cfg(any(test, feature = "test"))]
/// One seeded episode row of the GUI's Podcasts helpers: `(episode id, title,
/// description, publish instant in unix seconds, duration seconds)`.
#[cfg(any(test, feature = "test"))]
pub(in crate::app) type SeededAbsEpisode<'a> =
    (&'a str, &'a str, Option<&'a str>, Option<u64>, Option<f64>);

/// One seeded book row of the GUI's Audiobooks catalog helper: `(id, title,
/// author display, author sort key, duration seconds, cover path)`.
#[cfg(any(test, feature = "test"))]
pub(in crate::app) type SeededAbsBook<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    &'a str,
    f64,
    Option<&'a str>,
);

impl App {
    /// The Library tabs the catalog holds, for a Client's test that asserts a
    /// startup landed the catalog (add-gui-library-panel design D11).
    #[must_use]
    pub fn libs_for_test(&self) -> &[mbv_ui_model::library_tab::LibraryTab] {
        &self.libs
    }

    /// Home's stored content snapshot, for a Client's test that asserts the
    /// frame the next tick carries reads the stored snapshot
    /// (add-gui-library-panel row 6.1).
    #[must_use]
    pub fn home_content_for_test(&self) -> &crate::app::state::playback::HomeContent {
        &self.home_content
    }

    /// Seed Home's content snapshot with `items` (add-gui-library-panel
    /// design D12): the Client tests build the Home frame over it without
    /// the bootstrap's mock transport, so the seeded snapshot is the one
    /// the projection reads.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_home_content_for_test(&mut self, items: Vec<mbv_emby_model::EmbyItem>) {
        self.home_content.continue_items = items;
        self.home_content.loading = false;
    }

    /// Make the grouped Music library the GUI Library test's destination
    /// (add-gui-library-panel row 3.1): the Emby runtime becomes Ready with
    /// `client` (built over a mock transport), and the Music tab's group level
    /// holds Hip-Hop and Soul with Hip-Hop's albums above it. A Client seeds it
    /// after its core thread is built, because the launch Home load rebuilds
    /// the catalog from the Service's views; Music is then the active
    /// destination, as the Library panel keeps it.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_library_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime =
            crate::app::state::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));
        self.seed_gui_music_library_tab_for_test();
    }

    /// The grouped Music library without a Service client (add-gui-library-panel
    /// design D9): a launch-location test seeds the catalog this way, so the
    /// launch Home load starts no Emby request and the seeded tabs survive it.
    /// The Music tab's group level holds Hip-Hop and Soul with Hip-Hop's albums
    /// above it, and Music is the active destination.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_library_tab_for_test(&mut self) {
        self.music_levels = vec!["group".to_string(), "album".to_string()];
        let mut library = mbv_emby_model::test_support::make_item("Music", "CollectionFolder");
        library.id = GUI_MUSIC_LIBRARY_ID.to_string();
        library.collection_type = "music".to_string();
        library.is_folder = true;
        let mut tab = mbv_ui_model::library_tab::LibraryTab::new(library);
        tab.nav_stack.push(music_group_level(vec![
            group_item("grp-hiphop", "Hip-Hop"),
            group_item("grp-soul", "Soul"),
        ]));
        tab.nav_stack.push(music_album_level(vec![
            album_item("alb-ill", "Licensed to Ill"),
            album_item("alb-paul", "Paul's Boutique"),
        ]));
        self.libs = vec![tab];
        self.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
        for album_id in ["alb-ill", "alb-paul"] {
            self.album_artist_cache
                .insert(album_id.to_string(), "Beastie Boys".to_string());
        }
    }

    /// Make the Shows library the GUI Library test's destination (design
    /// D12): the Emby runtime becomes Ready with `client` (built over a mock
    /// transport), and the Shows tab holds an empty root level — the state a
    /// first open finds before its mode's load lands — with no resolved TV
    /// mode, so the open's own default rule applies. Shows is then the
    /// active destination, as the Library panel keeps it.
    #[cfg(feature = "test")]
    pub fn seed_gui_tv_library_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime =
            crate::app::state::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));
        let mut library = mbv_emby_model::test_support::make_item("Shows", "CollectionFolder");
        library.id = GUI_TV_LIBRARY_ID.to_string();
        library.collection_type = "tvshows".to_string();
        let mut tab = mbv_ui_model::library_tab::LibraryTab::new(library);
        tab.nav_stack.push(mbv_ui_model::browse::BrowseLevel {
            parent_id: GUI_TV_LIBRARY_ID.to_string(),
            title: "Shows".to_string(),
            items: Vec::new(),
            rows: mbv_ui_model::browse::ServerRows::new(0),
            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: Some("Series".to_string()),
            unplayed_only: false,
            sort_by: "SortName".to_string(),
            sort_order: "Ascending".to_string(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        });
        self.libs = vec![tab];
        self.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
    }

    /// Pin `lib_idx`'s captured library total (add-gui-library-panel design
    /// D12): a Client's test that scripts one mode load pins the total, so
    /// the root load's completion captures nothing and spawns no
    /// default-mode reload behind the scripted answers.
    #[cfg(feature = "test")]
    pub fn pin_gui_library_total_for_test(&mut self, lib_idx: usize, total: usize) {
        if let Some(tab) = self.libs.get_mut(lib_idx) {
            tab.library_total = Some(total);
        }
    }

    /// The root level's item ids of the Shows library `lib_idx`, for a
    /// Client's test that asserts a mode load landed the rows
    /// (add-gui-library-panel design D12). `None` while the library holds
    /// no root level.
    #[must_use]
    pub fn tv_root_item_ids_for_test(&self, lib_idx: usize) -> Option<Vec<String>> {
        Some(
            self.libs
                .get(lib_idx)?
                .nav_stack
                .last()?
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect(),
        )
    }

    /// The active destination, for a Client's test that asserts an open-
    /// destination command landed (add-gui-library-panel design D2).
    #[must_use]
    pub fn tab_for_test(&self) -> mbv_ui_model::tab_selection::TabSelection {
        self.tab
    }

    /// Push `library` as another Emby library tab (add-gui-library-panel
    /// design D9): a launch-location test holds a catalog destination that
    /// the GUI Client does not present.
    #[cfg(feature = "test")]
    pub fn push_gui_library_tab_for_test(&mut self, library: mbv_emby_model::EmbyItem) {
        self.libs
            .push(mbv_ui_model::library_tab::LibraryTab::new(library));
    }

    /// Seed the GUI Music store with `albums` for the `group_id` of `lib_idx`,
    /// as a landed fetch would (add-gui-library-panel design D5): a projection
    /// or frame test reads the store alone, with no request behind it. Each
    /// album is a folder, as the live parse makes every `MusicAlbum`
    /// (`mbv_emby::parse_item`), so its item menu is the album menu.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_albums_for_test(
        &mut self,
        lib_idx: usize,
        group_id: &str,
        albums: &[(&str, &str)],
    ) {
        let albums = albums
            .iter()
            .map(|(id, name)| {
                let mut album = mbv_emby_model::test_support::make_item(name, "MusicAlbum");
                album.id = (*id).to_string();
                album.is_folder = true;
                album
            })
            .collect();
        self.gui_music_albums.insert(
            (lib_idx, group_id.to_string()),
            crate::app::gui_library::GuiGroupAlbums::Loaded(albums),
        );
    }

    /// Deliver `key`'s image as a completed fetch would (add-gui-library-panel
    /// design D13): the next tick's card-image drain lands it in the cache, so
    /// a frame test observes the art with no request and no thread.
    #[cfg(feature = "test")]
    pub fn complete_card_image_for_test(&self, key: &str, img: image::DynamicImage) {
        let source = crate::app::queue_card_artwork::test_source(&img);
        let _ = self.images.card_image_tx().send((
            key.to_string(),
            mbv_images::ImageFetchOutcome::Image { img, source },
        ));
    }

    /// The Grouped Music library's selected group ids in pill order
    /// (add-gui-library-panel design D5).
    #[must_use]
    pub fn gui_library_groups_for_test(&self, lib_idx: usize) -> Option<&str> {
        self.gui_library_groups.get(&lib_idx).map(String::as_str)
    }

    /// Seed the GUI Music selection with `group_ids` for `lib_idx`, as a
    /// landed set-collections command would (add-gui-library-panel): a
    /// command test pins the selection before its first step, so the launch
    /// restore's own application skips it and the test's mock answers serve
    /// the test's commands alone.
    #[cfg(feature = "test")]
    pub fn seed_gui_library_groups_for_test(&mut self, lib_idx: usize, group_id: &str) {
        self.gui_library_groups
            .insert(lib_idx, group_id.to_string());
    }

    /// The album ids the GUI Music store loaded for `group_id`; `None` while
    /// the group is loading or after its fetch failed (design D5).
    #[must_use]
    pub fn gui_group_album_ids_for_test(
        &self,
        lib_idx: usize,
        group_id: &str,
    ) -> Option<Vec<String>> {
        match self.gui_music_albums.get(&(lib_idx, group_id.to_string())) {
            Some(crate::app::gui_library::GuiGroupAlbums::Loaded(albums)) => {
                Some(albums.iter().map(|album| album.id.clone()).collect())
            }
            Some(
                crate::app::gui_library::GuiGroupAlbums::Loading
                | crate::app::gui_library::GuiGroupAlbums::Failed,
            )
            | None => None,
        }
    }

    /// The album a collection group's pill last showed (design D13).
    #[must_use]
    pub fn gui_library_pick_for_test(&self, lib_idx: usize, group_id: &str) -> Option<&str> {
        self.gui_library_picks
            .get(&(lib_idx, group_id.to_string()))
            .map(String::as_str)
    }

    /// The row the GUI Library's Hero shows for the plain Emby library
    /// `lib_idx` (add-gui-library-panel row 8.1).
    #[must_use]
    pub fn gui_emby_picked_row_for_test(&self, lib_idx: usize) -> Option<&str> {
        self.gui_emby_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_row.as_deref())
    }

    /// The root item ids the GUI's Emby root-list store loaded for `lib_idx`;
    /// `None` while the list is loading or after its fetch failed (row 8.1).
    #[must_use]
    pub fn gui_emby_root_ids_for_test(&self, lib_idx: usize) -> Option<Vec<String>> {
        match self.gui_emby_roots.get(&lib_idx) {
            Some(crate::app::gui_library::GuiEmbyRootItems::Loaded(items)) => {
                Some(items.iter().map(|item| item.id.clone()).collect())
            }
            Some(
                crate::app::gui_library::GuiEmbyRootItems::Loading
                | crate::app::gui_library::GuiEmbyRootItems::Failed,
            )
            | None => None,
        }
    }

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

    /// Seed the GUI's plain Emby root-list store with `items` for `lib_idx`,
    /// as a landed fetch would (add-gui-library-panel row 8.1): a Client's
    /// command test injects the rows and observes the frame with no fetch
    /// behind it.
    #[cfg(feature = "test")]
    pub fn seed_gui_emby_roots_for_test(
        &mut self,
        lib_idx: usize,
        items: Vec<mbv_emby_model::EmbyItem>,
    ) {
        self.gui_emby_roots.insert(
            lib_idx,
            crate::app::gui_library::GuiEmbyRootItems::Loaded(items),
        );
    }

    /// Make a feed-view home-videos library the GUI Library test's
    /// destination (add-gui-library-panel row 8.1): the Emby runtime becomes
    /// Ready with `client` (built over a mock transport), the feed-view
    /// config names the library, the root listing holds one folder whose
    /// counted videos admit its pill, and the feed state is present with the
    /// default All selection — the state a landed root listing and count
    /// fan-out leave. A Client seeds it after its core thread is built, so
    /// the launch Home load starts no request the test would script.
    ///
    /// # Panics
    /// Panics if the settings mutex is poisoned: a previous holder panicked.
    #[cfg(feature = "test")]
    pub fn seed_gui_feed_view_library_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime =
            crate::app::state::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));
        self.config
            .lock()
            .unwrap()
            .feed_view_libraries
            .push("youtube".to_string());
        let mut library = mbv_emby_model::test_support::make_item("YouTube", "CollectionFolder");
        library.id = "lib-youtube".to_string();
        library.collection_type = "homevideos".to_string();
        library.is_folder = true;
        let mut folder = mbv_emby_model::test_support::make_item("Channel 1", "Folder");
        folder.id = "folder-1".to_string();
        folder.is_folder = true;
        let mut tab = mbv_ui_model::library_tab::LibraryTab::new(library);
        tab.nav_stack.push(mbv_ui_model::browse::BrowseLevel {
            parent_id: "lib-youtube".to_string(),
            title: "YouTube".to_string(),
            items: vec![folder],
            rows: mbv_ui_model::browse::ServerRows::new(0),
            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".to_string(),
            sort_order: "Ascending".to_string(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        });
        tab.feed_home_video = Some(mbv_ui_model::feed::FeedHomeVideoState {
            folder_counts: [("folder-1".to_string(), 2_usize)].into_iter().collect(),
            // The default All selection arrives cached empty, so a Client's
            // set-feed-group test makes the folder fetch the one items fetch
            // the mock answers.
            items: [(mbv_ui_model::feed::FeedHomeVideoSelection::All, Vec::new())]
                .into_iter()
                .collect(),
            ..mbv_ui_model::feed::FeedHomeVideoState::default()
        });
        self.libs = vec![tab];
        self.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
    }

    /// Whether the album's track fetch is in flight (design D5): the open-
    /// album command's effect before its completion lands.
    #[must_use]
    pub fn album_tracks_loading_for_test(&self, album_id: &str) -> bool {
        self.album_tracks_loading.contains(album_id)
    }

    /// Seed the album track cache with `tracks` for `album_id`, as a landed
    /// album-track fetch would (add-gui-library-panel): the autoload add and
    /// play rules read the cache, so a command test injects the tracks and
    /// observes the Owner request with no fetch behind it.
    #[cfg(feature = "test")]
    pub fn seed_gui_album_tracks_for_test(
        &mut self,
        album_id: &str,
        tracks: &[(&str, &str, i64, i64)],
    ) {
        let tracks = tracks
            .iter()
            .map(|(id, name, number, seconds)| {
                let mut track = mbv_emby_model::test_support::make_item(name, "Audio");
                track.id = (*id).to_string();
                track.media_type = "Audio".to_string();
                track.index_number = *number;
                track.runtime_ticks = seconds * mbv_emby_model::TICKS_PER_SECOND;
                track
            })
            .collect();
        self.album_tracks_cache.insert(album_id.to_string(), tracks);
    }

    /// Set the autoload setting (add-gui-library-panel): the Library's add
    /// and play commands read it, and a Client's test has no settings UI.
    ///
    /// # Panics
    /// Panics if the settings mutex is poisoned: a previous holder panicked.
    #[cfg(feature = "test")]
    pub fn set_autoload_for_test(&mut self, autoload: bool) {
        self.config.lock().unwrap().autoload = autoload;
    }

    /// Whether the core holds no pending TUI overlay (add-gui-library-panel
    /// row 3.6): the GUI runs an item-menu entry without arming one.
    #[must_use]
    pub fn pending_overlay_is_none_for_test(&self) -> bool {
        self.pending_overlay.is_none()
    }

    /// Queue a Home `Recently added` fetch result for the Client tests' next
    /// tick (add-gui-library-panel row 6.2): the event rides the lib channel
    /// exactly as the background fetch sends it, so the tick's drain that
    /// accepts it is the production path with no worker thread. The source
    /// is the Audiobookshelf podcast library `pod-lib`, at this App's
    /// Audiobookshelf runtime generation — the hermetic arm, whose rows
    /// carry their provider timestamp in seconds and so need no date-string
    /// parse at the 14-day cutoff.
    #[cfg(feature = "test")]
    pub fn queue_home_recently_added_for_test(&mut self, items: Vec<mbv_queue::QueueItem>) {
        use crate::app::{LibEvent, ModelContentEvent};

        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
            ModelContentEvent::HomeRecentlyAddedFetched {
                source: crate::app::state::playback::DestinationLatestSource::Audiobookshelf(
                    "pod-lib".to_string(),
                ),
                generation: self.audiobookshelf_runtime.generation(),
                items,
            },
        ));
    }

    /// Queue a Home content refresh for the Client tests' next tick
    /// (add-gui-library-panel row 6.2): the event rides the lib channel
    /// exactly as the App-internal Continue Watching writers send it, so
    /// the tick's drain that applies it is the production path with no
    /// worker thread. The snapshot arrives settled (`loading` false), as
    /// every computed refresh carries.
    #[cfg(feature = "test")]
    pub fn queue_home_content_refreshed_for_test(&mut self, items: Vec<mbv_emby_model::EmbyItem>) {
        use crate::app::{LibEvent, ModelContentEvent};

        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
            ModelContentEvent::HomeContentRefreshed(Box::new(
                crate::app::state::playback::HomeContent {
                    continue_items: items,
                    loading: false,
                },
            )),
        ));
    }

    /// Queue a Home content clear for the Client tests' next tick
    /// (add-gui-library-panel row 6.2): the Service teardown event, on the
    /// lib channel, exactly as the teardown sends it.
    #[cfg(feature = "test")]
    pub fn queue_home_content_cleared_for_test(&mut self) {
        use crate::app::{LibEvent, ModelContentEvent};

        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
            ModelContentEvent::HomeContentCleared,
        ));
    }

    /// Block until one library event lands on the lib channel and apply it
    /// through the same App arm the tick's drain runs (add-gui-library-panel
    /// design D13): a Client test that pumps GUI ticks over a mocked fetch
    /// waits here for the completion instead of sleeping between steps, so a
    /// passing pump spends no real time and the fetch workers never starve
    /// behind a spinning test. Returns whether an event landed within
    /// `bound` — a safety net that fails a broken pipeline instead of hanging
    /// the test, never a cadence a passing pump spends.
    #[cfg(feature = "test")]
    #[must_use]
    pub fn drain_lib_event_blocking_for_test(&mut self, bound: std::time::Duration) -> bool {
        let Ok(event) = self.channels.lib_rx.recv_timeout(bound) else {
            return false;
        };
        self.handle_lib_event(event);
        true
    }

    /// The ids of the Home `Recently added` rows the App stores, over the
    /// sources in store order (add-gui-library-panel row 6.2): a Client's
    /// test reads them without the crate-internal source type. Each id is
    /// the queue row's own stable identifier.
    #[must_use]
    pub fn home_recently_added_ids_for_test(&self) -> Vec<String> {
        self.home_recently_added
            .values()
            .flatten()
            .map(|item| item.id().to_string())
            .collect()
    }

    /// Whether the `/Sessions` load a playback-target discovery starts is in
    /// flight.
    #[must_use]
    pub fn sessions_load_started_for_test(&self) -> bool {
        self.sessions_loading
    }

    /// How many cast discovery browses this `App` started.
    #[must_use]
    pub fn cast_discovery_starts_for_test(&self) -> usize {
        self.cast_discovery_starts
    }
}
