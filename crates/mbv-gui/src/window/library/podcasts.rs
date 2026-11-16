//! The Podcasts destination's interaction state (`add-gui-library-panel`
//! design D12, row 10.1, the port of `lib/library/podcasts.ts`): what the
//! window content owns and the core never reads — the `/` filter's text and the
//! pill row's request. The scroll offset and the measured viewport live in the
//! Podcasts list's scroll store ([`super::scroll`], design D9). The core owns
//! the pill selection and the selected episode (the frame's `pills` and
//! `selected`), so nothing here mirrors them: a press sends the select command,
//! and the frame carries the choice back. The row model is
//! [`super::podcasts_list`]'s; the Hero overlay stays in [`super::state`]'s
//! shared store. There is no play-next: `n` is consumed and does nothing.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).
//!
//! The state, the filter, and the scroll store are the pill-list controller's
//! ([`super::pill_list`], row 7C.1); this module keeps the Podcasts frame, the
//! Podcasts rows, the Podcasts command shapes, and the Podcasts names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::library::{GuiLibraryCommand, GuiPodcastPill, GuiPodcastState};
use crate::window::library::list::{ListMove, scroll_step};
use crate::window::library::parts::library_podcasts_list;
use crate::window::library::podcasts_list::PodcastsItemRow;

use super::pill_list::PillListUiState;
use super::scroll::set_scroll_top;
use super::state::LibraryState;

/// The Podcasts list's interaction state: the shared pill-list state over the
/// Podcasts pill (design D12).
pub type PodcastsUiState = PillListUiState<GuiPodcastPill>;

/// The state a freshly mounted Podcasts list starts with.
#[must_use]
pub fn initial_podcasts_ui() -> PodcastsUiState {
    PillListUiState::initial()
}

/// The pill's stable key: the each-block's identity and the pill row's focus
/// key, over the pill object the frame carries (design D12).
#[must_use]
pub fn podcast_pill_key(pill: &GuiPodcastPill) -> String {
    match pill {
        GuiPodcastPill::Latest => "latest".to_owned(),
        GuiPodcastPill::State(state) => format!("state:{}", podcast_state_key(*state)),
        GuiPodcastPill::Show(id) => format!("show:{id}"),
    }
}

/// The state pill's stable key segment (design D12, row 10.1): the filter the
/// pill names, as the wire pill's id spells it.
#[must_use]
pub fn podcast_state_key(state: GuiPodcastState) -> &'static str {
    match state {
        GuiPodcastState::All => "all",
        GuiPodcastState::Unplayed => "unplayed",
        GuiPodcastState::Played => "played",
    }
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
    pub fn podcasts_rows(&self) -> Arc<[PodcastsItemRow]> {
        let list = self.sections.library_list.peek();
        let Some(podcasts) = library_podcasts_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let filter = self.podcasts_ui.peek().filter.clone();
        self.podcasts_rows_memo
            .rows(revision, &podcasts.rows, &filter)
    }

    /// The row the pointer's marker names (design D1, D2): the marker carries
    /// the row's own id, so the painted rows resolve it and a stale or foreign
    /// id names no episode. `None` while no painted row carries the id.
    #[must_use]
    pub fn podcasts_row_at(&self, row_id: &str) -> Option<PodcastsItemRow> {
        self.podcasts_rows()
            .iter()
            .find(|row| row.id == row_id)
            .cloned()
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule). The filter narrows the rows and selects
    /// nothing, and the scroll stays inside the narrower list.
    pub fn set_podcasts_filter(&self, filter: String) {
        let mut podcasts_ui = self.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.filter = filter);
        self.clamp_podcasts_scroll();
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it, so
    /// a closed filter narrows nothing. The scroll follows the widened rows.
    pub fn close_podcasts_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut podcasts_ui = self.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.filter.clear());
        self.clamp_podcasts_scroll();
    }

    /// Select the row (design D12): the select command the core stores, whose
    /// frame paints the Hero. A select repaints the Narrow arrangement's Hero
    /// overlay and closes an open menu, because the facts they paint belong to
    /// the previous episode, as a Music pick's do.
    pub fn select_podcasts_row(&self, row: &PodcastsItemRow) {
        let Some(lib_idx) = self.podcasts_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::PodcastsSelect {
            lib_idx,
            row: row.row.id.clone(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press an episode row (design D12): the press selects the episode the
    /// Hero shows.
    pub fn press_podcasts_row(&self, row: &PodcastsItemRow) {
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
    /// row. There is no play-next on the destination, so `n` is consumed and
    /// does nothing here.
    pub fn queue_podcasts_row(&self, item_id: &str) {
        let Some(lib_idx) = self.podcasts_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::PodcastsQueue {
            lib_idx,
            item_id: item_id.to_owned(),
        });
    }

    /// Move the list by one movement key's scroll (design D6): Up and Down move
    /// by the height of the row at the top edge, Page Up and Page Down by the
    /// viewport height, and Home and End go to the list's ends, inside the
    /// list's range.
    pub fn step_podcasts_scroll(&self, mv: ListMove) {
        let rows = self.podcasts_rows();
        let held = *self.scrolls.podcasts.peek();
        set_scroll_top(self.scrolls.podcasts, &rows, scroll_step(&rows, held, mv));
    }

    /// Report the list's measured viewport height (design D12): the rows in
    /// view follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_podcasts_list(&self, viewport_height: f32) {
        let mut podcasts = self.scrolls.podcasts;
        podcasts.with_mut(|scroll| scroll.viewport_height = viewport_height);
        self.clamp_podcasts_scroll();
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_podcasts_list(&self, scroll_top: f32) {
        let rows = self.podcasts_rows();
        set_scroll_top(self.scrolls.podcasts, &rows, scroll_top);
    }

    /// Hold the Podcasts list's scroll offset inside the rows the filter
    /// leaves.
    fn clamp_podcasts_scroll(&self) {
        let rows = self.podcasts_rows();
        let held = *self.scrolls.podcasts.peek();
        set_scroll_top(self.scrolls.podcasts, &rows, held.scroll_top);
    }

    /// Ask for a pill to be selected (design D6, D12): the pill row turns the
    /// request into the set-pill command.
    pub fn request_podcasts_pill_select(&self, pill: GuiPodcastPill) {
        let mut podcasts_ui = self.podcasts_ui;
        podcasts_ui.with_mut(|ui| ui.pill_select = Some(pill));
    }

    /// Select the view pill `pill` of the Podcasts library `lib_idx` (design
    /// D12, row 10.1): one pill is always selected, and Latest is the default
    /// (design D6). The core stores the choice and loads the pill's episodes,
    /// and the pills paint the core's choice back, so the row and the list
    /// cannot disagree.
    pub fn select_podcasts_pill(&self, lib_idx: usize, pill: GuiPodcastPill) {
        self.link
            .library(GuiLibraryCommand::PodcastsSetPill { lib_idx, pill });
    }

    /// Turn the keys' pill request into the set-pill command the pill sends
    /// (design D12). The intent is consumed here, so one key sends one
    /// command.
    pub fn apply_podcasts_pill_select(&self, lib_idx: usize) {
        let pill = self.podcasts_ui.peek().pill_select.clone();
        if let Some(pill) = pill {
            let mut podcasts_ui = self.podcasts_ui;
            podcasts_ui.with_mut(|ui| ui.pill_select = None);
            self.select_podcasts_pill(lib_idx, pill);
        }
    }
}
