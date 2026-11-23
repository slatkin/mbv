//! The tab and catalog facts a Client reads: which tab is active, and the
//! Emby, Audiobookshelf, and Feeds state behind each tab.

use crate::app::App;
use mbv_ui_model::audiobookshelf_browse::{
    AudiobookshelfBookBrowseState, AudiobookshelfBrowseState,
};
use mbv_ui_model::feed_tab::FeedTabState;
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::tab_selection::TabSelection;

/// The browse identity of one Emby library's open level: the facts a
/// destination owner is re-seeded from. `level` is `None` before the library
/// opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowseLevelFacts {
    /// How many levels deep the library's nav stack is.
    pub depth: usize,
    /// The open level's browse facts, while one is open.
    pub level: Option<BrowseLevelIdentity>,
    /// Whether the library holds a feed/home-video group state.
    pub has_feed_home_video: bool,
}

/// The browse facts of one open level, as a destination owner is re-seeded
/// from them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowseLevelIdentity {
    pub parent_id: String,
    pub letter_filter: Option<usize>,
    pub sort_by: String,
    pub sort_order: String,
    pub unplayed_only: bool,
    /// Whether the level paints as loading.
    pub loading: bool,
    /// The level's own TV content mode, if it chose one.
    pub tv_content_mode: Option<mbv_queue::TvContentMode>,
}

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

    /// The Service id of the Emby library at `index`.
    #[must_use]
    pub fn emby_library_id(&self, index: usize) -> Option<&str> {
        self.libs.get(index).map(|tab| tab.library.id.as_str())
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

    /// The Emby library's root level at `index`, or `None` before the library
    /// opens its first level. The root is the level a library's folder catalog
    /// reads.
    #[must_use]
    pub fn emby_library_root_level(
        &self,
        index: usize,
    ) -> Option<&mbv_ui_model::browse::BrowseLevel> {
        self.libs.get(index)?.nav_stack.first()
    }

    /// The TV content mode the Emby library at `index` last chose, whatever its
    /// open level chose since.
    #[must_use]
    pub fn emby_library_stored_tv_mode(&self, index: usize) -> Option<&mbv_queue::TvContentMode> {
        self.libs.get(index)?.tv_content_mode.as_ref()
    }

    /// The identifier of the item the Emby library at `index` rests its cursor
    /// on in its open level: the activated album after a wholesale nav-stack
    /// replacement, else the row the persistence holds.
    #[must_use]
    pub fn emby_library_resting_item_id(&self, index: usize) -> Option<String> {
        let level = self.emby_library_level(index)?;
        level
            .items
            .get(level.resting().cursor())
            .map(|item| item.id.clone())
    }

    /// Every Emby library tab's index, Service id, and collection type, in tab
    /// order: the identity a navigation target resolves against, without the
    /// tab's own state travelling with it.
    #[must_use]
    pub fn emby_library_identities(&self) -> Vec<(usize, String, String)> {
        self.libs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                (
                    index,
                    tab.library.id.clone(),
                    tab.library.collection_type.clone(),
                )
            })
            .collect()
    }

    /// The collection type the Emby library at `index` serves, or `None` when no
    /// tab sits there.
    #[must_use]
    pub fn emby_library_collection_type(&self, index: usize) -> Option<&str> {
        self.libs
            .get(index)
            .map(|tab| tab.library.collection_type.as_str())
    }

    /// A copy of the series detail cached for `series_id`, if its cards are
    /// fetched.
    #[must_use]
    pub fn series_detail_snapshot(
        &self,
        series_id: &str,
    ) -> Option<mbv_ui_model::browse::SeriesDetail> {
        self.series_detail_cache.get(series_id).cloned()
    }

    /// The browse identity of the Emby library at `index`: how deep its nav
    /// stack is, the open level's browse facts, and whether it holds a
    /// feed/home-video group state. `None` when no tab sits there.
    #[must_use]
    pub fn emby_library_level_facts(&self, index: usize) -> Option<BrowseLevelFacts> {
        let tab = self.libs.get(index)?;
        let level = tab.nav_stack.last().map(|level| BrowseLevelIdentity {
            parent_id: level.parent_id.clone(),
            letter_filter: level.letter_filter.as_ref().map(|filter| filter.index),
            sort_by: level.sort_by.clone(),
            sort_order: level.sort_order.clone(),
            unplayed_only: level.unplayed_only,
            loading: level.loading,
            tv_content_mode: level.tv_content_mode.clone(),
        });
        Some(BrowseLevelFacts {
            depth: tab.nav_stack.len(),
            level,
            has_feed_home_video: tab.feed_home_video.is_some(),
        })
    }

    /// Whether the root level of the Emby library at `index` paints as
    /// loading, with no root answering `false`.
    #[must_use]
    pub fn emby_library_root_loading(&self, index: usize) -> bool {
        self.libs
            .get(index)
            .and_then(|tab| tab.nav_stack.first())
            .is_some_and(|root| root.loading)
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

    /// Every retained Audiobookshelf podcast browse state, one per stored
    /// library.
    #[must_use]
    pub fn audiobookshelf_browse_states(&self) -> &[AudiobookshelfBrowseState] {
        &self.audiobookshelf_browse
    }

    /// The Audiobookshelf book browse state of the library at `index`.
    #[must_use]
    pub fn audiobookshelf_book_browse_state(
        &self,
        index: usize,
    ) -> Option<&AudiobookshelfBookBrowseState> {
        self.audiobookshelf_book_browse.get(index)
    }

    /// Every retained Audiobookshelf book browse state, one per stored
    /// library.
    #[must_use]
    pub fn audiobookshelf_book_states(&self) -> &[AudiobookshelfBookBrowseState] {
        &self.audiobookshelf_book_browse
    }

    /// The Feeds tab state: subscriptions and their entries.
    #[must_use]
    pub fn feeds_state(&self) -> &FeedTabState {
        &self.feed_tab
    }

    /// The local queue's projected view, as a UI-state reset leaves it: a
    /// Client's test asserts what the reset kept and what it rewrote through
    /// the same view the Client projects.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn local_queue_view_for_test(&self) -> &crate::app::QueueView {
        &self.local_view
    }

    /// The resting cursor of `level`'s browse level in `index`'s Emby
    /// library: the position the shell's own browse state holds, which the
    /// panel's owner selection is re-pointed from. `None` for a library or a
    /// level that is not open.
    ///
    /// A level-indexed read has no production caller, so it stays a test
    /// seam (design D4b rule 5).
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn emby_library_level_cursor_for_test(&self, index: usize, level: usize) -> Option<usize> {
        Some(
            self.libs
                .get(index)?
                .nav_stack
                .get(level)?
                .resting()
                .cursor(),
        )
    }

    /// The retained Audiobookshelf book states, one per stored library, as a
    /// landed catalog leaves them.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn audiobookshelf_book_states_for_test(&self) -> &[AudiobookshelfBookBrowseState] {
        &self.audiobookshelf_book_browse
    }

    /// The overlay request the core holds for its Client to mount, as a
    /// Client's own action left it: whether one is pending, and which.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_overlay_request_for_test(
        &self,
    ) -> Option<&mbv_ui_model::overlay::OverlayRequest> {
        self.pending_overlay.as_ref()
    }

    /// The retained Audiobookshelf browse states, one per stored library, as
    /// a UI-state reset leaves them.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn audiobookshelf_browse_states_for_test(
        &self,
    ) -> &[mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState] {
        &self.audiobookshelf_browse
    }
}
