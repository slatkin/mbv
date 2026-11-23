//! The TUI shell's Home and Feeds test seeds (`split-tui-gui-clients` design
//! D4b rule 5): the snapshot, subscription, and entry fixtures the shell's
//! Home, Feeds, and status-row tests assert a projection of.
//!
//! Each seam names the fixture it builds — a Home snapshot, a Feeds tab with
//! its subscriptions and entries, a Service state — never one field, so the
//! tests that drove those shapes keep their claims and reach no field.

use super::super::App;
use mbv_emby_model::EmbyItem;
use mbv_queue::FeedEntry;
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::tab_selection::TabSelection;

/// One seeded track of an album's landed track list: `(id, title, index
/// number, duration seconds)`.
#[cfg(any(test, feature = "test"))]
pub(in crate::app) type SeededAlbumTrack<'a> = (&'a str, &'a str, i64, i64);

impl App {
    /// Hold `items` as Home's Continue Watching snapshot, as a launch load
    /// would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_home_continue_items_for_test(&mut self, items: Vec<EmbyItem>) {
        self.home_content.continue_items = items;
    }

    /// Hold the Feeds tab the given `subscriptions` configure and the given
    /// `entries` it last fetched, as a startup load would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_feeds_tab_for_test(
        &mut self,
        subscriptions: Vec<mbv_config::FeedSubscription>,
        entries: Vec<FeedEntry>,
    ) {
        self.feed_tab.subscriptions = subscriptions;
        self.feed_tab.all_entries = entries;
    }

    /// Make `tab` the active left-panel tab without the settling pass a user's
    /// tab change runs: the state a Client holds once it has settled there.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_active_tab_for_test(&mut self, tab: TabSelection) {
        self.tab = tab;
    }

    /// Hold `state` as the Audiobookshelf Service's state, as a setup attempt
    /// leaves it.    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_service_state_for_test(
        &mut self,
        state: mbv_core::service_runtime::ServiceState,
    ) {
        self.audiobookshelf_runtime.state = state;
    }

    /// Hold a ready Emby runtime: a configured, unroutable client whose state
    /// is `Ready`, so the launch-time fetch paths arm with no live Service.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_ready_emby_runtime_for_test(&mut self) {
        self.emby_runtime = super::artists::ready_emby_runtime_for_test();
    }

    /// Hold `config` as this Client's configuration and `client` as its ready
    /// Emby runtime, as a launch that already holds an Emby setup would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_configured_emby_client_for_test(
        &mut self,
        config: crate::app::config::Config,
        client: mbv_emby::EmbyClient,
    ) {
        self.config = std::sync::Arc::new(std::sync::Mutex::new(config));
        self.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
    }

    /// Hold `entry` as the playlist the Playlists overlay has open, with the
    /// rows it holds, as a settled open would leave them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_open_playlist_for_test(&mut self, entry: EmbyItem, rows: Vec<EmbyItem>) {
        self.playlists_open = Some(entry);
        self.playlists_open_items = rows;
    }

    /// Drop every live toast, so a test observes only the toast its own
    /// action raised.
    #[cfg(any(test, feature = "test"))]
    pub fn clear_toasts_for_test(&mut self) {
        self.toasts.clear();
    }

    /// Hold the owner's reported playback output, as a drain would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playback_output_for_test(&mut self, output: Option<mbv_ctrl::OutputState>) {
        self.playback_output = output;
    }

    /// Take every Emby library tab out of the catalog, leaving the destination
    /// empty: a test that builds its own catalog takes the fixture's out first.
    #[cfg(any(test, feature = "test"))]
    pub fn take_libraries_for_test(&mut self) -> Vec<LibraryTab> {
        std::mem::take(&mut self.libs)
    }

    /// Hold `library` as the catalog's next destination, as a landed view load
    /// leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn push_library_for_test(&mut self, library: LibraryTab) {
        self.libs.push(library);
    }

    /// Rewrite the Emby library at `index` through `edit`, as a landed fetch or
    /// a user's browse change would.
    #[cfg(any(test, feature = "test"))]
    pub fn edit_library_for_test(&mut self, index: usize, edit: impl FnOnce(&mut LibraryTab)) {
        if let Some(tab) = self.libs.get_mut(index) {
            edit(tab);
        }
    }

    /// Set the collection type the Emby library at `index` serves, as a landed
    /// catalog names it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_library_collection_type_for_test(&mut self, index: usize, collection_type: &str) {
        if let Some(tab) = self.libs.get_mut(index) {
            tab.library.collection_type = collection_type.to_string();
        }
    }

    /// Set the captured library total of the Emby library at `index`, as a
    /// landed root listing reports it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_library_total_for_test(&mut self, index: usize, total: usize) {
        if let Some(tab) = self.libs.get_mut(index) {
            tab.library_total = Some(total);
        }
    }

    /// Set the item type of `level`'s row `row` in the Emby library at
    /// `index`, as a landed level names it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_library_row_item_type_for_test(
        &mut self,
        index: usize,
        level: usize,
        row: usize,
        item_type: &str,
    ) {
        if let Some(item) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.get_mut(level))
            .and_then(|open| open.items.get_mut(row))
        {
            item.item_type = item_type.to_string();
        }
    }

    /// Push `item` as the last row of `level`'s browse level in the Emby
    /// library at `index`, as a landed row append would.
    #[cfg(any(test, feature = "test"))]
    pub fn push_library_row_for_test(
        &mut self,
        index: usize,
        level: usize,
        item: mbv_emby_model::EmbyItem,
    ) {
        if let Some(open) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.get_mut(level))
        {
            open.items.push(item);
        }
    }

    /// Push `level` as the next browse level of the Emby library at `index`,
    /// as a drill into an album's track list does.
    #[cfg(any(test, feature = "test"))]
    pub fn push_library_level_for_test(
        &mut self,
        index: usize,
        level: mbv_ui_model::browse::BrowseLevel,
    ) {
        if let Some(tab) = self.libs.get_mut(index) {
            tab.nav_stack.push(level);
        }
    }

    /// Rewrite `level`'s browse level in the Emby library at `index` through
    /// `edit`, as a landed fetch or a browse change would.
    #[cfg(any(test, feature = "test"))]
    pub fn edit_library_level_for_test(
        &mut self,
        index: usize,
        level: usize,
        edit: impl FnOnce(&mut mbv_ui_model::browse::BrowseLevel),
    ) {
        if let Some(open) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.get_mut(level))
        {
            edit(open);
        }
    }

    /// Move the resting cursor of `level`'s browse level in the Emby library
    /// at `index`, as the shell's re-anchors read it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_library_level_cursor_for_test(&mut self, index: usize, level: usize, cursor: usize) {
        if let Some(open) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.get_mut(level))
        {
            open.set_resting_cursor(cursor);
        }
    }

    /// Set the letter filter `level`'s browse level in the Emby library at
    /// `index` carries, as a landed bucket listing leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_library_level_letter_filter_for_test(
        &mut self,
        index: usize,
        level: usize,
        filter: mbv_ui_model::sort_filter::LetterFilter,
    ) {
        if let Some(open) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.get_mut(level))
        {
            open.letter_filter = Some(filter);
        }
    }

    /// Hold `mode` as the TV content mode of the Emby library at `index`'s open
    /// level, as a content selection leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_library_tv_content_mode_for_test(
        &mut self,
        index: usize,
        mode: mbv_queue::TvContentMode,
    ) {
        if let Some(open) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.last_mut())
        {
            open.tv_content_mode = Some(mode);
        }
    }

    /// Take the Emby library's open browse levels at `index`, leaving its stack
    /// empty, as the shell's recursive-album landing does.
    #[cfg(any(test, feature = "test"))]
    pub fn take_library_levels_for_test(
        &mut self,
        index: usize,
    ) -> Vec<mbv_ui_model::browse::BrowseLevel> {
        self.libs
            .get_mut(index)
            .map_or_else(Vec::new, |tab| std::mem::take(&mut tab.nav_stack))
    }

    /// Note that the Music library was configured with the grouped levels
    /// (`group`, then `album`).
    #[cfg(any(test, feature = "test"))]
    pub fn seed_grouped_music_levels_for_test(&mut self) {
        self.music_levels = vec!["group".to_string(), "album".to_string()];
    }

    /// Hold `tracks` as the landed album-track fetch for each named album, each
    /// [`SeededAlbumTrack`].
    #[cfg(any(test, feature = "test"))]
    pub fn seed_album_tracks_for_test(&mut self, albums: &[(&str, &[SeededAlbumTrack<'_>])]) {
        for (album_id, tracks) in albums {
            let rows = tracks
                .iter()
                .map(|(id, title, number, seconds)| {
                    let mut track = mbv_emby_model::test_support::make_item(title, "Audio");
                    track.id = (*id).to_string();
                    track.media_type = "Audio".to_string();
                    track.index_number = *number;
                    track.runtime_ticks = seconds * mbv_emby_model::TICKS_PER_SECOND;
                    track
                })
                .collect();
            self.album_tracks_cache
                .insert((*album_id).to_string(), rows);
        }
    }

    /// Hold `states` as the Audiobookshelf screen states behind the
    /// destination, one per stored library.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_browse_for_test(
        &mut self,
        states: Vec<mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState>,
    ) {
        self.audiobookshelf_browse = states;
    }

    /// Hold `states` as the Audiobookshelf book states behind the destination,
    /// one per stored library.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_book_browse_for_test(
        &mut self,
        states: Vec<mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState>,
    ) {
        self.audiobookshelf_book_browse = states;
    }

    /// Hold `libraries` as the Audiobookshelf catalog the destination lists.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_libraries_for_test(
        &mut self,
        libraries: Vec<mbv_audiobookshelf::AudiobookshelfLibrary>,
    ) {
        self.audiobookshelf_libraries = libraries;
    }

    /// Hold `request` as the overlay the core raised and has not handed to its
    /// Client yet.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_pending_overlay_for_test(
        &mut self,
        request: mbv_ui_model::overlay::OverlayRequest,
    ) {
        self.pending_overlay = Some(request);
    }

    /// Move the local queue view's cursor, as a list surface's selection does.
    #[cfg(any(test, feature = "test"))]
    pub fn move_local_queue_cursor_for_test(&mut self, cursor: usize) {
        self.local_view.set_cursor(cursor);
    }

    /// The local queue view's cursor.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn local_queue_cursor_for_test(&self) -> usize {
        self.local_view.cursor()
    }

    /// The local queue view's total number of queue slots.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn local_queue_len_for_test(&self) -> usize {
        self.local_view.total_queue_len()
    }

    /// Rewrite this Client's configuration through `read`, as a Settings
    /// screen's field change does.
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is poisoned: a writer panicked
    /// holding it.
    #[cfg(any(test, feature = "test"))]
    pub fn config_for_test<R>(
        &mut self,
        read: impl FnOnce(&mut crate::app::config::Config) -> R,
    ) -> R {
        let mut config = self
            .config
            .lock()
            .unwrap_or_else(|poisoned| panic!("the configuration lock was poisoned: {poisoned:?}"));
        read(&mut config)
    }

    /// The player's subtitle and audio preference snapshot, as a Settings
    /// commit pushes it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn player_prefs_snapshot_for_test(&self) -> mbv_ctrl::player::SubtitlePrefs {
        self.player.subtitle_prefs_snapshot()
    }

    /// Hold `items` as the playback queue's slots, cursor at `cursor`, as an
    /// Owner snapshot would: the queue the playback projections read, which
    /// the local view mirrors.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playback_queue_for_test(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
    ) {
        let state = mbv_ctrl::UnifiedQueueStateData {
            queue: None,
            start_slot: None,
            status: mbv_ctrl::player::PlayerStatus::default(),
            active_slot: None,
            slots: items
                .into_iter()
                .enumerate()
                .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                    slot_id: u64::try_from(index).unwrap_or(u64::MAX) + 1,
                    item,
                })
                .collect(),
            revision: 1,
            source: mbv_queue::QueueSource::Unknown,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        };
        self.playback_queue_mut().adopt(
            &state,
            crate::app::state::queue_view::AdoptCause::Replacement,
        );
        self.seed_local_queue_snapshot_for_test(&state);
        self.local_view.set_cursor(cursor);
    }

    /// Hold `detail` as the landed series detail for `series_id`.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_series_detail_for_test(
        &mut self,
        series_id: &str,
        detail: mbv_ui_model::browse::SeriesDetail,
    ) {
        self.series_detail_cache
            .insert(series_id.to_string(), detail);
    }

    /// Clear the in-flight series detail fetches.
    #[cfg(any(test, feature = "test"))]
    pub fn clear_series_detail_loading_for_test(&mut self) {
        self.series_detail_loading.clear();
    }

    /// Note that `library_id`'s last TV Upcoming fetch finally failed.
    #[cfg(any(test, feature = "test"))]
    pub fn mark_tv_upcoming_fetch_failed_for_test(&mut self, library_id: &str) {
        self.tv_upcoming_fetches.insert(
            library_id.to_string(),
            crate::app::state::app_struct::TvUpcomingFetchState::Failed,
        );
    }

    /// Hold `client` as the ready Emby runtime, as a launch that already holds
    /// an Emby setup leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_ready_emby_client_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
    }

    /// Hold the unconfigured Emby runtime, so no background refresh runs over
    /// a test's own transport: what a launch before a setup leaves.
    #[cfg(any(test, feature = "test"))]
    pub fn clear_emby_runtime_for_test(&mut self) {
        self.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::default();
    }

    /// Whether the open level of the Emby library at `index` is loading, as a
    /// landed fetch clears it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn library_level_loading_for_test(&self, index: usize) -> bool {
        self.libs
            .get(index)
            .and_then(|tab| tab.nav_stack.first())
            .is_some_and(|open| open.loading)
    }

    /// Whether the Settings toggle's mouse-capture flip is armed, and the
    /// capture value it will apply.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn mouse_capture_pending_for_test(&self) -> Option<bool> {
        self.mouse_capture_pending
    }

    /// Hold the Audiobookshelf Service state and its stored credential user,
    /// as a login attempt's connect leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_service_connecting_for_test(
        &mut self,
        user: mbv_audiobookshelf::AudiobookshelfUser,
    ) {
        self.audiobookshelf_runtime.state = mbv_core::service_runtime::ServiceState::Connecting;
        self.audiobookshelf_runtime.user = Some(user);
    }

    /// Start the Audiobookshelf Service's setup attempt, as a Settings save
    /// does.
    #[cfg(any(test, feature = "test"))]
    pub fn begin_audiobookshelf_setup_for_test(&mut self) {
        self.audiobookshelf_runtime.begin_setup();
    }

    /// Hold `state` as the local queue's projected view, from the unified
    /// queue snapshot an Owner reports.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_state_for_test(&mut self, state: &mbv_ctrl::UnifiedQueueStateData) {
        self.local_view = crate::app::QueueView::from_snapshot(state);
    }

    /// Hold the local player's status as playing the queue's `current_idx`
    /// slot at `position_ticks`, over a runtime of `runtime_ticks`, as a
    /// status drain would (`None` leaves the runtime unset).
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playing_position_for_test(
        &mut self,
        current_idx: usize,
        position_ticks: i64,
        runtime_ticks: Option<i64>,
    ) {
        self.player.update_status(|status| {
            status.active = true;
            status.current_idx = current_idx;
            status.position_ticks = position_ticks;
            if let Some(runtime) = runtime_ticks {
                status.runtime_ticks = runtime;
            }
        });
    }

    /// Hold `len` as the local player's queued slot count, as a status drain
    /// reports it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_player_queue_len_for_test(&mut self, len: usize) {
        self.player.update_status(|status| status.queue_len = len);
    }

    /// Move the local player's playhead to slot `current_idx`, as a status
    /// drain reports it, leaving the rest of its status as it stands.
    #[cfg(any(test, feature = "test"))]
    pub fn set_player_current_idx_for_test(&mut self, current_idx: usize) {
        self.player
            .update_status(|status| status.current_idx = current_idx);
    }

    /// Hold `title` as the local player's reported title, as a status drain
    /// leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_player_title_for_test(&mut self, title: String) {
        self.player.update_status(|status| status.title = title);
    }

    /// Mark the local player idle after its queue was adopted.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_idle_player_for_test(&mut self) {
        self.player.update_status(|status| status.active = false);
    }

    /// Hold `position` as the saved library position of `library_id`.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_library_position_for_test(
        &mut self,
        library_id: &str,
        position: mbv_queue::LibraryPosition,
    ) {
        self.library_position_state
            .libraries
            .insert(library_id.to_string(), position);
    }

    /// Hold `state` as the launch restore the shell settles.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_launch_restore_for_test(
        &mut self,
        state: crate::app::state::app_struct::LaunchRestore,
    ) {
        self.launch_restore = state;
    }

    /// The launch restore the shell holds: whether a saved launch is still
    /// pending, settled onto a tab, or spent.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn launch_restore_for_test(&self) -> crate::app::state::app_struct::LaunchRestore {
        self.launch_restore.clone()
    }

    /// Empty the Emby catalog, as a Service teardown that dropped every view
    /// leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn clear_libraries_for_test(&mut self) {
        self.libs.clear();
    }

    /// Apply one library event through the production drain, with no Client
    /// stores mounted.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_lib_event_for_test(&mut self, event: crate::app::LibEvent) {
        self.handle_lib_event(event, None);
    }

    /// The stored Recently added rows under `source`, as the drain left them.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn home_recently_added_rows_for_test(
        &self,
        source: &crate::app::state::playback::DestinationLatestSource,
    ) -> &[mbv_queue::QueueItem] {
        self.home_recently_added_for(source)
    }

    /// Hold the local player's idle volume and mute, as a Settings save leaves
    /// them: the pair the status bar's indicator shows while nothing plays.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_idle_volume_and_mute_for_test(&mut self, volume: u8, muted: bool) {
        self.ui_volume = volume;
        self.mute_on = muted;
    }

    /// Send `event` on the core's library channel, as a fetch worker, a Service
    /// teardown, or a launch restore sends it.
    ///
    /// # Panics
    ///
    /// Panics when the library channel holds no receiver.
    #[cfg(any(test, feature = "test"))]
    pub fn send_lib_event_for_test(&mut self, event: crate::app::LibEvent) {
        self.channels
            .lib_tx
            .send(event)
            .expect("the library event channel holds a receiver");
    }

    /// Whether a library event the test did not expect is queued: the drain
    /// takes one event at a time, so what a send left behind is an extra
    /// fetch.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn has_queued_lib_event_for_test(&self) -> bool {
        self.try_recv_lib_event().is_some()
    }

    /// Send `action` on the core's notification-action channel.
    ///
    /// # Panics
    ///
    /// Panics when the notification-action channel holds no receiver.
    #[cfg(any(test, feature = "test"))]
    pub fn send_notif_action_for_test(&mut self, action: &str) {
        self.channels
            .notif_action_tx
            .send(action.to_string())
            .expect("the notification-action channel holds a receiver");
    }

    /// Send `event` on the core's session-event channel.
    ///
    /// # Panics
    ///
    /// Panics when the session-event channel holds no receiver.
    #[cfg(any(test, feature = "test"))]
    pub fn send_session_event_for_test(&mut self, event: crate::app::SessionEvent) {
        self.channels
            .sessions_tx
            .send(event)
            .expect("the session-event channel holds a receiver");
    }

    /// Hold `value` as whether the Sessions load is in flight.
    #[cfg(any(test, feature = "test"))]
    pub fn set_sessions_loading_for_test(&mut self, value: bool) {
        self.sessions_loading = value;
    }

    /// The Sessions the last `/Sessions` load reported.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn sessions_for_test(&self) -> &[mbv_emby::SessionInfo] {
        &self.sessions
    }

    /// Whether the Sessions load is in flight.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn sessions_loading_for_test(&self) -> bool {
        self.sessions_loading
    }

    /// The Emby Service's setup generation.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn emby_setup_generation_for_test(&self) -> mbv_core::service_runtime::SetupGeneration {
        self.emby_runtime.generation()
    }

    /// Start the Emby Service's setup attempt, as a Settings save does.
    #[cfg(any(test, feature = "test"))]
    pub fn begin_emby_setup_for_test(&mut self) {
        self.emby_runtime.begin_setup();
    }

    /// Hold `state` as the Emby Service state, as a connect leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_emby_service_state_for_test(
        &mut self,
        state: mbv_core::service_runtime::ServiceState,
    ) {
        self.emby_runtime.state = state;
    }

    /// The Audiobookshelf Service's setup generation.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn audiobookshelf_setup_generation_for_test(
        &self,
    ) -> mbv_core::service_runtime::SetupGeneration {
        self.audiobookshelf_runtime.generation()
    }

    /// Hold the local player's status as playing `queue_len` slots with
    /// `current_idx` active, as a status drain would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playing_status_for_test(&mut self, current_idx: usize, queue_len: usize) {
        self.player.update_status(|status| {
            status.active = true;
            status.current_idx = current_idx;
            status.queue_len = queue_len;
        });
    }
}
