use super::types_browse::{AlbumPathPart, AlbumSearchEntry, BrowseLevel};
use super::types_feed::FeedHomeVideoGroup;
use super::types_playback::{HomeContent, HomeLatestSource};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;

/// Per-kind landing payload for cross-surface item navigation (design D2 of
/// change `per-destination-item-navigation`): the Movie/generic arm keeps the
/// built ancestor-chain nav stack, the show arm carries the resolved reveal
/// Series, and the album arm carries the resolved reveal album plus its
/// folder chain between the library root and it (what the recursive album
/// activation consumes). Each kind is an explicit variant so every dispatch
/// site resolves the landing exhaustively.
pub(super) enum NavigateLanding {
    /// Movie/generic video: the ancestor-chain nav stack with the cursor
    /// resting on the item (the pre-change shape, kept verbatim).
    Chain { nav_stack: Vec<BrowseLevel> },
    /// TV: land the owning Series via the searched-series activation flow
    /// (task 2.2). `reveal` is the full series item, resolved and
    /// type-verified by the worker (design D1). `episode_id` carries the
    /// chosen episode for deep selection (task 6.1, design D6) — `Some` only
    /// when the reveal was resolved from an Episode; a Season reveal stays
    /// show-level with default selection.
    Series {
        reveal: Box<EmbyItem>,
        episode_id: Option<String>,
    },
    /// Music: land the owning album via recursive album activation
    /// (task 2.3). `reveal` is the full album item; `ancestors` is the
    /// root→album folder chain the activation walks (same shape the album
    /// index builds), empty for a flat album-at-root library.
    Album {
        reveal: Box<EmbyItem>,
        ancestors: Vec<AlbumPathPart>,
        /// Deep selection (task 6.2, design D6): the chosen track's id when
        /// the reveal was resolved from an Audio track.
        track_id: Option<String>,
    },
}

/// A `NavigateLanding::Series` the target library's corpus could not satisfy
/// yet (U2 correction): armed when the library has no loaded root level, the
/// series sits outside the loaded page, or the whole-library `all_items`
/// cache is absent. `Ensure-then-land`: the arm grows the corpus via
/// `ensure_lib_loaded_for`/`spawn_all_items_prefetch` and
/// `App::retry_pending_series_landing` retries the landing on that
/// library's next `Loaded`/`AllItemsPrefetched`/restored-position drain.
/// Cleared by the retry on success (land, save, switch per D4) or on a miss
/// against a complete corpus (flash, tab unchanged), by `LibEvent::Error`,
/// and by any manual tab change.
pub(super) struct PendingSeriesLanding {
    pub(super) lib_idx: usize,
    pub(super) reveal: Box<EmbyItem>,
    pub(super) switch_tab: bool,
    /// Deep selection carried through the deferred landing (task 6.1).
    pub(super) episode_id: Option<String>,
}

/// A completed `NavigateLanding::Series` that still owes the shell's detail
/// hand-off (task 3.1, design D3): armed by the per-kind landing on success --
/// the immediate `NavigateTo` arm or the deferred
/// `retry_pending_series_landing` -- and consumed by the shell's sync pass
/// (`Model::drain_series_navigation_handoff`), which runs the same
/// presentation sequence Inline Search's series activation runs.
///
/// Lifecycle: the landing has already succeeded when this is armed, so it is
/// NOT cleared by `LibEvent::Error` (an unrelated error after the landing must
/// not swallow the owed workspace/overlay open); it survives to the next sync
/// pass. A manual tab change in between discards it silently, without
/// surfacing an error.
pub(super) struct PendingSeriesHandoff {
    pub(super) lib_idx: usize,
    pub(super) reveal: Box<EmbyItem>,
    /// Deep selection carried through the hand-off (task 6.1, design D6).
    pub(super) episode_id: Option<String>,
}

pub(super) enum LibEvent {
    Loaded {
        lib_idx: usize,
        parent_id: String,
        level: Box<BrowseLevel>,
    },
    PageAppended {
        lib_idx: usize,
        parent_id: String,
        items: Vec<EmbyItem>,
        total_count: usize,
    },
    Refreshed {
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        items: Vec<EmbyItem>,
        total_count: usize,
    },
    SearchItemsLoaded {
        lib_idx: usize,
        parent_id: String,
        items: Vec<EmbyItem>,
    },
    AlbumIndexBuilt {
        library_id: String,
        result: Result<Vec<AlbumSearchEntry>, String>,
    },
    RecursiveAlbumActivated {
        library_id: String,
        nav_stack: Vec<BrowseLevel>,
    },
    AllItemsPrefetched {
        lib_idx: usize,
        parent_id: String,
        items: Vec<EmbyItem>,
    },
    FeedHomeVideoAggregated {
        lib_idx: usize,
        parent_id: String,
        all_items: Vec<EmbyItem>,
        groups: Vec<FeedHomeVideoGroup>,
    },
    AlbumArtistFetched {
        album_id: String,
        artist: String,
    },
    /// One level-fill request resolved (design D4 of
    /// `fix-music-artist-resolution-batching`): `artists` bulk-fills the
    /// album-artist cache for every album bucket in the level. Empty on HTTP
    /// failure (or a trackless level), which marks the level `Failed`.
    /// AlbumArtistLevelFetched` marks `Failed` or `Filled` on arrival; the
    /// producing worker thread is the level fetch (task 2.1, not wired yet).
    #[allow(dead_code)]
    AlbumArtistLevelFetched {
        level_id: String,
        artists: Vec<(String, String)>,
    },
    /// Track list for the album currently highlighted in the
    /// album-folder listing, fetched proactively (#145) so the inline album
    /// detail pane has data without a nav_stack drilldown.
    AlbumTracksFetched {
        album_id: String,
        tracks: Vec<EmbyItem>,
    },
    /// TV series detail (seasons + episodes) fetched proactively for inline
    /// rendering when a Series is selected.
    SeriesDetailFetched {
        series_id: String,
        seasons: Vec<EmbyItem>,
        episodes: std::collections::HashMap<String, Vec<EmbyItem>>,
    },
    /// Episodes for a specific season fetched when switching seasons in
    /// series-selection mode.
    SeriesSeasonEpisodesFetched {
        series_id: String,
        season_id: String,
        episodes: Vec<EmbyItem>,
    },
    AudiobookshelfDetailFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        /// The fetch's request serial (the browse state's
        /// `next_detail_request` at spawn): an arrival retires its in-flight
        /// mark and may write the cache only when the show's current mark
        /// still carries this serial.
        request: u64,
        library_item_id: String,
        result: Result<
            Vec<mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode>,
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    },
    AudiobookshelfShowsFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            mbv_core::audiobookshelf::AudiobookshelfShowPage,
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    },
    AudiobookshelfBooksFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            mbv_core::audiobookshelf::AudiobookshelfBookPage,
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    },
    AudiobookshelfShelfFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            Vec<mbv_core::audiobookshelf::AudiobookshelfShelf>,
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    },
    AudiobookshelfBookDetailFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        library_item_id: String,
        result: Result<
            (
                Vec<mbv_core::audiobookshelf::AudiobookshelfChapter>,
                Vec<mbv_core::audiobookshelf::AudiobookshelfAudioFile>,
            ),
            mbv_core::audiobookshelf::AudiobookshelfError,
        >,
    },
    AudiobookshelfProgressAcknowledged(mbv_core::player::AudiobookshelfProgressUpdate),
    AudiobookshelfBookProgressAcknowledged(mbv_core::player::AudiobookshelfBookProgressUpdate),
    /// `switch_tab`: true for user-initiated navigation (switch to the lib tab),
    /// false for startup restore (just populate nav_stack, stay on current tab).
    NavigateTo {
        lib_idx: usize,
        landing: NavigateLanding,
        switch_tab: bool,
    },
    RestoreLibraryPosition {
        lib_idx: usize,
        requested_position: crate::config::LibraryPosition,
        position: crate::config::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    },
    PlaylistsLoaded(Vec<EmbyItem>),
    PlaylistsLoadError(String),
    PlaylistItemsLoaded {
        playlist_id: String,
        items: Vec<EmbyItem>,
    },
    PlaylistItemsLoadError {
        playlist_id: String,
        error: String,
    },
    PlaylistRenamed {
        new_name: String,
    },
    PlaylistDeleted {
        name: String,
    },
    /// Best-effort background refresh of played/position state for the queue
    /// that `restore_queue_state` already populated synchronously from disk.
    /// See `spawn_enrich_queue_state`.
    #[rustfmt::skip]
    QueueEnriched { items: Vec<EmbyItem> },
    /// A freshly computed Home content snapshot delivered by an App-internal
    /// Home writer (task 5.3d): the fetch call itself is never deferred (its
    /// App-side side effects — `rebuild_library_tabs_from_views` +
    /// `start_album_index`, and `refresh_after_stop`'s feed-home-video loop —
    /// are synchronous and order-sensitive), but App-internal callers cannot
    /// touch Model-owned `home_content`, so the computed result travels here
    /// and the shell assigns it at the lib_rx drain, then re-projects.
    HomeContentRefreshed(Box<HomeContent>),
    /// The Emby service has been cleared (removed/replaced); the shell
    /// resets the Model-owned Continue Watching data (items, cursor, latest)
    /// and re-projects. The `loading` flag is intentionally untouched,
    /// matching the legacy `clear_emby_memory` which never reset it.
    HomeContentCleared,
    /// The Audiobookshelf Latest pill sections rebuilt from the shelf cache
    /// after a shelf fetch (task 5.3d). Cross-provider pill state lives in
    /// the Model, so the shell merges these into `home_content.latest` (the
    /// shared `merge_home_sections` splice) and re-projects.
    AudiobookshelfLatestRebuilt(Vec<(String, HomeLatestSource, Vec<QueueItem>)>),
    /// The Feeds Latest pill section (at most one) rebuilt from the Feeds
    /// tab after a refresh (task 5.3d). Like `AudiobookshelfLatestRebuilt`,
    /// the shell merges it into Model-owned `latest` — the feed drain runs
    /// after the lib_rx drain, so this lands on the next loop pass (a
    /// bounded one-iteration latency on the Feeds pill).
    FeedsLatestRebuilt(Vec<(String, HomeLatestSource, Vec<QueueItem>)>),
    Error(String),
}

pub(super) enum SessionEvent {
    Loaded {
        sessions: Vec<mbv_core::api::SessionInfo>,
    },
    CommandError {
        error: String,
    },
    PlaylistMutationComplete {
        mutation_id: u64,
        playlist_id: String,
        queue_lineage: u64,
        source_playlist_id: String,
        result: Result<(), String>,
    },
    PlaylistReplacementComplete {
        mutation_id: u64,
        playlist_id: String,
        queue_lineage: u64,
        name: String,
        result: Result<String, String>,
    },
    PlaylistCreateComplete {
        mutation_id: u64,
        coordinator_key: String,
        name: String,
        queue_lineage: u64,
        source_playlist_id: Option<String>,
        result: Result<String, String>,
    },
    Error(String),
}
