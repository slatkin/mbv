use crate::app::state::types::browse::{AlbumPathPart, AlbumSearchEntry, BrowseLevel};
use crate::app::state::types::feed::FeedHomeVideoGroup;
use crate::app::state::types::playback::HomeContent;
use mbv_core::api::EmbyItem;
use mbv_core::service_runtime::SetupGeneration;

/// Per-kind landing payload for cross-surface item navigation (design D2 of
/// change `per-destination-item-navigation`): the Movie/generic arm keeps the
/// built ancestor-chain nav stack, the show arm carries the resolved reveal
/// Series, and the album arm carries the resolved reveal album plus its
/// folder chain between the library root and it (what the recursive album
/// activation consumes). Each kind is an explicit variant so every dispatch
/// site resolves the landing exhaustively.
pub(in crate::app) enum NavigateLanding {
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
pub(in crate::app) struct PendingSeriesLanding {
    pub(in crate::app) lib_idx: usize,
    pub(in crate::app) reveal: Box<EmbyItem>,
    pub(in crate::app) switch_tab: bool,
    /// Deep selection carried through the deferred landing (task 6.1).
    pub(in crate::app) episode_id: Option<String>,
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
pub(in crate::app) struct PendingSeriesHandoff {
    pub(in crate::app) lib_idx: usize,
    pub(in crate::app) reveal: Box<EmbyItem>,
    /// Deep selection carried through the hand-off (task 6.1, design D6).
    pub(in crate::app) episode_id: Option<String>,
}

pub(in crate::app) enum LibEvent {
    Loaded {
        lib_idx: usize,
        parent_id: String,
        level: Box<BrowseLevel>,
    },
    EmbyLatestSnapshotFetched {
        library_id: String,
        title: String,
        items: Vec<EmbyItem>,
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
    /// One level-fill request resolved (design D4 of
    /// `fix-music-artist-resolution-batching`): `artists` bulk-fills the
    /// album-artist cache for every album bucket in the level. Empty on HTTP
    /// failure (or a trackless level), which marks the level `Failed`; the
    /// handler otherwise marks `Filled` on arrival.
    AlbumArtistLevelFetched {
        level_id: String,
        artists: Vec<(String, String)>,
    },
    /// Startup warm-up (design D5 of `fix-music-artist-resolution-batching`):
    /// a music library's group-level listing (its root children), fetched in
    /// the background once the Service became Ready. The handler queues one
    /// level fill per listed child and drains the bounded warm-up scheduler,
    /// deduped through the shared level-fill state. Not presentation-affecting
    /// (no shell arm): it only mutates App
    /// caches, and failures arrive as empty `AlbumArtistLevelFetched`
    /// artists that merely mark the level `Failed`.
    MusicGroupWarmupListed {
        generation: SetupGeneration,
        groups: Vec<EmbyItem>,
    },
    /// Track list for the album currently highlighted in the
    /// album-folder listing, fetched proactively (#145) so the inline album
    /// detail pane has data without a nav_stack drilldown.
    AlbumTracksFetched {
        album_id: String,
        tracks: Vec<EmbyItem>,
    },
    /// Completion of the typed `ArtistItems`-ID Audio query (design D7,
    /// task 6.1). The destination, Service setup generation, and settled
    /// revision all ride the completion so the shell can reject a response
    /// from a navigated-away or replaced source instead of painting it.
    ArtistTracksFetched {
        destination: crate::app::components::library_panel::LibraryKey,
        generation: mbv_core::service_runtime::SetupGeneration,
        artist_id: String,
        revision: u64,
        result: Result<Vec<EmbyItem>, String>,
    },
    /// Completion notification emitted after the shared image/cache boundary
    /// has stored an artist's stable-ID artwork (task 6.2).
    ArtistArtworkFetched {
        destination: crate::app::components::library_panel::LibraryKey,
        generation: mbv_core::service_runtime::SetupGeneration,
        artist_id: String,
        revision: u64,
        cache_key: String,
        available: bool,
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
    /// A freshly computed Continue Watching snapshot delivered by an
    /// App-internal writer. App-internal callers cannot touch Model-owned
    /// `home_content`, so the result travels here for shell assignment.
    HomeContentRefreshed(Box<HomeContent>),
    /// The Emby service has been cleared (removed/replaced); the shell
    /// resets the Model-owned Continue Watching items and re-projects. The
    /// `loading` flag is intentionally untouched,
    /// matching the legacy `clear_emby_memory` which never reset it.
    HomeContentCleared,
    Error(String),
}

pub(in crate::app) enum SessionEvent {
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
        owner_queue_lineage: Option<mbv_core::ctrl::QueueLineage>,
        result: Result<String, String>,
    },
    Error(String),
}
