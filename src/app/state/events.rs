use crate::app::state::playback::HomeContent;
use mbv_core::service_runtime::SetupGeneration;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::browse::{AlbumPathPart, AlbumSearchEntry, BrowseLevel};
use mbv_ui_model::feed::FeedHomeVideoGroup;

/// Per-kind landing payload for cross-surface item navigation (design D2 of
/// change `per-destination-item-navigation`): the Movie/generic arm keeps the
/// built ancestor-chain nav stack, the show arm carries the resolved reveal
/// Series, and the album arm carries the resolved reveal album plus its
/// folder chain between the library root and it (what the recursive album
/// activation consumes). Each kind is an explicit variant so every dispatch
/// site resolves the landing exhaustively.
#[derive(Debug)]
pub enum NavigateLanding {
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
#[derive(Debug)]
pub struct PendingSeriesLanding {
    pub lib_idx: usize,
    pub reveal: Box<EmbyItem>,
    pub switch_tab: bool,
    /// Deep selection carried through the deferred landing (task 6.1).
    pub episode_id: Option<String>,
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
#[derive(Debug)]
pub struct PendingSeriesHandoff {
    pub lib_idx: usize,
    pub reveal: Box<EmbyItem>,
    /// Deep selection carried through the hand-off (task 6.1, design D6).
    pub episode_id: Option<String>,
}

#[derive(Debug)]
pub enum BrowseEvent {
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
    /// `switch_tab`: true for user-initiated navigation (switch to the lib tab),
    /// false for startup restore (just populate `nav_stack`, stay on current tab).
    NavigateTo {
        lib_idx: usize,
        landing: NavigateLanding,
        switch_tab: bool,
    },
    RestoreLibraryPosition {
        lib_idx: usize,
        requested_position: mbv_queue::LibraryPosition,
        position: mbv_queue::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    },
}

#[derive(Debug)]
pub enum MusicEvent {
    AlbumIndexBuilt {
        library_id: String,
        result: Result<Vec<AlbumSearchEntry>, mbv_emby::EmbyError>,
    },
    RecursiveAlbumActivated {
        library_id: String,
        nav_stack: Vec<BrowseLevel>,
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
    GroupWarmupListed {
        generation: SetupGeneration,
        groups: Vec<EmbyItem>,
    },
    /// Track list for the album currently highlighted in the
    /// album-folder listing, fetched proactively (#145) so the inline album
    /// detail pane has data without a `nav_stack` drilldown.
    AlbumTracksFetched {
        album_id: String,
        tracks: Vec<EmbyItem>,
    },
    /// Completion of the typed `ArtistItems`-ID Audio query (design D7,
    /// task 6.1). The destination, Service setup generation, and settled
    /// revision all ride the completion so the shell can reject a response
    /// from a navigated-away or replaced source instead of painting it.
    ArtistTracksFetched {
        destination: mbv_ui_model::library::LibraryKey,
        generation: mbv_core::service_runtime::SetupGeneration,
        artist_id: String,
        revision: u64,
        result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
    },
    /// Completion notification emitted after the shared image/cache boundary
    /// has stored an artist's stable-ID artwork (task 6.2).
    ArtistArtworkFetched {
        destination: mbv_ui_model::library::LibraryKey,
        generation: mbv_core::service_runtime::SetupGeneration,
        artist_id: String,
        revision: u64,
        cache_key: String,
        available: bool,
    },
}

#[derive(Debug)]
pub enum SeriesEvent {
    /// TV series detail (seasons + episodes) fetched proactively for inline
    /// rendering when a Series is selected.
    DetailFetched {
        series_id: String,
        seasons: Vec<EmbyItem>,
        episodes: std::collections::HashMap<String, Vec<EmbyItem>>,
    },
    /// Episodes for a specific season fetched when switching seasons in
    /// series-selection mode.
    SeasonEpisodesFetched {
        series_id: String,
        season_id: String,
        episodes: Vec<EmbyItem>,
    },
}

#[derive(Debug)]
pub enum AudiobookshelfEvent {
    DetailFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        /// The fetch's request serial (the browse state's
        /// `next_detail_request` at spawn): an arrival retires its in-flight
        /// mark and may write the cache only when the show's current mark
        /// still carries this serial.
        request: u64,
        library_item_id: String,
        result: Result<
            Vec<mbv_audiobookshelf::AudiobookshelfDownloadedEpisode>,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    },
    ShowsFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        /// The catalog request serial the fetch was issued under (the browse
        /// state's `catalog_request`): a pre-refresh result carrying a
        /// superseded serial is discarded whole.
        request: u64,
        library_id: String,
        result: Result<
            mbv_audiobookshelf::AudiobookshelfShowPage,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    },
    BooksFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        /// The catalog request serial the fetch was issued under (the browse
        /// state's `catalog_request`): a pre-refresh result carrying a
        /// superseded serial is discarded whole.
        request: u64,
        library_id: String,
        result: Result<
            mbv_audiobookshelf::AudiobookshelfBookPage,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    },
    ShelfFetched {
        generation: mbv_core::service_runtime::SetupGeneration,
        library_id: String,
        result: Result<
            Vec<mbv_audiobookshelf::AudiobookshelfShelf>,
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    },
    BookDetailLoaded {
        generation: mbv_core::service_runtime::SetupGeneration,
        /// The fetch's request serial (the book browse state's
        /// `next_detail_request` at spawn): an arrival retires its in-flight
        /// mark and may write the cache only when the book's current mark
        /// still carries this serial.
        request: u64,
        library_item_id: String,
        result: Result<
            (
                Vec<mbv_audiobookshelf::AudiobookshelfChapter>,
                Vec<mbv_audiobookshelf::AudiobookshelfAudioFile>,
            ),
            mbv_audiobookshelf::AudiobookshelfError,
        >,
    },
}

#[derive(Debug)]
pub enum PlaylistEvent {
    ListLoaded(Vec<EmbyItem>),
    ListLoadError(String),
    ItemsLoaded {
        playlist_id: String,
        items: Vec<EmbyItem>,
    },
    ItemsLoadError {
        playlist_id: String,
        error: String,
    },
    Renamed {
        new_name: String,
    },
    Deleted {
        name: String,
    },
}

#[derive(Debug)]
pub enum ModelContentEvent {
    EmbyLatestSnapshotFetched {
        library_id: String,
        title: String,
        items: Vec<EmbyItem>,
    },
    /// A freshly computed Continue Watching snapshot delivered by an
    /// App-internal writer. App-internal callers cannot touch Model-owned
    /// `home_content`, so the result travels here for shell assignment.
    HomeContentRefreshed(Box<HomeContent>),
    /// The Emby service has been cleared (removed/replaced); the shell
    /// resets the Model-owned Continue Watching items and re-projects. The
    /// `loading` flag is intentionally untouched,
    /// matching the legacy `clear_emby_memory` which never reset it.
    HomeContentCleared,
}

/// Library events are grouped by the App state handler that owns them.
/// A new variant goes in the family whose handler file owns its state.
#[derive(Debug)]
pub enum LibEvent {
    Browse(BrowseEvent),
    Music(MusicEvent),
    Series(SeriesEvent),
    Audiobookshelf(AudiobookshelfEvent),
    Playlist(PlaylistEvent),
    ModelContent(ModelContentEvent),
    Error(String),
}

#[derive(Debug)]
pub enum SessionEvent {
    Loaded {
        sessions: Vec<mbv_emby::SessionInfo>,
    },
    CommandError {
        error: String,
    },
    PlaylistMutationComplete {
        mutation_id: u64,
        playlist_id: String,
        origin: crate::app::state::queue_owner::QueueOrigin,
        source_playlist_id: String,
        result: Result<(), mbv_emby::EmbyError>,
    },
    PlaylistReplacementComplete {
        mutation_id: u64,
        playlist_id: String,
        origin: crate::app::state::queue_owner::QueueOrigin,
        name: String,
        result: Result<String, mbv_emby::EmbyError>,
    },
    PlaylistCreateComplete {
        mutation_id: u64,
        coordinator_key: String,
        name: String,
        origin: crate::app::state::queue_owner::QueueOrigin,
        source_playlist_id: Option<String>,
        result: Result<String, mbv_emby::EmbyError>,
    },
    Error(String),
}
