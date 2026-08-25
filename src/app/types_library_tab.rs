use super::types_browse::BrowseLevel;
use super::types_feed::FeedHomeVideoState;
use mbv_core::api::EmbyItem;

pub(super) struct LibraryTab {
    pub(super) library: EmbyItem,
    pub(super) nav_stack: Vec<BrowseLevel>,
    pub(super) feed_home_video: Option<FeedHomeVideoState>,
    /// The library's TRUE unfiltered `TotalRecordCount`, captured from the
    /// first unfiltered fetch of the library's top level. `None` until that
    /// first load completes. Used to gate the letter pill row and per-letter
    /// header grouping so a scoped (small) fetch doesn't look "small" to the
    /// UI. See `LIBRARY_PILL_THRESHOLD`.
    pub(super) library_total: Option<usize>,
}

impl LibraryTab {
    pub(super) fn new(library: EmbyItem) -> Self {
        Self {
            library,
            nav_stack: Vec::new(),
            feed_home_video: None,
            library_total: None,
        }
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
        if let Some(state) = self.feed_home_video.as_mut() {
            state.selected_group = position.feed_selected_group;
            state.video_cursor = position.feed_video_cursor;
            state.video_scroll = position.feed_video_scroll;
        }
    }
}
