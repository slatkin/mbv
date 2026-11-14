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
use crate::window::library::list::{
    ListMove, clamp_scroll_top, list_window, scroll_top_to_show_cursor,
};
use crate::window::library::parts::library_home_list;
use crate::window::library::scroll::ListScroll;

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
    pub(super) fn home_rows(&self, filter: &str) -> Arc<[HomeListRow]> {
        let list = self.sections.library_list.peek();
        let Some(home) = library_home_list(list.as_ref()) else {
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

    /// Scroll the list so the cursor's row paints fully, in the scroll store
    /// (design D12, D9).
    fn follow_home_cursor(&self, rows: &[HomeListRow], cursor: Option<&str>) {
        let held = *self.scrolls.home.peek();
        let shown = ListScroll {
            scroll_top: scroll_top_to_show_cursor(
                rows,
                cursor,
                held.scroll_top,
                held.viewport_height,
            ),
            viewport_height: held.viewport_height,
        };
        if shown != held {
            let mut home_scroll = self.scrolls.home;
            home_scroll.set(shown);
        }
    }

    /// Settle the cursor once the painted rows may have narrowed (design D12):
    /// the cursor keeps its row while that row still paints, and falls to the
    /// first item row otherwise. The scroll store then shows the cursor — the
    /// Music filter's own resolve (design D7).
    fn settle_home_cursor(&self) {
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        let kept = ui
            .cursor
            .as_deref()
            .is_some_and(|cursor| rows.iter().any(|row| row.id() == cursor));
        let cursor = if kept {
            ui.cursor.clone()
        } else {
            move_home_cursor(&rows, None, ListMove::Home, 1)
        };
        if cursor != ui.cursor {
            let mut home_ui = self.home_ui;
            home_ui.with_mut(|current| current.cursor.clone_from(&cursor));
        }
        self.follow_home_cursor(&rows, cursor.as_deref());
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
        let mut home_ui = self.home_ui;
        home_ui.with_mut(|ui| ui.filter = filter);
        self.settle_home_cursor();
        self.settle_home_selection_after(before.as_deref());
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The cursor follows the widened
    /// rows, and its settle selects the row it rests on.
    pub fn close_home_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let before = self.home_ui.peek().cursor.clone();
        let mut home_ui = self.home_ui;
        home_ui.with_mut(|ui| ui.filter.clear());
        self.settle_home_cursor();
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

    /// Accept the filter with Enter: the field closes and the list keeps the
    /// text (design D3, step 2).
    pub fn accept_home_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
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
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        let scroll = *self.scrolls.home.peek();
        let view = list_window(&rows, scroll.scroll_top, scroll.viewport_height);
        let cursor = move_home_cursor(&rows, ui.cursor.as_deref(), mv, view.end - view.start);
        if cursor == ui.cursor {
            return;
        }
        let mut home_ui = self.home_ui;
        home_ui.with_mut(|current| current.cursor.clone_from(&cursor));
        self.follow_home_cursor(&rows, cursor.as_deref());
        let row = Self::home_cursor_row(&rows, cursor.as_deref());
        self.settle_home_selection(row);
    }

    /// Report the Home list's measured viewport height (design D12): the rows
    /// in view follow it, and the scroll offset stays inside the narrower
    /// range.
    pub fn measure_home_list(&self, viewport_height: f32) {
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        let held = *self.scrolls.home.peek();
        let shown = ListScroll {
            viewport_height,
            scroll_top: clamp_scroll_top(&rows, held.scroll_top, viewport_height),
        };
        if shown != held {
            let mut home_scroll = self.scrolls.home;
            home_scroll.set(shown);
        }
    }

    /// Report the Home list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_home_list(&self, scroll_top: f32) {
        let ui = self.home_ui.peek().clone();
        let rows = self.home_rows(&ui.filter);
        let held = *self.scrolls.home.peek();
        let shown = ListScroll {
            scroll_top: clamp_scroll_top(&rows, scroll_top, held.viewport_height),
            viewport_height: held.viewport_height,
        };
        if shown != held {
            let mut home_scroll = self.scrolls.home;
            home_scroll.set(shown);
        }
    }
}

#[cfg(test)]
mod tests;
