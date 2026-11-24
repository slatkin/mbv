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
use crate::window::library::lettered::{AudiobooksList, LetteredItemRow, build_rows};
use crate::window::library::list::LetteredRow;
use crate::window::library::parts::library_audiobooks_list;

use super::list_state::ListUiState;
use super::state::LibraryState;

/// The book list's interaction state (`audiobooks.ts`'s `AudiobooksUiState`):
/// the shared filtered-list state (design D4). The books' `/` filter is the
/// same one text the other lettered destinations carry, so one generic method
/// set serves all four.
pub type AudiobooksUiState = ListUiState;

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
            .rows(revision, filter.to_owned(), |filter| {
                build_rows::<AudiobooksList>(&audiobooks.rows, filter)
            })
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

    /// Select the book (row 9.1): the select command the core stores, whose
    /// frame paints the Hero. A select never opens the Narrow arrangement's
    /// Hero overlay (only View Detail does) and closes an open menu, because
    /// the facts they paint belong to the previous book, as a Music pick's do.
    /// A heading names no book, so its select changes nothing.
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

    /// Add a book to the queue's tail (row 9.1): `a` on the hovered book.
    pub fn queue_audiobooks_row(&self, book_id: &str) {
        let Some(lib_idx) = self.audiobooks_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::AbsQueue {
            lib_idx,
            item_id: book_id.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests;
