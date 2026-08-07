use super::types_browse::BrowseLevel;
use super::types_browse::LibSearch;
use super::types_feed::FeedHomeVideoState;
use super::types_playback::ArtistHeaderSelection;
use mbv_core::api::MediaItem;

pub(super) struct LibraryTab {
    pub(super) library: MediaItem,
    pub(super) nav_stack: Vec<BrowseLevel>,
    pub(super) search: Option<LibSearch>,
    pub(super) feed_home_video: Option<FeedHomeVideoState>,
    /// `Some(idx)` = track-selection mode is active for the album currently
    /// shown inline at the album-folder-listing nav level (#145 task 3);
    /// `idx` indexes into that album's cached track list
    /// (`App::album_tracks_cache`). `None` = normal album-list navigation.
    pub(super) album_track_focus: Option<usize>,
    pub(super) artist_header_focus: Option<ArtistHeaderSelection>,
    /// `Some(ep_idx)` = series-selection mode is active for the Series item
    /// currently shown inline at the library list nav level;
    /// `ep_idx` indexes into the cached episode list for the current season.
    /// `None` = normal list navigation.
    pub(super) series_selection: Option<usize>,
    /// Which season is selected in series-selection mode (index into
    /// `SeriesDetail.seasons`). Only meaningful when `series_selection.is_some()`.
    pub(super) series_season_cursor: usize,
    /// The library's TRUE unfiltered `TotalRecordCount`, captured from the
    /// first unfiltered fetch of the library's top level. `None` until that
    /// first load completes. Used to gate the letter pill row and per-letter
    /// header grouping so a scoped (small) fetch doesn't look "small" to the
    /// UI. See `LIBRARY_PILL_THRESHOLD`.
    pub(super) library_total: Option<usize>,
}

impl LibraryTab {
    pub(super) fn clear_music_focus(&mut self) {
        self.album_track_focus = None;
        self.artist_header_focus = None;
    }

    pub(super) fn library_position_snapshot(&self) -> crate::config::LibraryPosition {
        let (feed_selected_group, feed_video_cursor, feed_video_scroll) = self
            .feed_home_video
            .as_ref()
            .map(|state| (state.selected_group, state.video_cursor, state.video_scroll))
            .unwrap_or((0, 0, 0));
        let mut levels: Vec<crate::config::LibraryPositionLevel> = self
            .nav_stack
            .iter()
            .map(BrowseLevel::to_position_level)
            .collect();
        // The true unfiltered library total only applies to the top (root)
        // level; stash it there so a restored session can gate the letter
        // pill row without an extra unfiltered fetch (see `library_total`).
        if let Some(root) = levels.first_mut() {
            root.library_total = self.library_total;
        }
        crate::config::LibraryPosition {
            levels,
            feed_selected_group,
            feed_video_cursor,
            feed_video_scroll,
        }
    }

    pub(super) fn apply_library_position(
        &mut self,
        position: crate::config::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    ) {
        self.library_total = position.levels.first().and_then(|l| l.library_total);
        self.nav_stack = nav_stack;
        self.search = None;
        self.clear_music_focus();
        if let Some(state) = self.feed_home_video.as_mut() {
            state.selected_group = position.feed_selected_group;
            state.video_cursor = position.feed_video_cursor;
            state.video_scroll = position.feed_video_scroll;
        }
    }
}
