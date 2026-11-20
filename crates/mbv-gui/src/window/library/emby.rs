//! The plain Emby-library destination's interaction state
//! (`add-gui-library-panel` design D12, row 8.1, the port of
//! `lib/library/emby.ts`): what the window content owns and the core never
//! reads — the `/` filter's text. The scroll offset and the measured viewport
//! live in the Emby list's scroll store ([`super::scroll`], design D9). The core
//! owns the selected row and the feed-group selection (the frame's `selected`
//! and `collections`), so nothing here mirrors them: a press and an Enter send
//! the command, and the frame carries the choice back. The row model is
//! [`super::emby_list`]'s; the Hero overlay stays in [`super::state`]'s shared
//! store, and the Hero carries no Workspace and no cursor of its own.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).
//!
//! The state, the filter, and the scroll store are the filtered-list
//! controller's ([`super::list_state`], row 7C.1); this module keeps the Emby
//! frame, the Emby rows and their letter headings, the letter rail, the
//! play-next command, the Emby command shapes, and the Emby names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::emby_list::{EmbyItemRow, EmbyListRow, emby_letter_rail};
use crate::window::library::list::{ListMove, jump_to_letter, scroll_step};
use crate::window::library::parts::library_emby_list;

use super::list_state::ListUiState;
use super::scroll::set_scroll_top;
use super::state::LibraryState;

/// The Emby list's interaction state: the shared filtered-list state over the
/// feed-group mode's folder selector (gui-collection-dropdown design D6).
pub type EmbyUiState = ListUiState;

/// The state a freshly mounted Emby list starts with.
#[must_use]
pub fn initial_emby_ui() -> EmbyUiState {
    ListUiState::initial()
}

impl LibraryState {
    /// The Emby library index a command names, or `None` while another
    /// destination holds the window. The frame is the section's carried
    /// value; a keydown borrows it and copies only the index.
    pub(super) fn emby_lib_idx(&self) -> Option<usize> {
        let list = self.sections.library_list.peek();
        library_emby_list(list.as_deref()).map(|emby| emby.lib_idx)
    }

    /// The painted rows of the carried Emby content (row 8.1): the plain
    /// mode's rows under their letter headings, or the feed-group mode's flat
    /// rows, narrowed by the state's filter text. No carried Emby content
    /// paints no rows.
    #[must_use]
    pub fn emby_rows(&self) -> Arc<[EmbyListRow]> {
        let list = self.sections.library_list.peek();
        let Some(frame) = library_emby_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let filter = self.emby_ui.peek().filter.clone();
        self.emby_rows_memo
            .rows(revision, frame.feed_group, &frame.rows, &filter)
    }

    /// The row the pointer's marker names (design D1, D2): the marker carries
    /// the row's own id, so the painted rows resolve it and a stale or foreign
    /// id names no item. `None` while no painted item row carries the id — a
    /// letter heading carries none at all.
    #[must_use]
    pub fn emby_row_at(&self, row_id: &str) -> Option<EmbyItemRow> {
        let rows = self.emby_rows();
        rows.iter().find_map(|row| match row {
            EmbyListRow::Item(item) if item.id == row_id => Some(item.clone()),
            _ => None,
        })
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule). The filter narrows the rows and selects
    /// nothing, and the scroll stays inside the narrower list.
    pub fn set_emby_filter(&self, filter: String) {
        let mut emby_ui = self.emby_ui;
        emby_ui.with_mut(|ui| ui.filter = filter);
        self.clamp_emby_scroll();
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it, so
    /// a closed filter narrows nothing. The scroll follows the widened rows.
    pub fn close_emby_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut emby_ui = self.emby_ui;
        emby_ui.with_mut(|ui| ui.filter.clear());
        self.clamp_emby_scroll();
    }

    /// Select the row (row 8.1): the select command the core stores, whose
    /// frame paints the Hero. A select never opens the Narrow arrangement's
    /// Hero overlay (only View Detail does) and closes an open menu, because the facts they paint belong to
    /// the previous row, as a Music pick's do. A heading names no row, so its
    /// select changes nothing.
    pub fn select_emby_row(&self, row: &EmbyListRow) {
        let EmbyListRow::Item(item) = row else {
            return;
        };
        let Some(lib_idx) = self.emby_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::EmbySelect {
            lib_idx,
            row: item.row.id.clone(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press a row (row 8.1): the press selects the row the Hero shows, as a
    /// Home press does. A heading names no row, so its press changes nothing.
    pub fn press_emby_row(&self, row: &EmbyListRow) {
        self.select_emby_row(row);
    }

    /// Play a row (row 8.1): Enter on the hovered row, and a double-click's
    /// play. Every row is playable, a folder and a boxset included, because
    /// the core's folder-play rule expands a collection row. The row id is the
    /// item's stable Service id.
    pub fn play_emby_row(&self, row_id: &str) {
        let Some(lib_idx) = self.emby_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::EmbyPlay {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// Play a row next (row 8.1): `n` on the hovered row.
    pub fn play_emby_row_next(&self, row_id: &str) {
        let Some(lib_idx) = self.emby_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::EmbyPlayNext {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// Add a row to the queue's tail (row 8.1): `a` on the hovered row.
    pub fn queue_emby_row(&self, row_id: &str) {
        let Some(lib_idx) = self.emby_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::EmbyQueue {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// Scroll the list by one movement key (design D6): Up and Down move by the
    /// height of the row at the top edge, Page Up and Page Down by the viewport
    /// height, and Home and End go to the list's ends, inside the list's range.
    pub fn step_emby_scroll(&self, mv: ListMove) {
        let rows = self.emby_rows();
        let held = *self.scrolls.emby.peek();
        set_scroll_top(self.scrolls.emby, &rows, scroll_step(&rows, held, mv));
    }

    /// Jump to a letter of the rail (row 8.1, design D7): the list scrolls so
    /// the first row under the letter paints fully. A letter without rows
    /// changes nothing.
    pub fn jump_emby_to_letter(&self, letter: char) {
        let rows = self.emby_rows();
        let view = *self.scrolls.emby.peek();
        let Some((_, scroll_top)) = jump_to_letter(&emby_letter_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        set_scroll_top(self.scrolls.emby, &rows, scroll_top);
    }

    /// Report the list's measured viewport height (row 8.1): the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_emby_list(&self, viewport_height: f32) {
        let mut emby = self.scrolls.emby;
        emby.with_mut(|scroll| scroll.viewport_height = viewport_height);
        self.clamp_emby_scroll();
    }

    /// Report the list's native scroll offset (row 8.1): a user scroll moves
    /// the rows in view, and the offset stays inside the list's range.
    pub fn scroll_emby_list(&self, scroll_top: f32) {
        let rows = self.emby_rows();
        set_scroll_top(self.scrolls.emby, &rows, scroll_top);
    }

    /// Hold the Emby list's scroll offset inside the rows the filter leaves.
    fn clamp_emby_scroll(&self) {
        let rows = self.emby_rows();
        let held = *self.scrolls.emby.peek();
        set_scroll_top(self.scrolls.emby, &rows, held.scroll_top);
    }
}
