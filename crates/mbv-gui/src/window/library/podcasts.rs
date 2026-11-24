//! The Podcasts destination's interaction state (`add-gui-library-panel`
//! design D12, row 10.1, the port of `lib/library/podcasts.ts`): what the
//! window content owns and the core never reads — the `/` filter's text. The
//! scroll offset and the measured viewport live in the Podcasts list's scroll
//! store ([`super::scroll`], design D9). The core owns the collection selection
//! and the selected episode (the frame's `collections` and `selected`), so
//! nothing here mirrors them: a press sends the select command, and the frame
//! carries the choice back. The row model is [`super::lettered`]'s; the
//! Hero overlay stays in [`super::state`]'s shared store. `n` is not a Library
//! key, so it goes to the core's router.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).
//!
//! The state, the filter, and the scroll store are the filtered-list
//! controller's ([`super::list_state`], row 7C.1); this module keeps the
//! Podcasts frame, the Podcasts rows, the Podcasts command shapes, and the
//! Podcasts names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEntryRowFrame;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::lettered::{LetteredItemRow, PodcastsList, build_rows};
use crate::window::library::list::LetteredRow;
use crate::window::library::parts::library_podcasts_list;

use super::list_state::ListUiState;
use super::state::LibraryState;

/// The Podcasts list's interaction state: the shared filtered-list state over
/// the show selector (gui-collection-dropdown design D6).
pub type PodcastsUiState = ListUiState;

/// The state a freshly mounted Podcasts list starts with.
#[must_use]
pub fn initial_podcasts_ui() -> PodcastsUiState {
    ListUiState::initial()
}

impl LibraryState {
    /// The Podcasts library index the Podcasts commands name, or `None` while
    /// another destination holds the window. The frame is the section's
    /// carried value; a keydown copies only the index.
    fn podcasts_lib_idx(&self) -> Option<usize> {
        let list = self.sections.library_list.peek();
        library_podcasts_list(list.as_deref()).map(|podcasts| podcasts.lib_idx)
    }

    /// The painted rows of the carried Podcasts content (design D12), narrowed
    /// by the state's filter text. No carried Podcasts content paints no rows.
    /// The rows borrow the section's carried frame through the memo's revision
    /// key — a keydown copies none of it.
    #[must_use]
    pub fn podcasts_rows(&self) -> Arc<[LetteredRow<LetteredItemRow<GuiEntryRowFrame>>]> {
        let list = self.sections.library_list.peek();
        let Some(podcasts) = library_podcasts_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let filter = self.podcasts_ui.peek().filter.clone();
        self.podcasts_rows_memo.rows(revision, filter, |filter| {
            build_rows::<PodcastsList>(&podcasts.rows, filter)
        })
    }

    /// The row the pointer's marker names (design D1, D2): the marker carries
    /// the row's own id, so the painted rows resolve it and a stale or foreign
    /// id names no episode. `None` while no painted row carries the id.
    #[must_use]
    pub fn podcasts_row_at(&self, row_id: &str) -> Option<LetteredItemRow<GuiEntryRowFrame>> {
        self.podcasts_rows().iter().find_map(|row| match row {
            LetteredRow::Item(item) if item.id == row_id => Some(item.clone()),
            _ => None,
        })
    }

    /// Select the row (design D12): the select command the core stores, whose
    /// frame paints the Hero. A select never opens the Narrow arrangement's
    /// Hero overlay (only View Detail does) and closes an open menu, because
    /// the facts they paint belong to the previous episode, as a Music pick's
    /// do.
    pub fn select_podcasts_row(&self, row: &LetteredItemRow<GuiEntryRowFrame>) {
        let Some(lib_idx) = self.podcasts_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::PodcastsSelect {
            lib_idx,
            row: row.row.id.clone(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press an episode row (design D12): the press selects the episode the
    /// Hero shows.
    pub fn press_podcasts_row(&self, row: &LetteredItemRow<GuiEntryRowFrame>) {
        self.select_podcasts_row(row);
    }

    /// Play an episode (design D12): Enter on the hovered row, and a
    /// double-click's play. The queue is replaced under the Audiobookshelf
    /// admission rule, through the shared play path. The row id is the
    /// episode's stable identity the frame carried.
    pub fn play_podcasts_row(&self, item_id: &str) {
        let Some(lib_idx) = self.podcasts_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::PodcastsPlay {
            lib_idx,
            item_id: item_id.to_owned(),
        });
    }

    /// Add an episode to the queue's tail (design D12): `a` on the hovered
    /// row.
    pub fn queue_podcasts_row(&self, item_id: &str) {
        let Some(lib_idx) = self.podcasts_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::PodcastsQueue {
            lib_idx,
            item_id: item_id.to_owned(),
        });
    }
}
