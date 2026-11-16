//! The Feeds destination's interaction state (`add-gui-library-panel` design
//! D12, row 11.1, the port of `lib/library/feeds.ts`): what the window content
//! owns and the core never reads — the `/` filter's text and the pill row's
//! request. The scroll offset and the measured viewport live in the Feeds
//! list's scroll store ([`super::scroll`], design D9). The core owns the pill
//! selection and the selected entry (the frame's `pills` and `selected`), so
//! nothing here mirrors them: a press sends the select command, and the frame
//! carries the choice back. The row model is [`super::feeds_list`]'s; the Hero
//! overlay stays in [`super::state`]'s shared store. There is no play-next: `n`
//! is consumed and does nothing. A Client holds one feed tab, so no library
//! index rides the commands.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).
//!
//! The state, the filter, and the scroll store are the pill-list controller's
//! ([`super::pill_list`], row 7C.1); this module keeps the Feeds frame, the
//! Feeds rows, the Feeds command shapes, and the Feeds names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEntryRowFrame;
use crate::bridge::library::{GuiFeedPill, GuiLibraryCommand};
use crate::window::library::feeds_list::FeedsItemRow;
use crate::window::library::list::{ListMove, scroll_step};
use crate::window::library::parts::library_feeds_list;

use super::parts::library_feeds_detail;
use super::pill_list::PillListUiState;
use super::scroll::set_scroll_top;
use super::state::LibraryState;

/// The Feeds list's interaction state: the shared pill-list state over the
/// Feeds pill (design D12).
pub type FeedsUiState = PillListUiState<GuiFeedPill>;

/// The state a freshly mounted Feeds list starts with.
#[must_use]
pub fn initial_feeds_ui() -> FeedsUiState {
    PillListUiState::initial()
}

/// The pill's stable key: the each-block's identity, over the pill object the
/// frame carries (design D12).
#[must_use]
pub fn feed_pill_key(pill: &GuiFeedPill) -> String {
    match pill {
        GuiFeedPill::Latest => "latest".to_owned(),
        GuiFeedPill::Subscription(id) => format!("subscription:{id}"),
    }
}

impl LibraryState {
    /// The painted rows of the carried Feeds content (design D12), narrowed by
    /// the state's filter text. No carried Feeds content paints no rows. The
    /// rows borrow the section's carried frame through the memo's revision
    /// key — a keydown copies none of it.
    #[must_use]
    pub fn feeds_rows(&self) -> Arc<[FeedsItemRow]> {
        let list = self.sections.library_list.peek();
        let Some(feeds) = library_feeds_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let filter = self.feeds_ui.peek().filter.clone();
        self.feeds_rows_memo.rows(revision, &feeds.rows, &filter)
    }

    /// The row the pointer's marker names (design D1, D2): the marker carries
    /// the row's own id, so the painted rows resolve it and a stale or foreign
    /// id names no entry. `None` while no painted row carries the id.
    #[must_use]
    pub fn feeds_row_at(&self, row_id: &str) -> Option<FeedsItemRow> {
        self.feeds_rows()
            .iter()
            .find(|row| row.id == row_id)
            .cloned()
    }

    /// The Hero's feed entry the pointer's marker names (design D1, D2): the
    /// Hero's list band paints the carried rows whose subscription is the
    /// selected feed's, and the marker carries the entry's id, so a stale or
    /// foreign id names no entry. `None` while the Hero paints no such entry.
    #[must_use]
    pub fn feeds_hero_row_at(&self, row_id: &str) -> Option<GuiEntryRowFrame> {
        let detail_section = self.sections.library_detail.peek();
        let parent = library_feeds_detail(detail_section.as_deref())?
            .hero
            .as_ref()?
            .parent
            .clone()?;
        let list_section = self.sections.library_list.peek();
        let rows = library_feeds_list(list_section.as_deref())?;
        rows.rows
            .iter()
            .find(|row| row.id == row_id && row.subtitle.as_deref() == Some(parent.as_str()))
            .cloned()
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule). The filter narrows the rows and selects
    /// nothing, and the scroll stays inside the narrower list.
    pub fn set_feeds_filter(&self, filter: String) {
        let mut feeds_ui = self.feeds_ui;
        feeds_ui.with_mut(|ui| ui.filter = filter);
        self.clamp_feeds_scroll();
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it, so
    /// a closed filter narrows nothing. The scroll follows the widened rows.
    pub fn close_feeds_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut feeds_ui = self.feeds_ui;
        feeds_ui.with_mut(|ui| ui.filter.clear());
        self.clamp_feeds_scroll();
    }

    /// Select the row (design D12): the select command the core stores, whose
    /// frame paints the Hero. A select repaints the Narrow arrangement's Hero
    /// overlay and closes an open menu, because the facts they paint belong to
    /// the previous entry, as a Music pick's do.
    pub fn select_feeds_row(&self, row: &FeedsItemRow) {
        self.select_feeds_row_id(&row.row.id);
    }

    /// Select the row the id names (design D12): the same select by the row
    /// id alone, which the Hero Workspace table's press carries — its rows
    /// name the stable identity, never a `FeedsItemRow`.
    pub fn select_feeds_row_id(&self, row_id: &str) {
        self.link.library(GuiLibraryCommand::FeedsSelect {
            row: row_id.to_owned(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press an entry row (design D12): the press selects the entry the Hero
    /// shows.
    pub fn press_feeds_row(&self, row: &FeedsItemRow) {
        self.select_feeds_row(row);
    }

    /// Play an entry (design D12): Enter on the hovered entry, and a
    /// double-click's play. The queue is replaced, as the TUI's feeds play
    /// replaces it. The row id is the entry's stable identity the frame
    /// carried.
    pub fn play_feeds_row(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::FeedsPlay {
            item_id: item_id.to_owned(),
        });
    }

    /// Add an entry to the queue's tail (design D12): `a` on the hovered row.
    /// There is no play-next on the destination, so `n` is consumed and does
    /// nothing here.
    pub fn queue_feeds_row(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::FeedsQueue {
            item_id: item_id.to_owned(),
        });
    }

    /// Scroll the list by one movement key (design D6): Up and Down move by the
    /// height of the row at the top edge, Page Up and Page Down by the viewport
    /// height, and Home and End go to the list's ends, inside the list's range.
    pub fn step_feeds_scroll(&self, mv: ListMove) {
        let rows = self.feeds_rows();
        let held = *self.scrolls.feeds.peek();
        set_scroll_top(self.scrolls.feeds, &rows, scroll_step(&rows, held, mv));
    }

    /// Report the list's measured viewport height (design D12): the rows in
    /// view follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_feeds_list(&self, viewport_height: f32) {
        let mut feeds = self.scrolls.feeds;
        feeds.with_mut(|scroll| scroll.viewport_height = viewport_height);
        self.clamp_feeds_scroll();
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_feeds_list(&self, scroll_top: f32) {
        let rows = self.feeds_rows();
        set_scroll_top(self.scrolls.feeds, &rows, scroll_top);
    }

    /// Hold the Feeds list's scroll offset inside the rows the filter leaves.
    fn clamp_feeds_scroll(&self) {
        let rows = self.feeds_rows();
        let held = *self.scrolls.feeds.peek();
        set_scroll_top(self.scrolls.feeds, &rows, held.scroll_top);
    }

    /// Ask for a pill to be selected (design D6, D12): the pill row turns the
    /// request into the set-pill command.
    pub fn request_feeds_pill_select(&self, pill: GuiFeedPill) {
        let mut feeds_ui = self.feeds_ui;
        feeds_ui.with_mut(|ui| ui.pill_select = Some(pill));
    }

    /// Set the Feeds destination's pill `pill` (design D12, row 11.1): one
    /// pill is always selected, and Latest is the default (design D6). The
    /// core stores the choice and loads the pill's entries, and the pills
    /// paint the core's choice back, so the row and the list cannot disagree.
    pub fn select_feeds_pill(&self, pill: GuiFeedPill) {
        self.link.library(GuiLibraryCommand::FeedsSetPill { pill });
    }

    /// Turn the keys' pill request into the set-pill command the pill sends
    /// (design D12). The intent is consumed here, so one key sends one
    /// command.
    pub fn apply_feeds_pill_select(&self) {
        let pill = self.feeds_ui.peek().pill_select.clone();
        if let Some(pill) = pill {
            let mut feeds_ui = self.feeds_ui;
            feeds_ui.with_mut(|ui| ui.pill_select = None);
            self.select_feeds_pill(pill);
        }
    }
}
