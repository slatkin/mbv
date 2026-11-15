//! The Podcasts destination's interaction state (`add-gui-library-panel`
//! design D12, row 10.1, the port of `lib/library/podcasts.ts`): what the
//! window content owns and the core never reads — the cursor and the pill
//! row's focus. The scroll offset and the measured viewport live in the
//! Podcasts list's scroll store ([`super::scroll`], design D9). The core owns
//! the pill selection and the selected episode (the frame's `pills` and
//! `selected`), so nothing here mirrors them: a press sends the select
//! command, and the frame carries the choice back. The row model is
//! [`super::podcasts_list`]'s; the panel's pane focus and the Hero overlay
//! stay in [`super::state`]'s shared store. There is no play-next: `n` is
//! consumed and does nothing.
//!
//! The state, the filter, the cursor, the scroll store, and the pill focus are
//! the pill-list controller's ([`super::pill_list`], row 7C.1); this module
//! keeps the Podcasts frame, the Podcasts rows, the Podcasts command shapes,
//! and the Podcasts names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiPodcastsList;
use crate::bridge::library::{GuiLibraryCommand, GuiPodcastPill, GuiPodcastState};
use crate::window::library::list::ListMove;
use crate::window::library::parts::library_podcasts_list;
use crate::window::library::podcasts_list::{PodcastsItemRow, move_podcasts_cursor};

use super::pill_list::{ListControl, PillListUiState, move_pill_focus};
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

    /// The row the Podcasts cursor rests on, or `None`: the keys read it to
    /// play the episode or queue it. Every row is an item row, so no heading
    /// stands in the find.
    #[must_use]
    pub fn podcasts_cursor_row(&self) -> Option<PodcastsItemRow> {
        let rows = self.podcasts_rows();
        let cursor = self.podcasts_ui.peek().cursor.clone();
        rows.iter()
            .find(|row| Some(row.id.as_str()) == cursor.as_deref())
            .cloned()
    }

    /// The Podcasts list under the shared list controller (design D12).
    fn podcasts_list(
        &self,
    ) -> ListControl<PodcastsUiState, PodcastsItemRow, impl Fn() -> Arc<[PodcastsItemRow]> + '_>
    {
        ListControl {
            ui: self.podcasts_ui,
            scroll: self.scrolls.podcasts,
            rows: || self.podcasts_rows(),
            move_over: move_podcasts_cursor,
        }
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule): the cursor follows the rows.
    pub fn set_podcasts_filter(&self, filter: String) {
        self.podcasts_list().set_filter(filter);
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it.
    pub fn close_podcasts_filter(&self) {
        self.podcasts_list().close_filter(self.ui);
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

    /// Play an episode (design D12): Enter on the cursor row, and a
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

    /// Add an episode to the queue's tail (design D12): `a` on the cursor
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

    /// Move the list's cursor one movement along (design D12): Up and Down
    /// move a row, Page Up and Page Down move the rows in view, and Home and
    /// End take the list's ends. The scroll offset follows, so the cursor row
    /// stays fully visible.
    pub fn move_podcasts_list_cursor(&self, mv: ListMove) {
        self.podcasts_list().move_cursor(mv);
    }

    /// Report the list's measured viewport height (design D12): the rows in
    /// view follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_podcasts_list(&self, viewport_height: f32) {
        self.podcasts_list().measure(viewport_height);
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_podcasts_list(&self, scroll_top: f32) {
        self.podcasts_list().scroll_to(scroll_top);
    }

    /// The pill the pill row's focus holds, resolved against the carried
    /// pills so a stale pill focuses nothing: the pill the keys' Space and
    /// Enter select, and the pill the row paints the accent outline on
    /// (design D6, D12).
    #[must_use]
    pub fn focused_podcasts_pill<'a>(
        &self,
        podcasts: &'a GuiPodcastsList,
    ) -> Option<&'a GuiPodcastPill> {
        let focused = self.podcasts_ui.peek().focused_pill.clone()?;
        let focused_key = podcast_pill_key(&focused);
        podcasts
            .pills
            .iter()
            .map(|frame| &frame.pill)
            .find(|pill| podcast_pill_key(pill) == focused_key)
    }

    /// Move the focused pill one pill along the row (design D12): Right and
    /// Left walk the pills, and the first of them with no pill focused takes
    /// the end it walks from.
    pub fn move_podcasts_pill_focus(&self, step: i32) {
        let list = self.sections.library_list.peek();
        let Some(podcasts) = library_podcasts_list(list.as_deref()) else {
            return;
        };
        let pills: Vec<GuiPodcastPill> = podcasts
            .pills
            .iter()
            .map(|frame| frame.pill.clone())
            .collect();
        move_pill_focus(
            self.podcasts_ui,
            &pills,
            podcast_pill_key,
            Clone::clone,
            podcast_pill_key,
            step,
        );
    }

    /// Ask for the focused pill to be selected: Space and Enter request it
    /// (design D6, D12).
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
