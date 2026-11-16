//! The Audiobooks destination's interaction state (`add-gui-library-panel`
//! design D12, row 9.1, the port of `lib/library/audiobooks.ts`): what the
//! window content owns and the core never reads — the cursor. The scroll
//! offset and the measured viewport live in the Audiobooks list's scroll store
//! ([`super::scroll`], design D9). The core owns the selected book (the
//! frame's `selected` and its Hero), so nothing here mirrors it: a press sends
//! the select command, and the frame carries the choice back. The row model is
//! [`super::audiobooks_list`]'s; the Hero's chapter cursor belongs to the Hero
//! ([`super::audiobooks_workspace`]), and the Hero
//! overlay stay in [`super::state`]'s shared store. There is no pill row on
//! the destination.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::audiobooks_list::{
    AudiobooksListRow, audiobooks_letter_rail, move_audiobooks_cursor,
};
use crate::window::library::list::{ListMove, jump_to_letter};
use crate::window::library::parts::library_audiobooks_list;

use super::pill_list::{ListControl, ListUi};
use super::state::LibraryState;

/// The book list's interaction state (`audiobooks.ts`'s `AudiobooksUiState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudiobooksUiState {
    /// The cursor's row id, or `None` while no row carries the cursor (row
    /// 9.1). The cursor moves over the book rows only.
    pub cursor: Option<String>,
    /// The `/` filter's text: the list paints only the books whose title
    /// contains it, without case (design D7's filter rule, the rail's
    /// keyboard path).
    pub filter: String,
}

impl ListUi for AudiobooksUiState {
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

impl AudiobooksUiState {
    /// The state a freshly mounted book list starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self::default()
    }
}

impl LibraryState {
    /// The Audiobooks library index the Audiobooks commands name, or `None`
    /// while another destination holds the window. The frame is the section's
    /// carried value; a keydown copies only the index.
    fn audiobooks_lib_idx(&self) -> Option<usize> {
        let list = self.sections.library_list.peek();
        library_audiobooks_list(list.as_deref()).map(|audiobooks| audiobooks.lib_idx)
    }

    /// The painted rows of the carried book content (row 9.1), narrowed by
    /// `filter`'s text. No carried book content paints no rows. The rows
    /// borrow the section's carried frame through the memo's revision key —
    /// a keydown copies none of it.
    #[must_use]
    pub fn audiobooks_rows(&self, filter: &str) -> Arc<[AudiobooksListRow]> {
        let list = self.sections.library_list.peek();
        let Some(audiobooks) = library_audiobooks_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        self.audiobooks_rows_memo
            .rows(revision, &audiobooks.rows, filter)
    }

    /// The row the book cursor rests on, or `None` (row 9.1): the keys read it
    /// to play the book or queue it. A heading never carries the cursor.
    #[must_use]
    pub fn audiobooks_cursor_row(&self) -> Option<AudiobooksListRow> {
        let ui = self.audiobooks_ui.peek().clone();
        let rows = self.audiobooks_rows(&ui.filter);
        rows.iter()
            .find(|row| {
                !matches!(row, AudiobooksListRow::Heading { .. })
                    && Some(row.id()) == ui.cursor.as_deref()
            })
            .cloned()
    }

    /// The Audiobooks list under the shared list controller (design D12).
    fn audiobooks_list(
        &self,
    ) -> ListControl<AudiobooksUiState, AudiobooksListRow, impl Fn() -> Arc<[AudiobooksListRow]> + '_>
    {
        ListControl {
            ui: self.audiobooks_ui,
            scroll: self.scrolls.audiobooks,
            rows: || self.audiobooks_rows(&self.audiobooks_ui.peek().filter),
            move_over: move_audiobooks_cursor,
        }
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule): the cursor follows the rows.
    pub fn set_audiobooks_filter(&self, filter: String) {
        self.audiobooks_list().set_filter(filter);
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it.
    pub fn close_audiobooks_filter(&self) {
        self.audiobooks_list().close_filter(self.ui);
    }

    /// Select the book (row 9.1): the select command the core stores, whose
    /// frame paints the Hero. A select repaints the Narrow arrangement's Hero
    /// overlay and closes an open menu, because the facts they paint belong to
    /// the previous book, as a Music pick's do. A heading names no book, so
    /// its select changes nothing.
    pub fn select_audiobooks_row(&self, row: &AudiobooksListRow) {
        let AudiobooksListRow::Item(item) = row else {
            return;
        };
        let Some(lib_idx) = self.audiobooks_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::AbsSelect {
            lib_idx,
            row: item.row.id.clone(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press a book row (row 9.1): the press selects the book the Hero shows.
    /// A heading names no book, so its press changes nothing.
    pub fn press_audiobooks_row(&self, row: &AudiobooksListRow) {
        self.select_audiobooks_row(row);
    }

    /// Play a book (row 9.1): Enter on the cursor book, and a double-click's
    /// play. The Owner replaces the queue with the book at its saved position,
    /// through the shared play path's admission. The book id is the row's
    /// stable `library_item_id`.
    pub fn play_audiobooks_row(&self, book_id: &str) {
        let Some(lib_idx) = self.audiobooks_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::AbsPlay {
            lib_idx,
            item_id: book_id.to_owned(),
        });
    }

    /// Add a book to the queue's tail (row 9.1): `a` on the cursor book. Books
    /// have no play-next, so `n` is consumed and does nothing here.
    pub fn queue_audiobooks_row(&self, book_id: &str) {
        let Some(lib_idx) = self.audiobooks_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::AbsQueue {
            lib_idx,
            item_id: book_id.to_owned(),
        });
    }

    /// Move the book list's cursor one movement along (row 9.1): Up and Down
    /// move a row, Page Up and Page Down move the rows in view, and Home and
    /// End take the list's ends, stepping over the letter headings. The scroll
    /// offset follows, so the cursor row stays fully visible.
    pub fn move_audiobooks_list_cursor(&self, mv: ListMove) {
        self.audiobooks_list().move_cursor(mv);
    }

    /// Jump to a letter of the rail (row 9.1, design D7): the cursor moves to
    /// the first book under the letter and that book scrolls into view. A
    /// letter without books changes nothing.
    pub fn jump_audiobooks_to_letter(&self, letter: char) {
        let ui = self.audiobooks_ui.peek().clone();
        let rows = self.audiobooks_rows(&ui.filter);
        let view = *self.scrolls.audiobooks.peek();
        let Some((cursor, scroll_top)) =
            jump_to_letter(&audiobooks_letter_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        let mut audiobooks_ui = self.audiobooks_ui;
        audiobooks_ui.with_mut(|held| held.cursor = Some(cursor));
        let mut audiobooks_scroll = self.scrolls.audiobooks;
        audiobooks_scroll.with_mut(|scroll| scroll.scroll_top = scroll_top);
    }

    /// Report the list's measured viewport height (row 9.1): the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_audiobooks_list(&self, viewport_height: f32) {
        self.audiobooks_list().measure(viewport_height);
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_audiobooks_list(&self, scroll_top: f32) {
        self.audiobooks_list().scroll_to(scroll_top);
    }
}

#[cfg(test)]
mod tests;
