use super::types_browse::{AlbumSearchEntry, BrowseLevel};
use super::types_feed::FeedHomeVideoGroup;
use super::types_playback::{HomeContent, HomeLatestSource};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;

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
        nav_stack: Vec<BrowseLevel>,
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

pub(super) struct ReconciliationCommand {
    pub(super) session_id: String,
    pub(super) tracking_id: u64,
    pub(super) tracker_epoch: u64,
    pub(super) generation: u64,
}

pub(super) enum SessionEvent {
    Loaded {
        sessions: Vec<mbv_core::api::SessionInfo>,
        generation: u64,
    },
    ItemRefreshed(String, Box<mbv_core::api::EmbyItem>), // (item_id, fresh)
    CommandError {
        error: String,
        reconciliation: Option<ReconciliationCommand>,
    },
    /// A tracked remote command succeeded on the Emby server, correlated
    /// separately from the follow-up session poll. Emitted even when that
    /// immediate poll fails, so command acknowledgment never freezes tracking.
    CommandAcknowledged(ReconciliationCommand),
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
        source_playlist_id: String,
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
