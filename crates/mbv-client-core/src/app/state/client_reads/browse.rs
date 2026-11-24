//! The Emby-library and Audiobookshelf catalog facts only the GUI Client
//! reads: a library's Service id and stored TV mode, its root level, and the
//! retained Audiobookshelf browse states.
//!
//! The tab and library facts both Clients read stay in `shell_reads::browse`;
//! a fact here that gains a TUI reader moves there.

use crate::app::App;
use mbv_ui_model::audiobookshelf_browse::{
    AudiobookshelfBookBrowseState, AudiobookshelfBrowseState,
};

impl App {
    /// The Service id of the Emby library at `index`.
    #[must_use]
    pub fn emby_library_id(&self, index: usize) -> Option<&str> {
        self.libs.get(index).map(|tab| tab.library.id.as_str())
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

    /// Every retained Audiobookshelf podcast browse state, one per stored
    /// library.
    #[must_use]
    pub fn audiobookshelf_browse_states(&self) -> &[AudiobookshelfBrowseState] {
        &self.audiobookshelf_browse
    }

    /// Every retained Audiobookshelf book browse state, one per stored
    /// library.
    #[must_use]
    pub fn audiobookshelf_book_states(&self) -> &[AudiobookshelfBookBrowseState] {
        &self.audiobookshelf_book_browse
    }

    /// Whether an Audiobookshelf catalog fetch is in flight: the libraries list
    /// is not final until it lands.
    #[must_use]
    pub fn audiobookshelf_catalog_fetching(&self) -> bool {
        self.setup.audiobookshelf_catalog_rx.is_some()
    }
}
