//! The tab and catalog facts a Client reads: which tab is active, and the
//! Emby, Audiobookshelf, and Feeds state behind each tab.

use crate::app::App;
use mbv_ui_model::audiobookshelf_browse::{
    AudiobookshelfBookBrowseState, AudiobookshelfBrowseState,
};
use mbv_ui_model::feed_tab::FeedTabState;
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::tab_selection::TabSelection;

impl App {
    /// The active left-panel tab.
    #[must_use]
    pub fn active_tab(&self) -> TabSelection {
        self.tab
    }

    /// The Emby library index the active tab shows, if the active tab is an
    /// Emby library.
    #[must_use]
    pub fn active_emby_library_index(&self) -> Option<usize> {
        self.tab.emby_library_index()
    }

    /// The Audiobookshelf library index the active tab shows, if the active
    /// tab is an Audiobookshelf library.
    #[must_use]
    pub fn active_audiobookshelf_library_index(&self) -> Option<usize> {
        self.tab.audiobookshelf_index()
    }

    /// The Emby library tabs, in tab order.
    #[must_use]
    pub fn emby_libraries(&self) -> &[LibraryTab] {
        &self.libs
    }

    /// The one Emby library tab at `index`.
    #[must_use]
    pub fn emby_library(&self, index: usize) -> Option<&LibraryTab> {
        self.libs.get(index)
    }

    /// The name the Emby library at `index` shows.
    #[must_use]
    pub fn emby_library_name(&self, index: usize) -> Option<&str> {
        self.libs.get(index).map(|tab| tab.library.name.as_str())
    }

    /// The tab index of the Emby library whose Service id is `library_id`.
    #[must_use]
    pub fn emby_library_index_of(&self, library_id: &str) -> Option<usize> {
        self.libs
            .iter()
            .position(|tab| tab.library.id == library_id)
    }

    /// The TV Latest root the restored position landed on, as the Latest strip
    /// captures it: the library id and its root rows, when the library is a TV
    /// library sitting one level deep in its Latest mode.
    #[must_use]
    pub fn tv_latest_root_after_restore(
        &self,
        index: usize,
    ) -> Option<(String, Vec<mbv_queue::QueueItem>)> {
        let tab = self.libs.get(index)?;
        let level = tab.nav_stack.last()?;
        let is_latest_root = tab.library.collection_type == "tvshows"
            && tab.nav_stack.len() == 1
            && tab.tv_content_mode == Some(mbv_queue::TvContentMode::Latest);
        is_latest_root.then(|| {
            (
                tab.library.id.clone(),
                level
                    .items
                    .iter()
                    .cloned()
                    .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                    .collect(),
            )
        })
    }

    /// The browse level the Emby library at `index` currently shows, or `None`
    /// before the library opens.
    #[must_use]
    pub fn emby_library_level(&self, index: usize) -> Option<&mbv_ui_model::browse::BrowseLevel> {
        self.libs.get(index)?.nav_stack.last()
    }

    /// The feed/home-video group state of the Emby library at `index`.
    #[must_use]
    pub fn emby_library_feed_home_video(
        &self,
        index: usize,
    ) -> Option<&mbv_ui_model::feed::FeedHomeVideoState> {
        self.libs.get(index)?.feed_home_video.as_ref()
    }

    /// The selected TV content mode of the Emby library at `index`: the level's
    /// own choice first, then the library's last chosen mode.
    #[must_use]
    pub fn emby_library_tv_content_mode(&self, index: usize) -> Option<&mbv_queue::TvContentMode> {
        let tab = self.libs.get(index)?;
        tab.nav_stack
            .last()
            .and_then(|level| level.tv_content_mode.as_ref())
            .or(tab.tv_content_mode.as_ref())
    }

    /// Whether `row` addresses a row of the Emby library's current feed/home
    /// video group or browse level.
    #[must_use]
    pub fn emby_library_row_in_range(&self, index: usize, row: usize) -> bool {
        let Some(tab) = self.libs.get(index) else {
            return false;
        };
        if self.is_feed_home_video_group_view(index) {
            return tab
                .feed_home_video
                .as_ref()
                .is_some_and(|state| row < state.selected_len());
        }
        tab.nav_stack
            .last()
            .is_some_and(|level| row < level.items.len())
    }

    /// Whether the Emby library at `index` exists.
    #[must_use]
    pub fn emby_library_exists(&self, index: usize) -> bool {
        index < self.libs.len()
    }

    /// How many Emby library tabs the catalog holds.
    #[must_use]
    pub fn emby_library_count(&self) -> usize {
        self.libs.len()
    }

    /// The Audiobookshelf libraries, in tab order.
    #[must_use]
    pub fn audiobookshelf_libraries(&self) -> &[mbv_audiobookshelf::AudiobookshelfLibrary] {
        &self.audiobookshelf_libraries
    }

    /// The one Audiobookshelf library at `index`.
    #[must_use]
    pub fn audiobookshelf_library(
        &self,
        index: usize,
    ) -> Option<&mbv_audiobookshelf::AudiobookshelfLibrary> {
        self.audiobookshelf_libraries.get(index)
    }

    /// The Audiobookshelf browse state of the library at `index`.
    #[must_use]
    pub fn audiobookshelf_browse_state(&self, index: usize) -> Option<&AudiobookshelfBrowseState> {
        self.audiobookshelf_browse.get(index)
    }

    /// The Audiobookshelf book browse state of the library at `index`.
    #[must_use]
    pub fn audiobookshelf_book_browse_state(
        &self,
        index: usize,
    ) -> Option<&AudiobookshelfBookBrowseState> {
        self.audiobookshelf_book_browse.get(index)
    }

    /// The Feeds tab state: subscriptions and their entries.
    #[must_use]
    pub fn feeds_state(&self) -> &FeedTabState {
        &self.feed_tab
    }
}
