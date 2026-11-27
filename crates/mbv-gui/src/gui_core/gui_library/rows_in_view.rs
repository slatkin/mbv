//! The Library rows in view (gui-delineated-rows design D7): the stable row ids
//! the window last painted, the row art source each id names, and the `Row`
//! fetches the Client core queues for them once navigation is idle.
//!
//! The window knows which rows it paints and the core knows which image each
//! row needs, so one command (`GuiLibraryCommand::RowsInView`) carries the ids
//! and no coordinates. A row that leaves the view before its fetch starts is
//! dropped by the cache's replaced row queue (design D7), and an interrupted
//! key retries only while its row is still in the stored set (invariant 27).
//!
//! The stored set and the queueing step are GUI-only, so they are `impl
//! GuiCore` entries; the row source lookups read the `App` alone, so they stay
//! free functions the stores seam and the projections also call (design D4,
//! task 7.4).

use mbv_emby_model::EmbyItem;
use mbv_images::ImageFetchReq;
use mbv_queue::QueueItem;

use super::GuiRowArtScope;
use super::artwork::{
    ARTIST_NAME_ROW_ID_PREFIX, GuiRowSource, artist_id_row_source, artist_row_source,
    audiobookshelf_updated_at_ms, catalog_book_row_source, emby_row_source, episode_row_source,
    feed_row_source, queue_item_row_source, row_fetch_req, upcoming_row_source as upcoming_source,
};
use super::feeds_projection::{GuiFeedEntryId, feed_entry_matches};
use super::home::GuiHomeRowId;
use super::podcasts_projection::GuiPodcastEpisodeId;
use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;
use mbv_client_core::dispatch::library::artist_lookup::ArtistNameLookup;

impl GuiCore {
    /// Store one `RowsInView` set and queue its fetches once navigation is idle
    /// (design D7): the set replaces the scope's previous one and the waiting
    /// row fetches with it, the navigation clock closes the idle gate as a
    /// scroll does, and the idle pass — here or on the deadline mark — resolves
    /// each row's source.
    pub(crate) fn apply_gui_rows_in_view(&mut self, scope: GuiRowArtScope, ids: Vec<String>) {
        let app = &mut self.app;
        let gui = &mut self.gui;
        touch_in_view_row_images(app, gui, scope, &ids);
        gui.rows_in_view.insert(scope, ids);
        app.stamp_navigation();
        app.drop_pending_row_fetches();
        queue_in_view_row_fetches(app, gui);
    }

    /// Queue a `Row` fetch for every in-view row of the active list (design D7):
    /// the idle pass and the interrupted-key retry (invariant 27) both run it.
    /// Nothing queues while navigation is in flight, and a row whose artwork
    /// policy declares no source makes no request.
    pub fn gui_queue_in_view_row_fetches(&mut self) {
        queue_in_view_row_fetches(&mut self.app, &mut self.gui);
    }
}

/// Queue a `Row` fetch for every in-view row of the active list (design D7).
pub(crate) fn queue_in_view_row_fetches(app: &mut App, gui: &mut GuiClientState) {
    if !app.navigation_idle() || !app.image_cache().protocol_enabled() {
        return;
    }
    let Some(scope) = row_art_scope(app, gui) else {
        return;
    };
    let Some(ids) = gui.rows_in_view.get(&scope).cloned() else {
        return;
    };
    if scope == GuiRowArtScope::Music {
        for name in ids
            .iter()
            .filter_map(|id| id.strip_prefix(ARTIST_NAME_ROW_ID_PREFIX))
        {
            ArtistNameLookup::request(app, name);
        }
    }
    let requests: Vec<ImageFetchReq> = ids
        .iter()
        .filter_map(|id| row_source(app, gui, scope, id))
        .filter_map(|source| row_fetch_req(app, &source))
        .collect();
    app.fetch_row_images(requests);
}

/// Mark the cached image of every in-view row as read, so the row cache
/// evicts rows the window scrolled past before rows it still paints. It
/// runs on every set, ahead of the idle gate, because a scroll is the
/// moment the cache fills.
fn touch_in_view_row_images(
    app: &mut App,
    gui: &GuiClientState,
    scope: GuiRowArtScope,
    ids: &[String],
) {
    let keys: Vec<String> = ids
        .iter()
        .filter_map(|id| row_source(app, gui, scope, id))
        .filter_map(|source| row_fetch_req(app, &source))
        .map(|req| req.cache_key)
        .collect();
    for key in keys {
        app.touch_row_image(&key);
    }
}

/// The scope whose rows the active destination paints (design D7): the one
/// list that reports rows in view, so a stored set of another list queues
/// nothing.
fn row_art_scope(app: &App, gui: &GuiClientState) -> Option<GuiRowArtScope> {
    match super::tabs::active_destination(app)? {
        super::GuiDestinationKind::Home => Some(GuiRowArtScope::Home),
        super::GuiDestinationKind::Music { .. } => Some(GuiRowArtScope::Music),
        super::GuiDestinationKind::Shows { lib_idx } => {
            Some(match super::shows::library_view(gui, lib_idx) {
                super::GuiLibraryView::New => GuiRowArtScope::ShowsNew,
                super::GuiLibraryView::Upcoming => GuiRowArtScope::ShowsUpcoming,
                // The Favourites list's own scope arrives with its list
                // component (emby-favourites design D6, unit 5.1), so the All
                // tree's scope stands in beside All and no fetch names it.
                super::GuiLibraryView::All | super::GuiLibraryView::Favourites => {
                    GuiRowArtScope::ShowsSeries
                }
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
fn row_source(
    app: &App,
    gui: &GuiClientState,
    scope: GuiRowArtScope,
    id: &str,
) -> Option<GuiRowSource> {
    match scope {
        GuiRowArtScope::Home => home_row_source(app, id),
        GuiRowArtScope::Emby => {
            let lib_idx = active_library_index(app)?;
            emby_row_source(&super::emby_projection::emby_item(app, gui, lib_idx, id)?)
        }
        GuiRowArtScope::Music => music_row_source(app, id),
        GuiRowArtScope::ShowsSeries => emby_row_source(&shows_series_item(app, id)?),
        GuiRowArtScope::ShowsNew => {
            let lib_idx = active_shows_library_index(app)?;
            emby_row_source(&super::shows_projection::shows_item(app, gui, lib_idx, id)?)
        }
        GuiRowArtScope::ShowsUpcoming => upcoming_row_source(app, id),
        GuiRowArtScope::Podcasts => podcast_row_source(app, id),
        GuiRowArtScope::Audiobooks => {
            let lib_idx = super::audiobooks_projection::audiobooks_destination(app)?;
            let state = app.audiobookshelf_book_browse_state(lib_idx)?;
            let book = state.books.iter().find(|book| book.library_item_id == id)?;
            catalog_book_row_source(book)
        }
        GuiRowArtScope::Feeds => {
            let entry = GuiFeedEntryId::decode(id)?;
            let entry = app
                .feeds_state()
                .all_entries
                .iter()
                .find(|candidate| feed_entry_matches(candidate, &entry))?;
            feed_row_source(entry)
        }
    }
}

/// The row art source of a Music artist row (design D5): a Service artist's
/// own id, else the artist item its name resolves to.
fn music_row_source(app: &App, id: &str) -> Option<GuiRowSource> {
    artist_id_row_source(id).or_else(|| {
        let name = id.strip_prefix(ARTIST_NAME_ROW_ID_PREFIX)?;
        ArtistNameLookup::item_id(app, name).map(artist_row_source)
    })
}

/// The row art source of a Shows Upcoming row: the premiere's series image.
fn upcoming_row_source(app: &App, id: &str) -> Option<GuiRowSource> {
    let premiere = app
        .tv_upcoming_premieres()
        .find(|premiere| premiere.stable_key() == id)?;
    let series = shows_series_item(app, premiere.series_id.as_str());
    Some(upcoming_source(premiere, series.as_ref()))
}

/// The row art source of a Podcasts episode row: the show's cover.
fn podcast_row_source(app: &App, id: &str) -> Option<GuiRowSource> {
    let lib_idx = active_podcast_library_index(app)?;
    let episode = GuiPodcastEpisodeId::decode(id)?;
    let item = super::podcasts_projection::podcast_hero_item(app, lib_idx, &episode)?;
    let updated_at = audiobookshelf_updated_at_ms(app, &item.library_item_id);
    episode_row_source(&item, updated_at)
}

/// The row art source of a Home row (design D5): the row's item through its
/// provider's artwork policy. The Home ids are the frame's own stable row
/// ids, so a stale set resolves nothing.
fn home_row_source(app: &App, id: &str) -> Option<GuiRowSource> {
    let item = home_row_item(app, id)?;
    queue_item_row_source(app, &item)
}

/// The Home row's item, as the projection builds it (design D12).
fn home_row_item(app: &App, id: &str) -> Option<QueueItem> {
    let continued = app
        .home_content()
        .continue_items
        .iter()
        .find(|item| GuiHomeRowId::Continue(item.id.clone()).rows_in_view_id() == id);
    if let Some(item) = continued {
        return Some(QueueItem::Emby(Box::new(item.clone())));
    }
    let now_secs = mbv_ui_model::feed_age::current_time_secs();
    app.home_recent_rows(now_secs)
        .into_iter()
        .find(|row| GuiHomeRowId::Recent(row.item.content_id()).rows_in_view_id() == id)
        .map(|row| row.item)
}

/// The Shows root list's series item `series_id` names, from any library's
/// root level: the declared tags a series row and an Upcoming row's series
/// image are addressed by.
pub(in crate::gui_core::gui_library) fn shows_series_item(
    app: &App,
    series_id: &str,
) -> Option<EmbyItem> {
    app.emby_libraries()
        .iter()
        .filter_map(|lib| lib.nav_stack.last())
        .flat_map(|level| level.items.iter())
        .find(|item| item.id == series_id && item.item_type == "Series")
        .cloned()
}

/// The active plain Emby library's index (design D7).
fn active_library_index(app: &App) -> Option<usize> {
    match super::tabs::active_destination(app) {
        Some(super::GuiDestinationKind::Library { lib_idx }) => Some(lib_idx),
        _ => None,
    }
}

/// The active Shows library's index (design D7).
fn active_shows_library_index(app: &App) -> Option<usize> {
    match super::tabs::active_destination(app) {
        Some(super::GuiDestinationKind::Shows { lib_idx }) => Some(lib_idx),
        _ => None,
    }
}

/// The active Podcasts library's index (design D7).
fn active_podcast_library_index(app: &App) -> Option<usize> {
    match super::tabs::active_destination(app) {
        Some(super::GuiDestinationKind::Podcasts { lib_idx }) => Some(lib_idx),
        _ => None,
    }
}
