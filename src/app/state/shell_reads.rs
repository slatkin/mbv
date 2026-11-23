//! The read methods the TUI shell calls to learn what the core holds.
//!
//! A Client never reaches an `App` field (change `split-tui-gui-clients`,
//! design D4/D4b rule 1). Each method here names the fact the Client asks for
//! rather than the field it lives in, so the field may move, change type, or
//! gain an invariant without a Client noticing. The methods that already
//! answered a fact before this module existed stay where they are
//! (`effective_panel_focus`, `image_font_size`, `feeds_tab_pos`, ...); this
//! module adds only the reads the shell made directly until now.
//!
//! The tab and catalog facts are in `browse`; this file holds the Settings,
//! configuration, Player, image-paint, and channel facts.

mod browse;
mod config;

pub use self::browse::{BrowseLevelFacts, BrowseLevelIdentity};
pub use self::config::SetupFormDraft;

use super::panel_targets::PanelTarget;
use super::playback::HomeContent;
use crate::app::App;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::browse::AlbumIndexState;
use mbv_ui_model::indicators::IndicatorStyle;
use mbv_ui_model::playback::QueueCardProjection;

/// What one artwork cache entry last built its protocols from: the decoded
/// pixels, the hero box, and the logo the presenting Client applied.
pub struct ImageBuildFacts {
    pub img: image::DynamicImage,
    pub cover_box: Option<(u16, u16)>,
    pub logo_key: Option<String>,
}

/// The facts carry decoded pixels, so `Debug` names the type and the two small
/// facts instead of dumping the bitmap.
impl std::fmt::Debug for ImageBuildFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageBuildFacts")
            .field("cover_box", &self.cover_box)
            .field("logo_key", &self.logo_key)
            .finish_non_exhaustive()
    }
}

impl App {
    /// The recursive album index of the library with `library_id`.
    #[must_use]
    pub fn album_index(&self, library_id: &str) -> Option<&AlbumIndexState> {
        self.album_indexes.get(library_id)
    }

    /// The TV series detail cached for `series_id`, if its cards are fetched.
    #[must_use]
    pub fn series_detail(&self, series_id: &str) -> Option<&mbv_ui_model::browse::SeriesDetail> {
        self.series_detail_cache.get(series_id)
    }

    /// The TV Upcoming rows the library with `library_id` last fetched.
    #[must_use]
    pub fn tv_upcoming_rows(
        &self,
        library_id: &str,
    ) -> Option<&[mbv_emby_model::UpcomingPremiere]> {
        self.tv_upcoming_snapshots
            .get(library_id)
            .map(Vec::as_slice)
    }

    /// Whether the library's last TV Upcoming fetch finally failed.
    #[must_use]
    pub fn tv_upcoming_fetch_failed(&self, library_id: &str) -> bool {
        matches!(
            self.tv_upcoming_fetches.get(library_id),
            Some(crate::app::state::app_struct::TvUpcomingFetchState::Failed)
        )
    }

    /// The series whose detail fetch is in flight, as a Client's own
    /// expansion armed it. A test observes the arm and its completion
    /// through it; the fetch itself is the production path under test.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn series_detail_loading_for_test(&self) -> &std::collections::HashSet<String> {
        &self.series_detail_loading
    }

    /// The seasons whose episode fetch is in flight, as `(series id, season
    /// id)`.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn series_season_loading_for_test(&self) -> &std::collections::HashSet<(String, String)> {
        &self.series_season_loading
    }

    /// The season expansions that wait on a series detail fetch, as
    /// `(series id, season id)`.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_series_season_expansions_for_test(
        &self,
    ) -> &std::collections::HashSet<(String, String)> {
        &self.pending_series_season_expansions
    }

    /// Whether a Client painted `index`'s Wide TV series Workspace wide in its
    /// last pass: the write-back fact the shared Series-activation branch
    /// reads.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn tv_workspace_painted_wide_for_test(&self, index: usize) -> bool {
        self.tv_workspace_wide.contains(&index)
    }

    /// Whether the albums' track lists cache holds `album_id`.
    #[must_use]
    pub fn album_tracks_cached(&self, album_id: &str) -> bool {
        self.album_tracks_cache.contains_key(album_id)
    }

    /// Whether the albums' track lists cache is fetching `album_id`.
    #[must_use]
    pub fn album_tracks_loading(&self, album_id: &str) -> bool {
        self.album_tracks_loading.contains(album_id)
    }

    /// Whether any album's track list is loading.
    #[must_use]
    pub fn album_tracks_any_loading(&self) -> bool {
        !self.album_tracks_loading.is_empty()
    }

    /// The cached track list of `album_id`.
    #[must_use]
    pub fn album_tracks(&self, album_id: &str) -> Option<&[EmbyItem]> {
        self.album_tracks_cache.get(album_id).map(Vec::as_slice)
    }

    /// The album whose cached track list holds the track `track_id`.
    #[must_use]
    pub fn album_of_cached_track(&self, track_id: &str) -> Option<String> {
        self.album_tracks_cache
            .iter()
            .find_map(|(album_id, tracks)| {
                tracks
                    .iter()
                    .any(|track| track.id == track_id)
                    .then(|| album_id.clone())
            })
    }

    /// The cached track `track_id`, from whichever track list holds it.
    #[must_use]
    pub fn cached_album_track(&self, track_id: &str) -> Option<&EmbyItem> {
        self.album_tracks_cache
            .values()
            .flatten()
            .find(|track| track.id == track_id)
    }

    /// Album id to album artist, as the level fills recorded it: the lookup the
    /// grouped views label their album rows from.
    #[must_use]
    pub fn album_artists(&self) -> &std::collections::HashMap<String, String> {
        &self.album_artist_cache
    }

    /// Whether the artist detail for `key` is in flight.
    #[must_use]
    pub fn artist_detail_pending(
        &self,
        key: &mbv_ui_model::music_artist_detail::ArtistDetailKey,
    ) -> bool {
        self.artist_detail_loading.contains(key)
    }

    /// Whether any artist detail is in flight.
    #[must_use]
    pub fn artist_detail_any_pending(&self) -> bool {
        !self.artist_detail_loading.is_empty()
    }

    /// The user's playlists, as the last load reported them.
    #[must_use]
    pub fn playlists(&self) -> &[EmbyItem] {
        &self.playlists
    }

    /// The playlist the Playlists overlay has open, if any.
    #[must_use]
    pub fn playlists_open(&self) -> Option<&EmbyItem> {
        self.playlists_open.as_ref()
    }

    /// The open playlist's rows.
    #[must_use]
    pub fn playlists_open_items(&self) -> &[EmbyItem] {
        &self.playlists_open_items
    }

    /// The playlist at `index` in the loaded list.
    #[must_use]
    pub fn playlist_at(&self, index: usize) -> Option<&EmbyItem> {
        self.playlists.get(index)
    }

    /// The open playlist's row at `index`.
    #[must_use]
    pub fn playlists_open_item_at(&self, index: usize) -> Option<&EmbyItem> {
        self.playlists_open_items.get(index)
    }

    /// Whether the playlist list is loading.
    #[must_use]
    pub fn playlists_loading(&self) -> bool {
        self.playlists_loading
    }

    /// Whether the open playlist's rows are loading.
    #[must_use]
    pub fn playlists_open_loading(&self) -> bool {
        self.playlists_open_loading
    }

    #[must_use]
    pub fn player_remote_disconnected(&self) -> bool {
        self.player.is_remote_disconnected()
    }

    /// The local player's status at this instant.
    #[must_use]
    pub fn player_status_snapshot(&self) -> mbv_ctrl::player::PlayerStatus {
        self.player.status_snapshot()
    }

    /// The stored Recently added rows one source contributed.
    #[must_use]
    pub fn home_recently_added_for(
        &self,
        source: &super::playback::DestinationLatestSource,
    ) -> &[mbv_queue::QueueItem] {
        self.home_recently_added
            .get(source)
            .map_or(&[], Vec::as_slice)
    }

    /// The next queued library event, waiting until one arrives.
    pub fn recv_lib_event(&self) -> Option<crate::app::LibEvent> {
        self.channels.lib_rx.recv().ok()
    }

    /// Whether the active tab shows the Feeds destination.
    #[must_use]
    pub fn active_tab_is_feeds(&self) -> bool {
        self.tab.is_feeds()
    }

    /// A clone of the library-event sender, for a worker the shell starts.
    ///
    /// The shell's drains take the receivers through the `try_recv_*`
    /// methods below.
    #[must_use]
    pub fn lib_event_sender(&self) -> std::sync::mpsc::Sender<crate::app::LibEvent> {
        self.channels.lib_tx.clone()
    }

    /// The next queued library event, or `None` when the queue is empty.
    pub fn try_recv_lib_event(&self) -> Option<crate::app::LibEvent> {
        self.channels.lib_rx.try_recv().ok()
    }

    /// The next queued Search result, or `None` when the queue is empty.
    pub fn try_recv_search_result(
        &self,
    ) -> Option<(String, Result<Vec<EmbyItem>, mbv_emby::EmbyError>)> {
        self.channels.search_rx.try_recv().ok()
    }

    /// The merged Emby and Cast panel target list.
    #[must_use]
    pub fn panel_targets(&self) -> &[PanelTarget] {
        &self.panel_targets
    }

    /// The panel target a Sessions or Cast row key names, if the current
    /// target list holds it.
    #[must_use]
    pub fn resolve_session_target(
        &self,
        key: &mbv_ui_model::panel_targets::SessionTargetKey,
    ) -> Option<PanelTarget> {
        super::panel_targets::resolve_session_target(&self.panel_targets, key)
    }

    /// Whether a Sessions load is in flight.
    #[must_use]
    pub fn sessions_loading(&self) -> bool {
        self.sessions_loading
    }

    /// Home's content: the Continue Watching snapshot and its loading bit.
    #[must_use]
    pub fn home_content(&self) -> &HomeContent {
        &self.home_content
    }

    /// Whether the user's font choice paints Nerd Font glyphs.
    #[must_use]
    pub fn use_nerd_fonts(&self) -> bool {
        self.use_nerd_fonts
    }

    /// The configured status-indicator style.
    #[must_use]
    pub fn indicator_style(&self) -> IndicatorStyle {
        self.indicator_style
    }

    /// The Emby service state the status row and the Sessions panel paint.
    #[must_use]
    pub fn emby_service_state(&self) -> mbv_core::service_runtime::ServiceState {
        self.emby_runtime.state
    }

    /// The Emby client's setup generation: it changes when a Service setup is
    /// replaced, so a worker keeps its results scoped to it.
    #[must_use]
    pub fn emby_setup_generation(&self) -> mbv_core::service_runtime::SetupGeneration {
        self.emby_runtime.generation()
    }

    /// The user the Audiobookshelf Service is signed in as, while it holds
    /// one.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn audiobookshelf_service_user_for_test(
        &self,
    ) -> Option<&mbv_audiobookshelf::AudiobookshelfUser> {
        self.audiobookshelf_runtime.user.as_ref()
    }

    /// The Audiobookshelf service state the status row and the Sessions panel
    /// paint.
    #[must_use]
    pub fn audiobookshelf_service_state(&self) -> mbv_core::service_runtime::ServiceState {
        self.audiobookshelf_runtime.state
    }

    /// The Audiobookshelf client's setup generation.
    #[must_use]
    pub fn audiobookshelf_setup_generation(&self) -> mbv_core::service_runtime::SetupGeneration {
        self.audiobookshelf_runtime.generation()
    }

    /// The queue card's visual-slot projection.
    #[must_use]
    pub fn queue_card_projection(&self) -> &QueueCardProjection {
        &self.queue_card_projection
    }

    /// The local queue view's total length, as the save-playlist gate asks it.
    #[must_use]
    pub fn local_queue_len(&self) -> usize {
        self.local_view.total_queue_len()
    }

    /// The artwork cache this Client paints from.
    ///
    /// A Client's painter keeps its own protocol store beside the cache (the
    /// TUI's `ImageProtocols` encodes and prunes the terminal protocols; the
    /// GUI resolves its own), and the store's operations take the cache as the
    /// authority on what is stored and under which generation. So the Client
    /// borrows the cache and never mutates it: every write goes through a core
    /// method, because the core owns the fetch queue and the eviction rules.
    #[must_use]
    pub fn image_cache(&self) -> &mbv_images::cache::ImageCache {
        &self.images
    }

    /// The most-recent `Newest Episodes` shelf the podcast library with
    /// `library_id` last fetched, empty while it holds none.
    #[must_use]
    pub fn audiobookshelf_shelf(
        &self,
        library_id: &str,
    ) -> Vec<mbv_queue::AudiobookshelfEpisodeCatalog> {
        self.audiobookshelf_shelf_cache
            .get(library_id)
            .cloned()
            .unwrap_or_default()
    }

    /// The cached artwork entry stored under `key`.
    #[must_use]
    pub fn cached_image(&self, key: &str) -> Option<&mbv_images::CachedImage> {
        self.images.image(key)
    }

    /// Whether launch restoration has settled: the shell reads it before it
    /// decides whether a settled destination still owns the snapshot.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn launch_restore_settled_for_test(&self) -> bool {
        self.launch_restore == crate::app::state::app_struct::LaunchRestore::Done
    }

    /// The Emby client the ready runtime holds, for a Client's own test that
    /// drives a mocked transport: `None` while no client is cached.
    ///
    /// # Panics
    ///
    /// Panics when the runtime's client lock is poisoned: a writer panicked
    /// holding it, so the client cannot be read.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn cached_emby_client_for_test(&self) -> Option<mbv_emby::EmbyClient> {
        let client = self.emby_runtime.client.as_ref()?;
        Some(
            client
                .lock()
                .unwrap_or_else(|poisoned| {
                    panic!("the Emby client lock was poisoned: {poisoned:?}")
                })
                .clone(),
        )
    }

    /// The image cache's recorded card size, as the last produced paint left
    /// it: whether a paint moved the geometry checkpoint.
    #[cfg(any(test, feature = "test"))]
    pub fn record_card_size_for_test(&mut self, height: u16, width: u16) {
        self.images.record_card_size(height, width);
    }

    /// The decoded pixels the artwork entry stored under `key` holds, cloned.
    /// `None` for an absent entry and for one that resolved without artwork.
    #[must_use]
    pub fn cached_image_pixels(&self, key: &str) -> Option<image::DynamicImage> {
        self.images.image(key).and_then(|entry| entry.img.clone())
    }

    /// Whether the artwork entry stored under `key` paints art: an absent
    /// entry counts as paintable, because the caller then paints the bundled
    /// placeholder. A stored entry that carries no decoded image does not.
    #[must_use]
    pub fn image_key_paints(&self, key: &str) -> bool {
        self.images
            .image(key)
            .is_none_or(mbv_images::CachedImage::has_art)
    }

    /// Whether the artwork entry stored under `key` carries a decoded image,
    /// as a derived title overlay variant must.
    #[must_use]
    pub fn image_cached_with_art(&self, key: &str) -> bool {
        self.images
            .image(key)
            .is_some_and(mbv_images::CachedImage::has_art)
    }

    /// The hero box and the logo the artwork entry stored under `key` last
    /// built its protocols from, beside its decoded pixels.
    #[must_use]
    pub fn cached_image_build_facts(&self, key: &str) -> Option<ImageBuildFacts> {
        let entry = self.images.image(key)?;
        let img = entry.img.clone()?;
        Some(ImageBuildFacts {
            img,
            cover_box: entry.cover_box,
            logo_key: entry.applied_logo_key.clone(),
        })
    }

    /// Whether the artwork cache holds an entry stored under `key`.
    #[must_use]
    pub fn image_cached(&self, key: &str) -> bool {
        self.images.is_cached(key)
    }

    /// Whether a fetch for `key` is outstanding.
    #[must_use]
    pub fn image_loading(&self, key: &str) -> bool {
        self.images.is_loading(key)
    }

    /// Whether the last fetch for `key` got no answer.
    #[must_use]
    pub fn image_fetch_interrupted(&self, key: &str) -> bool {
        self.images.is_fetch_interrupted(key)
    }

    /// The size the queue card's artwork last painted at.
    #[must_use]
    pub fn last_card_size(&self) -> (u16, u16) {
        self.images.last_card_size()
    }

    /// Whether the image cache paints a terminal image protocol at all.
    #[must_use]
    pub fn image_protocol_enabled(&self) -> bool {
        self.images.protocol_enabled()
    }

    /// The configured terminal image protocol, if the user pinned one.
    #[must_use]
    pub fn image_protocol_override(&self) -> Option<&str> {
        self.images.protocol_override()
    }

    /// The owner's reported playback output, as of the last drain.
    #[must_use]
    pub fn playback_output(&self) -> Option<&mbv_ctrl::OutputState> {
        self.playback_output.as_ref()
    }

    /// Whether a dimming backdrop currently forces the halfblock fallback.
    #[must_use]
    pub fn dim_backdrop_active(&self) -> bool {
        self.dim_backdrop_active
    }

    /// Whether the next frame must repaint from a cleared screen.
    #[must_use]
    pub fn force_clear(&self) -> bool {
        self.force_clear
    }

    /// The visualizer capture worker, while a capture runs.
    #[must_use]
    pub fn visualizer_worker(&self) -> Option<&mbv_visualizer::PipeWireWorker> {
        self.visualizer.as_ref()
    }

    /// The latest stereo sample window the visualizer painted from.
    #[must_use]
    pub fn visualizer_window(&self) -> &mbv_visualizer::StereoSampleWindow {
        &self.visualizer_window
    }

    /// The glyph the visualizer paints.
    #[must_use]
    pub fn visualizer_glyph(&self) -> &str {
        &self.visualizer_glyph
    }

    /// The idle feed the playback panel names while it rests.
    #[must_use]
    pub fn idle_feed(&self) -> Option<&mbv_ui_model::feed::IdleFeed> {
        self.idle_feed.as_ref()
    }

    /// The configured list-pane width override, if any.
    #[must_use]
    pub fn list_pane_width(&self) -> Option<u16> {
        self.list_pane_width
    }

    /// The queue column's configured width.
    #[must_use]
    pub fn queue_column_width(&self) -> u16 {
        self.queue_column_width
    }

    /// The stored Panel focus, before the mini-view and hidden-column rules
    /// override it for routing: what the focus setters hold.
    #[must_use]
    pub fn stored_panel_focus(&self) -> mbv_ui_model::settings::PanelFocus {
        self.panel_focus
    }

    /// Whether the prefix key is armed.
    #[must_use]
    pub fn prefix_armed(&self) -> bool {
        self.prefix_armed
    }

    /// The launch snapshot a settled tab still owes, while `tab` is the tab it
    /// settled on. A snapshot that settled on another tab is stale: it is
    /// cleared here and `None` comes back, so a launch never restores onto a tab
    /// the user moved on to.
    #[must_use]
    pub fn settled_launch_state_for(
        &mut self,
        tab: mbv_ui_model::tab_selection::TabSelection,
    ) -> Option<mbv_config::TuiLaunchState> {
        let settled_tab = match &self.launch_restore {
            crate::app::state::app_struct::LaunchRestore::TabSettled { tab, .. } => *tab,
            _ => return None,
        };
        if settled_tab != tab {
            self.launch_restore = crate::app::state::app_struct::LaunchRestore::Done;
            return None;
        }
        match &self.launch_restore {
            crate::app::state::app_struct::LaunchRestore::TabSettled { state, .. } => {
                Some(state.clone())
            }
            _ => None,
        }
    }

    /// The overlay request the core raised and the Client has not mounted yet.
    #[must_use]
    pub fn pending_overlay(&self) -> Option<&mbv_ui_model::overlay::OverlayRequest> {
        self.pending_overlay.as_ref()
    }

    /// Take the Series landing hand-off the core armed, for the shell's sync
    /// pass to run the Inline Search series presentation sequence.
    pub fn take_pending_series_handoff(
        &mut self,
    ) -> Option<crate::app::state::events::PendingSeriesHandoff> {
        self.pending_series_handoff.take()
    }

    /// Take the message an announced daemon shutdown wants printed once, after
    /// the Client has restored its terminal.
    pub fn take_pending_exit_message(&mut self) -> Option<String> {
        self.pending_exit_message.take()
    }

    /// Take the pending mouse-capture flip the Settings toggle requested, for
    /// the Client's run loop to apply to its own terminal.
    pub fn take_mouse_capture_request(&mut self) -> Option<bool> {
        self.mouse_capture_pending.take()
    }

    /// Take the pending tab-strip reveal request, clearing it: the core raises
    /// it where the Client must scroll its tab strip onto the current tab.
    pub fn take_tab_reveal_request(&mut self) -> bool {
        std::mem::take(&mut self.tab_reveal_requested)
    }

    /// The Client capability set this Client recorded at its Owner attach.
    #[must_use]
    pub fn client_capabilities(&self) -> &super::client_capabilities::ClientCapabilities {
        &self.client_capabilities
    }

    /// The presentation facts a Client's test reads back after an action: the
    /// Panel mode and focus, the queue column and hero list-pane widths, the
    /// visual slot, the volume and mute the idle indicator shows, and the
    /// Settings destination.
    ///
    /// A UI-state-reset test asserts what the reset restored to these fields
    /// and what it protected in the volume/mute pair.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn presentation_facts_for_test(&self) -> PresentationFacts {
        PresentationFacts {
            panel_mode: self.panel_mode,
            panel_focus: self.panel_focus,
            mini_view_focus: self.mini_view_focus,
            queue_column_width: self.queue_column_width,
            list_pane_width: self.list_pane_width,
            visual_slot_hidden: self.visual_slot_hidden,
            stored_volume: self.ui_volume,
            stored_mute: self.mute_on,
            settings_destination: self.settings_destination,
        }
    }
}

/// The presentation facts [`App::presentation_facts_for_test`] reads back.
#[cfg(any(test, feature = "test"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationFacts {
    pub panel_mode: mbv_ui_model::settings::PanelMode,
    pub panel_focus: mbv_ui_model::settings::PanelFocus,
    pub mini_view_focus: mbv_ui_model::settings::PanelFocus,
    pub queue_column_width: u16,
    pub list_pane_width: Option<u16>,
    pub visual_slot_hidden: bool,
    /// The stored idle volume, as the indicator reads it while nothing plays.
    pub stored_volume: u8,
    /// Whether the stored idle mute is on.
    pub stored_mute: bool,
    pub settings_destination: mbv_ui_model::settings::SettingsDestination,
}
