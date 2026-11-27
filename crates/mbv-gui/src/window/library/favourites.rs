//! The Favourites view's window state (emby-favourites design D6, D7): the
//! rows one destination's favourites list paints, the list's own scroll and
//! filter per library, and the commands its press, play, queue, and menu send.
//!
//! The list is one list for the three Emby destinations that paint it — Music,
//! Shows, and a plain Emby library — because its frame part rides each of their
//! list parts (`GuiFavouritesList`) and its rows are the plain library's own row
//! frame. [`FavouritesSite`] names the destination the carried list belongs to,
//! so the header names the right view dropdown and the filter text and the
//! scroll stay apart per library.
//!
//! The All lists, their selection, and their scroll belong to their own stores
//! and are never edited here (design D7): pressing All finds them as they were.
//! The favourites list keeps its own [`ListScroll`] per library and its own
//! filter text, so neither a scroll nor a filter key of the Favourites view
//! moves a store the All list paints.

use std::collections::BTreeMap;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiDestinationList, GuiEmbyRowFrame, GuiFavouritesList, GuiLibraryList,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::gui_core::GuiDestinationKind;
use crate::window::library::list::{
    GeometryRow, LIBRARY_ROW_HEIGHT, clamp_scroll_top, filter_match, measure_scroll_top,
    scroll_step,
};
use crate::window::library::scroll::ListScroll;

use super::state::LibraryState;

/// One painted row of the Favourites list (design D6): the plain library's row
/// frame under the fixed top-level row height, with its top edge. The list
/// paints no letter heading and no rail, so every row is an item row.
#[derive(Debug, Clone, PartialEq)]
pub struct FavouritesRow {
    /// The row's stable key: the pointer marker's value, the each-block's key,
    /// and the id the scroll geometry reads.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried row the item paints.
    pub row: GuiEmbyRowFrame,
}

impl GeometryRow for FavouritesRow {
    fn id(&self) -> &str {
        &self.id
    }

    fn top(&self) -> f32 {
        self.top
    }

    fn height(&self) -> f32 {
        self.height
    }
}

/// The destination whose list part carries a Favourites list (design D6): the
/// Shows destination or a plain Emby library — a Grouped Music destination is
/// excluded from favourites everywhere (design D11). The site names the
/// library the view dropdown and the list's own stores are keyed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavouritesSite {
    /// The Shows destination's Favourites view.
    Shows(usize),
    /// A plain Emby library's Favourites view.
    Emby(usize),
}

impl FavouritesSite {
    /// The library index the destination's commands name.
    #[must_use]
    pub fn lib_idx(self) -> usize {
        match self {
            Self::Shows(lib_idx) | Self::Emby(lib_idx) => lib_idx,
        }
    }

    /// The destination the view dropdown names (design D5): the carried kind
    /// whose choices the dropdown offers.
    #[must_use]
    pub fn destination(self) -> GuiDestinationKind {
        match self {
            Self::Shows(lib_idx) => GuiDestinationKind::Shows { lib_idx },
            Self::Emby(lib_idx) => GuiDestinationKind::Library { lib_idx },
        }
    }
}

/// The Favourites view's own window state of one library (design D6): the
/// list's scroll and its filter text. Both stay apart from the destination's All
/// stores, so pressing All finds the All list's scroll and filter untouched.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FavouritesView {
    /// The list's scroll offset and the viewport it measured.
    pub scroll: ListScroll,
    /// The filter field's text: it narrows the favourites rows alone.
    pub filter: String,
}

/// The Favourites list part the carried list section holds, with the
/// destination it belongs to (design D6): `None` while the active destination's
/// view is not Favourites.
#[must_use]
pub fn favourites_part(
    list: Option<&GuiLibraryList>,
) -> Option<(FavouritesSite, &GuiFavouritesList)> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Shows(destination)) => destination
            .favourites
            .as_ref()
            .map(|part| (FavouritesSite::Shows(destination.lib_idx), part)),
        Some(GuiDestinationList::Emby(destination)) => destination
            .favourites
            .as_ref()
            .map(|part| (FavouritesSite::Emby(destination.lib_idx), part)),
        Some(
            GuiDestinationList::Music(_)
            | GuiDestinationList::Home(_)
            | GuiDestinationList::Audiobooks(_)
            | GuiDestinationList::Podcasts(_)
            | GuiDestinationList::Feeds(_),
        )
        | None => None,
    }
}

/// The destination painting a Favourites view, or `None` while the active
/// destination paints its own list (design D6).
#[must_use]
pub fn favourites_site(list: Option<&GuiLibraryList>) -> Option<FavouritesSite> {
    favourites_part(list).map(|(site, _)| site)
}

/// The rows the Favourites list paints (design D6): the carried rows narrowed by
/// the view's filter text, each at the shared top-level row height, with no
/// letter heading and no rail.
#[must_use]
pub fn favourites_rows(part: &GuiFavouritesList, filter: &str) -> Vec<FavouritesRow> {
    let mut rows = Vec::with_capacity(part.rows.len());
    let mut top = 0.0;
    for row in &part.rows {
        if !filter_match(&row.title, filter) {
            continue;
        }
        rows.push(FavouritesRow {
            id: row.id.clone(),
            top,
            height: LIBRARY_ROW_HEIGHT,
            row: row.clone(),
        });
        top += LIBRARY_ROW_HEIGHT;
    }
    rows
}

impl LibraryState {
    /// The Favourites view's own state of `lib_idx` (design D6): its scroll and
    /// its filter text, unscrolled and unfiltered until the list moves them.
    #[must_use]
    pub fn favourites_view(&self, lib_idx: usize) -> FavouritesView {
        self.favourites_views
            .peek()
            .get(&lib_idx)
            .cloned()
            .unwrap_or_default()
    }

    /// Write one library's Favourites view state (design D6). An unchanged value
    /// marks no subscriber, so a repeated measure repaints nothing.
    fn set_favourites_view(&self, lib_idx: usize, view: FavouritesView) {
        if self.favourites_view(lib_idx) == view {
            return;
        }
        let mut store = self.favourites_views;
        store.with_mut(|views| {
            views.insert(lib_idx, view);
        });
    }

    /// Report the Favourites list's measured viewport height (design D6): the
    /// rows in view follow it, the offset stays inside the narrower range, and a
    /// measurement that moves the viewport keeps the list's picked row in view.
    pub fn measure_favourites_list(
        &self,
        lib_idx: usize,
        viewport_height: f32,
        selected: Option<&str>,
        rows: &[FavouritesRow],
    ) {
        let held = self.favourites_view(lib_idx);
        let top = measure_scroll_top(rows, selected, held.scroll, viewport_height);
        self.set_favourites_view(
            lib_idx,
            FavouritesView {
                scroll: ListScroll {
                    scroll_top: top,
                    viewport_height,
                },
                filter: held.filter,
            },
        );
    }

    /// Report the Favourites list's native scroll offset (design D6): a user
    /// scroll moves the rows in view, and the offset stays inside the list's
    /// range.
    pub fn scroll_favourites_list(&self, lib_idx: usize, scroll_top: f32, rows: &[FavouritesRow]) {
        let held = self.favourites_view(lib_idx);
        let top = clamp_scroll_top(rows, scroll_top, held.scroll.viewport_height);
        self.set_favourites_view(
            lib_idx,
            FavouritesView {
                scroll: ListScroll {
                    scroll_top: top,
                    viewport_height: held.scroll.viewport_height,
                },
                filter: held.filter,
            },
        );
    }

    /// Narrow the Favourites list to the titles that contain `filter`, without
    /// case (design D6): the list's own text narrows its own rows, so the
    /// destination's All filter keeps its text for the All list. The offset
    /// stays inside the rows the new text leaves.
    pub fn set_favourites_filter(&self, lib_idx: usize, filter: String) {
        let held = self.favourites_view(lib_idx);
        let rows = self.favourites_rows_under(lib_idx, &filter);
        let top = clamp_scroll_top(&rows, held.scroll.scroll_top, held.scroll.viewport_height);
        self.set_favourites_view(
            lib_idx,
            FavouritesView {
                scroll: ListScroll {
                    scroll_top: top,
                    viewport_height: held.scroll.viewport_height,
                },
                filter,
            },
        );
    }

    /// Step the Favourites list's scroll one movement key along (design D6):
    /// the keyboard's arrows, `PageUp` and `PageDown`, `Home`, and `End` over the list,
    /// through the same geometry the destination scrolls step. The list's own
    /// store moves, so the destination's All scroll never does.
    pub fn step_favourites_scroll(&self, lib_idx: usize, mv: super::list::ListMove) {
        let held = self.favourites_view(lib_idx);
        let rows = self.favourites_rows_under(lib_idx, &held.filter);
        let top = scroll_step(&rows, held.scroll, mv);
        self.set_favourites_view(
            lib_idx,
            FavouritesView {
                scroll: ListScroll {
                    scroll_top: top,
                    viewport_height: held.scroll.viewport_height,
                },
                filter: held.filter,
            },
        );
    }

    /// Close the Favourites list's filter with Esc (design D6): the text goes
    /// with it, so a closed filter narrows nothing, and the scroll follows the
    /// widened rows.
    pub fn close_favourites_filter(&self, lib_idx: usize) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        self.set_favourites_filter(lib_idx, String::new());
    }

    /// The rows `lib_idx`'s Favourites list paints under `filter` (design D6):
    /// the state's own read of the carried part, so a filter write clamps the
    /// offset against the rows the new text leaves. A destination that paints
    /// no Favourites list paints no row.
    fn favourites_rows_under(&self, lib_idx: usize, filter: &str) -> Vec<FavouritesRow> {
        let carried = self.sections.library_list.peek();
        favourites_part(carried.as_deref())
            .filter(|(site, _)| site.lib_idx() == lib_idx)
            .map_or_else(Vec::new, |(_, part)| favourites_rows(part, filter))
    }

    /// Press a Favourites row (design D6): the row's own stable Service id picks
    /// it for the Favourites Hero through the shared detail command, which
    /// resolves the id against the favourites store while the view is
    /// Favourites. A press closes an open menu, because its facts belong to the
    /// previous row.
    pub fn press_favourites_row(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::ShowDetail {
            item_id: item_id.to_owned(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Play a Favourites row (design D6): the double-click's play and the hover
    /// Play, through the shared play command. A favourite music artist plays its
    /// tracks (design D8).
    pub fn play_favourites_row(&self, item_id: &str) {
        self.play_library_item(item_id);
    }

    /// Add a Favourites row to the queue's tail (design D6): the hover Add,
    /// through the shared add command.
    pub fn queue_favourites_row(&self, item_id: &str) {
        self.queue_library_item(item_id);
    }
}

/// The Favourites view's window stores (design D6): one scroll and one filter
/// text per library, keyed by the library index the site names.
pub type FavouritesViews = BTreeMap<usize, FavouritesView>;

#[cfg(test)]
mod tests;
