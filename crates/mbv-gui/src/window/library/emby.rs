//! The plain Emby-library destination's interaction state
//! (`add-gui-library-panel` design D12, row 8.1, the port of
//! `lib/library/emby.ts`): what the window content owns and the core never
//! reads — the `/` filter's text. The scroll offset and the measured viewport
//! live in the Emby list's scroll store ([`super::scroll`], design D9). The core
//! owns the selected row and the feed-group selection (the frame's `selected`
//! and `collections`), so nothing here mirrors them: a press and an Enter send
//! the command, and the frame carries the choice back. The row model is
//! [`super::lettered`]'s; the Hero overlay stays in [`super::state`]'s shared
//! store, and the Hero carries no Workspace and no cursor of its own.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).
//!
//! The state, the filter, and the scroll store are the filtered-list
//! controller's ([`super::list_state`], row 7C.1); this module keeps the Emby
//! frame, the Emby rows and their letter headings, the letter rail, the Emby
//! command shapes, and the Emby names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyRowFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::lettered::{EmbyList, LetteredItemRow, build_rows};
use crate::window::library::list::LetteredRow;
use crate::window::library::parts::library_emby_list;

use super::list_state::ListUiState;
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

    /// The painted rows of the carried Emby content (row 8.1; gui-flat-list-sort
    /// design D8): the rows under their letter headings while the sort compares
    /// a name, flat under a date sort, narrowed by the state's filter text. No
    /// carried Emby content paints no rows.
    #[must_use]
    pub fn emby_rows(&self) -> Arc<[LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>]> {
        let list = self.sections.library_list.peek();
        let Some(frame) = library_emby_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let filter = self.emby_ui.peek().filter.clone();
        self.emby_rows_memo.rows(revision, filter, |filter| {
            build_rows::<EmbyList>(&frame.rows, filter)
        })
    }

    /// The row the pointer's marker names (design D1, D2): the marker carries
    /// the row's own id, so the painted rows resolve it and a stale or foreign
    /// id names no item. `None` while no painted item row carries the id — a
    /// letter heading carries none at all.
    #[must_use]
    pub fn emby_row_at(&self, row_id: &str) -> Option<LetteredItemRow<GuiEmbyRowFrame>> {
        let rows = self.emby_rows();
        rows.iter().find_map(|row| match row {
            LetteredRow::Item(item) if item.id == row_id => Some(item.clone()),
            _ => None,
        })
    }

    /// Select the row (row 8.1): the select command the core stores, whose
    /// frame paints the Hero. A select never opens the Narrow arrangement's
    /// Hero overlay (only View Detail does) and closes an open menu, because
    /// the facts they paint belong to the previous row, as a Music pick's do.
    /// A heading names no row, so its select changes nothing.
    pub fn select_emby_row(&self, row: &LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>) {
        let LetteredRow::Item(item) = row else {
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
    pub fn press_emby_row(&self, row: &LetteredRow<LetteredItemRow<GuiEmbyRowFrame>>) {
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
}
