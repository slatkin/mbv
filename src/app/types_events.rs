use super::types_browse::{AlbumSearchEntry, BrowseLevel};
use super::types_feed::FeedHomeVideoGroup;
use mbv_core::api::MediaItem;

pub(super) enum LibEvent {
    Loaded {
        lib_idx: usize,
        parent_id: String,
        level: Box<BrowseLevel>,
    },
    PageAppended {
        lib_idx: usize,
        parent_id: String,
        items: Vec<MediaItem>,
        total_count: usize,
    },
    Refreshed {
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        items: Vec<MediaItem>,
        total_count: usize,
    },
    SearchItemsLoaded {
        lib_idx: usize,
        parent_id: String,
        items: Vec<MediaItem>,
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
        items: Vec<MediaItem>,
    },
    FeedHomeVideoAggregated {
        lib_idx: usize,
        parent_id: String,
        all_items: Vec<MediaItem>,
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
        tracks: Vec<MediaItem>,
    },
    /// TV series detail (seasons + episodes) fetched proactively for inline
    /// rendering when a Series is selected.
    SeriesDetailFetched {
        series_id: String,
        seasons: Vec<MediaItem>,
        episodes: std::collections::HashMap<String, Vec<MediaItem>>,
    },
    /// Episodes for a specific season fetched when switching seasons in
    /// series-selection mode.
    SeriesSeasonEpisodesFetched {
        series_id: String,
        season_id: String,
        episodes: Vec<MediaItem>,
    },
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
    PlaylistsLoaded(Vec<MediaItem>),
    PlaylistItemsLoaded {
        playlist_id: String,
        items: Vec<MediaItem>,
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
    QueueEnriched { items: Vec<MediaItem> },
    Error(String),
}

pub(super) enum SessionEvent {
    Loaded {
        sessions: Vec<mbv_core::api::SessionInfo>,
        generation: u64,
    },
    ItemRefreshed(String, Box<mbv_core::api::MediaItem>), // (item_id, fresh)
    CommandError(String),
    ConsumeValidated {
        session_id: String,
        epoch: u64,
        occurrence_id: u64,
        playlist_id: String,
        entry_id: String,
        result: Result<(), String>,
    },
    ConsumeOutcome {
        session_id: String,
        epoch: u64,
        occurrence_id: u64,
        result: Result<(), String>,
    },
    Error(String),
}
