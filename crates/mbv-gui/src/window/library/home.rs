//! The Home destination's interaction state (`add-gui-library-panel` design
//! D1, D12, the port of `lib/library/home.ts`): what the window content owns
//! and the core never reads — the `/` filter's text. The scroll offset and the
//! measured viewport live in the Home list's scroll store ([`super::scroll`],
//! design D9). The core owns the selection (the frame's `selected` row), so
//! nothing here mirrors it: a press sends the select command, and the frame
//! carries the selection back. The keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), so the list keeps no cursor. The row
//! model is [`super::home_list`]'s; the Hero overlay stays in
//! [`super::state`]'s shared store.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeRowIdFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::home_list::HomeListRow;
use crate::window::library::list::{ListMove, scroll_step};
use crate::window::library::parts::library_home_list;
use crate::window::library::scroll::set_scroll_top;

use super::state::LibraryState;

/// The Home list's interaction state (`home.ts`'s `HomeUiState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeUiState {
    /// The `/` filter's text: the list paints only the item rows whose title
    /// contains it, without case (design D7's filter rule).
    pub filter: String,
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

    /// The item row the pointer's marker names (design D1, D2): the marker
    /// carries the row's own key, so the painted rows resolve it and a stale or
    /// foreign key names no row. `None` while the pointer rests on no item row —
    /// a heading, a spacer, or no row at all.
    #[must_use]
    pub fn home_row_id(&self, row_key: &str) -> Option<GuiHomeRowIdFrame> {
        let filter = self.home_ui.peek().filter.clone();
        let rows = self.home_rows(&filter);
        match rows.iter().find(|row| row.id() == row_key)? {
            HomeListRow::Item { row, .. } => Some(row.row_id.clone()),
            HomeListRow::Heading { .. } | HomeListRow::Spacer { .. } => None,
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

    /// Narrow the list to the titles that contain `filter`, without case (design
    /// D7's filter rule). The filter narrows the rows and selects nothing, and
    /// the scroll stays inside the narrower list.
    pub fn set_home_filter(&self, filter: String) {
        let mut ui = self.home_ui;
        ui.with_mut(|ui| ui.filter = filter);
        self.clamp_home_scroll();
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The scroll follows the widened rows.
    pub fn close_home_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut home = self.home_ui;
        home.with_mut(|ui| ui.filter.clear());
        self.clamp_home_scroll();
    }

    /// Press a Home row (design D12): the press selects, and Enter or a press
    /// on an already-selected row never plays it — the press is the selection's
    /// half, and the double-click is the play's. A heading or spacer names no
    /// item, so its press changes nothing.
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

    /// Scroll the Home list by one movement key (design D6): Up and Down move
    /// by the height of the row at the top edge, Page Up and Page Down by the
    /// viewport height, and Home and End go to the list's ends, inside the
    /// list's range.
    pub fn step_home_scroll(&self, mv: ListMove) {
        let filter = self.home_ui.peek().filter.clone();
        let rows = self.home_rows(&filter);
        let held = *self.scrolls.home.peek();
        set_scroll_top(self.scrolls.home, &rows, scroll_step(&rows, held, mv));
    }

    /// Report the Home list's measured viewport height (design D12): the rows
    /// in view follow it, and the scroll offset stays inside the narrower
    /// range.
    pub fn measure_home_list(&self, viewport_height: f32) {
        let mut home = self.scrolls.home;
        home.with_mut(|scroll| scroll.viewport_height = viewport_height);
        let filter = self.home_ui.peek().filter.clone();
        let rows = self.home_rows(&filter);
        let held = *home.peek();
        set_scroll_top(home, &rows, held.scroll_top);
    }

    /// Report the Home list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_home_list(&self, scroll_top: f32) {
        let filter = self.home_ui.peek().filter.clone();
        let rows = self.home_rows(&filter);
        set_scroll_top(self.scrolls.home, &rows, scroll_top);
    }

    /// Hold the Home list's scroll offset inside the rows the filter leaves.
    fn clamp_home_scroll(&self) {
        let filter = self.home_ui.peek().filter.clone();
        let rows = self.home_rows(&filter);
        let held = *self.scrolls.home.peek();
        set_scroll_top(self.scrolls.home, &rows, held.scroll_top);
    }
}

#[cfg(test)]
mod tests;
