//! The Audiobooks destination's interaction state (`add-gui-library-panel`
//! design D12, row 9.1, the port of `lib/library/audiobooks.ts`): what the
//! window content owns and the core never reads — the `/` filter's text. The
//! scroll offset and the measured viewport live in the Audiobooks list's scroll
//! store ([`super::scroll`], design D9). The core owns the selected book (the
//! frame's `selected` and its Hero), so nothing here mirrors it: a press sends
//! the select command, and the frame carries the choice back. The row model is
//! [`super::lettered`]'s, and the Hero's chapter Workspace is
//! [`super::audiobooks_workspace`]'s. There is no pill row on the destination.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiAudiobooksRowFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::lettered::LetteredItemRow;
use crate::window::library::list::{
    LetteredRow, ListMove, jump_to_letter, lettered_rail, scroll_step,
};
use crate::window::library::parts::library_audiobooks_list;

use super::scroll::set_scroll_top;
use super::state::LibraryState;

/// The book list's interaction state (`audiobooks.ts`'s `AudiobooksUiState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudiobooksUiState {
    /// The `/` filter's text: the list paints only the books whose title
    /// contains it, without case (design D7's filter rule, the rail's
    /// keyboard path).
    pub filter: String,
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
    pub fn audiobooks_rows(
        &self,
        filter: &str,
    ) -> Arc<[LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>]> {
        let list = self.sections.library_list.peek();
        let Some(audiobooks) = library_audiobooks_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        self.audiobooks_rows_memo
            .rows(revision, &audiobooks.rows, filter)
    }

    /// The painted rows under the filter's current text.
    fn audiobooks_painted_rows(
        &self,
    ) -> Arc<[LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>]> {
        let filter = self.audiobooks_ui.peek().filter.clone();
        self.audiobooks_rows(&filter)
    }

    /// The book row the pointer's marker names (design D1, D2): the marker
    /// carries the row's own id, so the painted rows resolve it and a stale or
    /// foreign id names no book. `None` while no painted item row carries the
    /// id — a letter heading carries none at all.
    #[must_use]
    pub fn audiobooks_row_at(
        &self,
        row_id: &str,
    ) -> Option<LetteredItemRow<GuiAudiobooksRowFrame>> {
        let rows = self.audiobooks_painted_rows();
        rows.iter().find_map(|row| match row {
            LetteredRow::Item(item) if item.id == row_id => Some(item.clone()),
            _ => None,
        })
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule). The filter narrows the rows and selects
    /// nothing, and the scroll stays inside the narrower list.
    pub fn set_audiobooks_filter(&self, filter: String) {
        let mut audiobooks_ui = self.audiobooks_ui;
        audiobooks_ui.with_mut(|ui| ui.filter = filter);
        self.clamp_audiobooks_scroll();
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it, so
    /// a closed filter narrows nothing. The scroll follows the widened rows.
    pub fn close_audiobooks_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut audiobooks_ui = self.audiobooks_ui;
        audiobooks_ui.with_mut(|ui| ui.filter.clear());
        self.clamp_audiobooks_scroll();
    }

    /// Select the book (row 9.1): the select command the core stores, whose
    /// frame paints the Hero. A select never opens the Narrow arrangement's
    /// Hero overlay (only View Detail does) and closes an open menu, because the facts they paint belong to
    /// the previous book, as a Music pick's do. A heading names no book, so
    /// its select changes nothing.
    pub fn select_audiobooks_row(&self, row: &LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>) {
        let LetteredRow::Item(item) = row else {
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
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press a book row (row 9.1): the press selects the book the Hero shows.
    /// A heading names no book, so its press changes nothing.
    pub fn press_audiobooks_row(&self, row: &LetteredRow<LetteredItemRow<GuiAudiobooksRowFrame>>) {
        self.select_audiobooks_row(row);
    }

    /// Play a book (row 9.1): Enter on the hovered book, and a double-click's
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

    /// Add a book to the queue's tail (row 9.1): `a` on the hovered book. Books
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

    /// Scroll the list by one movement key (design D6): Up and Down move by the
    /// height of the row at the top edge, Page Up and Page Down by the viewport
    /// height, and Home and End go to the list's ends, inside the list's range.
    pub fn step_audiobooks_scroll(&self, mv: ListMove) {
        let rows = self.audiobooks_painted_rows();
        let held = *self.scrolls.audiobooks.peek();
        set_scroll_top(self.scrolls.audiobooks, &rows, scroll_step(&rows, held, mv));
    }

    /// Jump to a letter of the rail (row 9.1, design D7): the list scrolls so
    /// the first book under the letter paints fully. A letter without books
    /// changes nothing.
    pub fn jump_audiobooks_to_letter(&self, letter: char) {
        let rows = self.audiobooks_painted_rows();
        let view = *self.scrolls.audiobooks.peek();
        let Some((_, scroll_top)) = jump_to_letter(&lettered_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        set_scroll_top(self.scrolls.audiobooks, &rows, scroll_top);
    }

    /// Report the list's measured viewport height (row 9.1): the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_audiobooks_list(&self, viewport_height: f32) {
        let mut audiobooks = self.scrolls.audiobooks;
        audiobooks.with_mut(|scroll| scroll.viewport_height = viewport_height);
        self.clamp_audiobooks_scroll();
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_audiobooks_list(&self, scroll_top: f32) {
        let rows = self.audiobooks_painted_rows();
        set_scroll_top(self.scrolls.audiobooks, &rows, scroll_top);
    }

    /// Hold the Audiobooks list's scroll offset inside the rows the filter
    /// leaves.
    fn clamp_audiobooks_scroll(&self) {
        let rows = self.audiobooks_painted_rows();
        let held = *self.scrolls.audiobooks.peek();
        set_scroll_top(self.scrolls.audiobooks, &rows, held.scroll_top);
    }
}

#[cfg(test)]
mod tests;
