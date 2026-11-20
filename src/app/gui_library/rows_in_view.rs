//! The Library rows in view (gui-delineated-rows design D7): the stable row ids
//! the window last painted, the row art source each id names, and the `Row`
//! fetches the Client core queues for them once navigation is idle.
//!
//! The window knows which rows it paints and the core knows which image each
//! row needs, so one command (`GuiLibraryCommand::RowsInView`) carries the ids
//! and no coordinates. A row that leaves the view before its fetch starts is
//! dropped by the cache's replaced row queue (design D7), and an interrupted
//! key retries only while its row is still in the stored set (invariant 27).

use mbv_emby_model::EmbyItem;
use mbv_images::ImageFetchReq;
use mbv_queue::QueueItem;

use super::GuiRowArtScope;
use super::artwork::{
    GuiRowSource, artist_id_row_source, catalog_book_row_source, emby_row_source,
    episode_row_source, feed_row_source, upcoming_row_source,
};
use super::feeds_projection::{GuiFeedEntryId, feed_entry_matches};
use super::home::GuiHomeRowId;
use super::podcasts_projection::GuiPodcastEpisodeId;
use crate::app::App;

impl App {
    /// Store one `RowsInView` set and queue its fetches once navigation is idle
    /// (design D7): the set replaces the scope's previous one and the waiting
    /// row fetches with it, the navigation clock closes the idle gate as a
    /// scroll does, and the idle pass — here or on the deadline mark — resolves
    /// each row's source.
    pub(in crate::app) fn apply_gui_rows_in_view(
        &mut self,
        scope: GuiRowArtScope,
        ids: Vec<String>,
    ) {
        self.gui_rows_in_view.insert(scope, ids);
        self.stamp_navigation();
        self.drop_pending_row_fetches();
        self.gui_queue_in_view_row_fetches();
    }

    /// Queue a `Row` fetch for every in-view row of the active list (design D7):
    /// the idle pass and the interrupted-key retry (invariant 27) both run it.
    /// Nothing queues while navigation is in flight, and a row whose artwork
    /// policy declares no source makes no request.
    pub fn gui_queue_in_view_row_fetches(&mut self) {
        if !self.navigation_idle() || !self.images.images_enabled() {
            return;
        }
        let Some(scope) = self.gui_row_art_scope() else {
            return;
        };
        let Some(ids) = self.gui_rows_in_view.get(&scope).cloned() else {
            return;
        };
        let requests: Vec<ImageFetchReq> = ids
            .iter()
            .filter_map(|id| self.gui_row_source(scope, id))
            .filter_map(|source| self.gui_row_fetch_req(&source))
            .collect();
        self.fetch_row_images(requests);
    }

    /// The scope whose rows the active destination paints (design D7): the one
    /// list that reports rows in view, so a stored set of another list queues
    /// nothing.
    fn gui_row_art_scope(&self) -> Option<GuiRowArtScope> {
        match self.gui_active_destination()? {
            super::GuiDestinationKind::Home => Some(GuiRowArtScope::Home),
            super::GuiDestinationKind::Music { .. } => Some(GuiRowArtScope::Music),
            super::GuiDestinationKind::Shows { lib_idx } => {
                Some(match self.gui_shows_view(lib_idx) {
                    super::GuiShowsView::All => GuiRowArtScope::ShowsSeries,
                    super::GuiShowsView::New => GuiRowArtScope::ShowsNew,
                    super::GuiShowsView::Upcoming => GuiRowArtScope::ShowsUpcoming,
                })
            }
            super::GuiDestinationKind::Library { .. } => Some(GuiRowArtScope::Emby),
            super::GuiDestinationKind::Audiobooks { .. } => Some(GuiRowArtScope::Audiobooks),
            super::GuiDestinationKind::Podcasts { .. } => Some(GuiRowArtScope::Podcasts),
            super::GuiDestinationKind::Feeds => Some(GuiRowArtScope::Feeds),
        }
    }

    /// The row art source an in-view row's stable id names in `scope` (design
    /// D5): the App's own data resolves the id, so the window sends no source.
    fn gui_row_source(&self, scope: GuiRowArtScope, id: &str) -> Option<GuiRowSource> {
        match scope {
            GuiRowArtScope::Home => self.gui_home_row_source(id),
            GuiRowArtScope::Emby => {
                let lib_idx = self.gui_active_library_index()?;
                emby_row_source(&self.gui_emby_item(lib_idx, id)?)
            }
            GuiRowArtScope::Music => artist_id_row_source(id),
            GuiRowArtScope::ShowsSeries => emby_row_source(&self.gui_shows_series_item(id)?),
            GuiRowArtScope::ShowsNew => emby_row_source(&self.gui_shows_new_item(id)?),
            GuiRowArtScope::ShowsUpcoming => {
                let premiere = self
                    .tv_upcoming_snapshots
                    .values()
                    .flatten()
                    .find(|premiere| premiere.stable_key() == id)?;
                let series = self.gui_shows_series_item(premiere.series_id.as_str());
                Some(upcoming_row_source(premiere, series.as_ref()))
            }
            GuiRowArtScope::Podcasts => {
                let lib_idx = self.gui_active_podcast_library_index()?;
                let episode = GuiPodcastEpisodeId::decode(id)?;
                let item = self.gui_podcast_hero_item(lib_idx, &episode)?;
                let updated_at = self.audiobookshelf_updated_at_ms(&item.library_item_id);
                episode_row_source(&item, updated_at)
            }
            GuiRowArtScope::Audiobooks => {
                let lib_idx = self.gui_active_book_library_index()?;
                let state = self.audiobookshelf_book_browse.get(lib_idx)?;
                let book = state.books.iter().find(|book| book.library_item_id == id)?;
                catalog_book_row_source(book)
            }
            GuiRowArtScope::Feeds => {
                let entry = GuiFeedEntryId::decode(id)?;
                let entry = self
                    .feed_tab
                    .all_entries
                    .iter()
                    .find(|candidate| feed_entry_matches(candidate, &entry))?;
                feed_row_source(entry)
            }
        }
    }

    /// The row art source of a Home row (design D5): the row's item through its
    /// provider's artwork policy. The Home ids are the frame's own stable row
    /// ids, so a stale set resolves nothing.
    fn gui_home_row_source(&self, id: &str) -> Option<GuiRowSource> {
        let item = self.gui_home_row_item(id)?;
        self.queue_item_row_source(&item)
    }

    /// The Home row's item, as the projection builds it (design D12).
    fn gui_home_row_item(&self, id: &str) -> Option<QueueItem> {
        let continued = self
            .home_content
            .continue_items
            .iter()
            .find(|item| GuiHomeRowId::Continue(item.id.clone()).rows_in_view_id() == id);
        if let Some(item) = continued {
            return Some(QueueItem::Emby(Box::new(item.clone())));
        }
        let now_secs = mbv_render::screens::feeds_model::current_time_secs();
        self.home_recent_rows(now_secs)
            .into_iter()
            .find(|row| GuiHomeRowId::Recent(row.item.content_id()).rows_in_view_id() == id)
            .map(|row| row.item)
    }

    /// The Shows root list's series item `series_id` names, from any library's
    /// root level: the declared tags a series row and an Upcoming row's series
    /// image are addressed by.
    pub(in crate::app::gui_library) fn gui_shows_series_item(
        &self,
        series_id: &str,
    ) -> Option<EmbyItem> {
        self.libs
            .iter()
            .filter_map(|lib| lib.nav_stack.last())
            .flat_map(|level| level.items.iter())
            .find(|item| item.id == series_id && item.item_type == "Series")
            .cloned()
    }

    /// The active plain Emby library's index (design D7).
    fn gui_active_library_index(&self) -> Option<usize> {
        match self.gui_active_destination() {
            Some(super::GuiDestinationKind::Library { lib_idx }) => Some(lib_idx),
            _ => None,
        }
    }

    /// The active Podcasts library's index (design D7).
    fn gui_active_podcast_library_index(&self) -> Option<usize> {
        match self.gui_active_destination() {
            Some(super::GuiDestinationKind::Podcasts { lib_idx }) => Some(lib_idx),
            _ => None,
        }
    }

    /// The active Audiobooks library's index (design D7).
    fn gui_active_book_library_index(&self) -> Option<usize> {
        match self.gui_active_destination() {
            Some(super::GuiDestinationKind::Audiobooks { lib_idx }) => Some(lib_idx),
            _ => None,
        }
    }
}
