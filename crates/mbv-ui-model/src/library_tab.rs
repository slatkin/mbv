use crate::browse::BrowseLevel;
use crate::feed::FeedHomeVideoState;
use mbv_emby_model::EmbyItem;

#[derive(Debug)]
pub struct LibraryTab {
    pub library: EmbyItem,
    pub nav_stack: Vec<BrowseLevel>,
    pub feed_home_video: Option<FeedHomeVideoState>,
    /// The library's TRUE unfiltered `TotalRecordCount`, captured from the
    /// first unfiltered fetch of the library's top level. `None` until that
    /// first load completes. Used to gate the letter pill row and per-letter
    /// header grouping so a scoped (small) fetch doesn't look "small" to the
    /// UI. See `LIBRARY_PILL_THRESHOLD`.
    pub library_total: Option<usize>,
    /// Selected top-level TV content mode. `None` is unresolved until the
    /// library total is known; non-TV libraries leave this unset.
    pub tv_content_mode: Option<mbv_queue::TvContentMode>,
}

impl LibraryTab {
    #[must_use]
    pub fn new(library: EmbyItem) -> Self {
        Self {
            library,
            nav_stack: Vec::new(),
            feed_home_video: None,
            library_total: None,
            tv_content_mode: None,
        }
    }

    pub fn library_position_snapshot(&self) -> mbv_queue::LibraryPosition {
        let (feed_selected_group, feed_video_cursor, feed_video_scroll) =
            self.feed_home_video.as_ref().map_or((0, 0, 0), |state| {
                (state.selected_group, state.video_cursor, state.video_scroll)
            });
        let mut levels: Vec<mbv_queue::LibraryPositionLevel> = self
            .nav_stack
            .iter()
            .map(BrowseLevel::to_position_level)
            .collect();
        // The true unfiltered library total only applies to the top (root)
        // level; stash it there so a restored session can gate the letter
        // pill row without an extra unfiltered fetch (see `library_total`).
        if let Some(root) = levels.first_mut() {
            root.library_total = self.library_total;
            root.tv_content_mode.clone_from(&self.tv_content_mode);
        }
        mbv_queue::LibraryPosition {
            levels,
            feed_selected_group,
            feed_video_cursor,
            feed_video_scroll,
        }
    }

    pub fn apply_library_position(
        &mut self,
        position: &mbv_queue::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    ) {
        self.library_total = position.levels.first().and_then(|l| l.library_total);
        self.tv_content_mode = position
            .levels
            .first()
            .and_then(|level| level.tv_content_mode.clone());
        self.nav_stack = nav_stack;
        if let Some(state) = self.feed_home_video.as_mut() {
            state.selected_group = position.feed_selected_group;
            state.video_cursor = position.feed_video_cursor;
            state.video_scroll = position.feed_video_scroll;
        }
    }
}
