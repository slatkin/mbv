//! The test-only reads and seeds a Client's test makes of an `App`'s state.
//! The gate spans the crate's own tests and a dependent Client's `test`
//! build, as `make_app_stub` does. The seed helpers compile only under the
//! `test` feature: the functions they use live in `app_stub` behind the
//! same gate.

#[cfg(any(test, feature = "test"))]
use super::App;

#[cfg(feature = "test")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "test")]
use super::{
    GUI_MUSIC_LIBRARY_ID, GUI_TV_LIBRARY_ID, album_item, group_item, music_album_level,
    music_group_level,
};

#[cfg(any(test, feature = "test"))]
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
        let _ = self
            .images
            .card_image_tx()
            .send((key.to_string(), mbv_images::ImageFetchOutcome::Image(img)));
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
