//! The Home destination's interaction state (`add-gui-library-panel` design
//! D1, D12, the port of `lib/library/home.ts`): what the window content owns
//! and the core never reads — the cursor. The scroll offset and the measured
//! viewport live in the Home list's scroll store ([`super::scroll`], design
//! D9). The core owns the selection (the frame's `selected` row), so nothing
//! here mirrors it: a press and the cursor's settle send the select command,
//! and the frame carries the selection back. The row model is
//! [`super::home_list`]'s; the panel's pane focus and the Hero overlay stay in
//! [`super::state`]'s shared store.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeRowIdFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::home_list::{HomeListRow, move_home_cursor};
use crate::window::library::list::ListMove;
use crate::window::library::parts::library_home_list;

use super::pill_list::{ListControl, ListUi};
use super::state::LibraryState;

/// The Home list's interaction state (`home.ts`'s `HomeUiState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeUiState {
    /// The cursor's row key, or `None` while no row carries the cursor
    /// (design D12). The cursor moves over the item rows only.
    pub cursor: Option<String>,
    /// The `/` filter's text: the list paints only the item rows whose title
    /// contains it, without case (design D7's filter rule).
    pub filter: String,
}

impl ListUi for HomeUiState {
    fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    fn set_cursor(&mut self, cursor: Option<String>) {
        self.cursor = cursor;
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }
}

impl HomeUiState {
    /// The state a freshly mounted Home list starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self::default()
    }
}

impl LibraryState {
    /// The painted rows of the carried Home content (design D12), narrowed by
    /// `filter`'s text. No carried Home content paints no rows.
    #[must_use]
    pub fn home_rows(&self, filter: &str) -> Arc<[HomeListRow]> {
        let list = self.sections.library_list.peek();
        let Some(home) = library_home_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        self.home_rows_memo.rows(revision, &home.rows, filter)
    }

    /// The row the cursor rests on, or `None` while no painted row carries it.
    fn home_cursor_row<'a>(
        rows: &'a [HomeListRow],
        cursor: Option<&str>,
    ) -> Option<&'a HomeListRow> {
        rows.iter().find(|row| Some(row.id()) == cursor)
    }

    /// The item row id the Home cursor rests on (design D12): the opaque
    /// identity the play, play-next, and add-to-queue keys carry. `None` while
    /// the cursor rests on no item row.
    #[must_use]
    pub fn home_cursor_row_id(&self) -> Option<GuiHomeRowIdFrame> {
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        match Self::home_cursor_row(&rows, ui.cursor.as_deref())? {
            HomeListRow::Item { row, .. } => Some(row.row_id.clone()),
            _ => None,
        }
    }

    /// The Home list under the shared list controller (design D12).
    fn home_list(
        &self,
    ) -> ListControl<HomeUiState, HomeListRow, impl Fn() -> Arc<[HomeListRow]> + '_> {
        ListControl {
            ui: self.home_ui,
            scroll: self.scrolls.home,
            rows: || self.home_rows(&self.home_ui.peek().filter),
            move_over: move_home_cursor,
        }
    }

    /// Select the row (design D12): the select command the core stores, whose
    /// frame paints the Hero. A select repaints the Narrow arrangement's Hero
    /// overlay and closes an open menu, because the facts they paint belong to
    /// the previous row (the pick's reset, as Music's does).
    fn settle_home_selection(&self, row: Option<&HomeListRow>) {
        let Some(HomeListRow::Item { row, .. }) = row else {
            return;
        };
        self.link.library(GuiLibraryCommand::HomeSelect {
            row: crate::bridge::library::GuiHomeRowId::from(&row.row_id),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule): the cursor follows the rows, and its settle
    /// selects the row it rests on, as a cursor move does.
    pub fn set_home_filter(&self, filter: String) {
        let before = self.home_ui.peek().cursor.clone();
        self.home_list().set_filter(filter);
        self.settle_home_selection_after(before.as_deref());
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The cursor follows the widened
    /// rows, and its settle selects the row it rests on.
    pub fn close_home_filter(&self) {
        let before = self.home_ui.peek().cursor.clone();
        self.home_list().close_filter(self.ui);
        self.settle_home_selection_after(before.as_deref());
    }

    /// Select the row the cursor settled on, when the settle moved it (design
    /// D12): the Hero follows the cursor, named by the opaque id and never a
    /// position.
    fn settle_home_selection_after(&self, before: Option<&str>) {
        let after = self.home_ui.peek().cursor.clone();
        if after.as_deref() == before {
            return;
        }
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        let row = Self::home_cursor_row(&rows, after.as_deref());
        self.settle_home_selection(row);
    }

    /// Press a Home row (design D12): the press selects, and Enter or a press
    /// on an already-selected row plays it — the press never plays on its own.
    /// A heading or spacer names no item, so its press changes nothing.
    pub fn press_home_row(&self, row: &HomeListRow) {
        self.settle_home_selection(Some(row));
    }

    /// Play a Home row (design D12): Enter's play, and a double-click's.
    pub fn play_home_row(&self, row_id: GuiHomeRowIdFrame) {
        self.link
            .library(GuiLibraryCommand::HomePlay { row: row_id.into() });
    }

    /// Enqueue a Home row directly after the active slot, or at the tail with
    /// no active slot (design D12): `n` and the Hero's Play next.
    pub fn play_home_row_next(&self, row_id: GuiHomeRowIdFrame) {
        self.link
            .library(GuiLibraryCommand::HomePlayNext { row: row_id.into() });
    }

    /// Add a Home row to the queue's tail (design D12): `a` and the Hero's Add
    /// to queue.
    pub fn queue_home_row(&self, row_id: GuiHomeRowIdFrame) {
        self.link
            .library(GuiLibraryCommand::HomeQueue { row: row_id.into() });
    }

    /// Move the Home cursor one movement along (design D12): Up and Down move
    /// an item row, Page Up and Page Down move the rows in view, and Home and
    /// End take the list's ends, stepping over the headings and spacers. The
    /// scroll offset follows, so the cursor row stays fully visible, and the
    /// cursor's settle selects the row it rests on — the Hero follows the
    /// cursor.
    pub fn move_home_list_cursor(&self, mv: ListMove) {
        if !self.home_list().move_cursor(mv) {
            return;
        }
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        let row = Self::home_cursor_row(&rows, ui.cursor.as_deref());
        self.settle_home_selection(row);
    }

    /// Report the Home list's measured viewport height (design D12): the rows
    /// in view follow it, and the scroll offset stays inside the narrower
    /// range.
    pub fn measure_home_list(&self, viewport_height: f32) {
        self.home_list().measure(viewport_height);
    }

    /// Report the Home list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_home_list(&self, scroll_top: f32) {
        self.home_list().scroll_to(scroll_top);
    }
}

#[cfg(test)]
mod tests;
