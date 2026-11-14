//! The Feeds destination's interaction state (`add-gui-library-panel` design
//! D12, row 11.1, the port of `lib/library/feeds.ts`): what the window
//! content owns and the core never reads — the cursor and the pill row's
//! focus. The scroll offset and the measured viewport live in the Feeds
//! list's scroll store ([`super::scroll`], design D9). The core owns the pill
//! selection and the selected entry (the frame's `pills` and `selected`), so
//! nothing here mirrors them: a press sends the select command, and the frame
//! carries the choice back. The row model is [`super::feeds_list`]'s; the
//! panel's pane focus and the Hero overlay stay in [`super::state`]'s shared
//! store. There is no play-next: `n` is consumed and does nothing. A Client
//! holds one feed tab, so no library index rides the commands.
//!
//! The state, the filter, the cursor, the scroll store, and the pill focus are
//! the pill-list controller's ([`super::pill_list`], row 7C.1); this module
//! keeps the Feeds frame, the Feeds rows, the Feeds command shapes, and the
//! Feeds names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiFeedsList;
use crate::bridge::library::{GuiFeedPill, GuiLibraryCommand};
use crate::window::library::feeds_list::{FeedsItemRow, move_feeds_cursor};
use crate::window::library::list::{
    ListMove, clamp_scroll_top, list_window, scroll_top_to_show_cursor, step_pill_focus,
};
use crate::window::library::parts::library_feeds_list;
use crate::window::library::scroll::ListScroll;

use super::pill_list::PillListUiState;
use super::state::LibraryState;

/// The Feeds list's interaction state: the shared pill-list state over the
/// Feeds pill (design D12).
pub type FeedsUiState = PillListUiState<GuiFeedPill>;

/// The state a freshly mounted Feeds list starts with.
#[must_use]
pub fn initial_feeds_ui() -> FeedsUiState {
    PillListUiState::initial()
}

/// The pill's stable key: the each-block's identity and the pill row's focus
/// key, over the pill object the frame carries (design D12).
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
    pub(super) fn feeds_rows(&self) -> Arc<[FeedsItemRow]> {
        let list = self.sections.library_list.peek();
        let Some(feeds) = library_feeds_list(list.as_ref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let filter = self.feeds_ui.peek().filter.clone();
        self.feeds_rows_memo.rows(revision, &feeds.rows, &filter)
    }

    /// The row the Feeds cursor rests on, or `None`: the keys read it to play
    /// the entry or queue it. Every row is an item row, so no heading stands
    /// in the find.
    #[must_use]
    pub fn feeds_cursor_row(&self) -> Option<FeedsItemRow> {
        let rows = self.feeds_rows();
        let cursor = self.feeds_ui.peek().cursor.clone();
        rows.iter()
            .find(|row| Some(row.id.as_str()) == cursor.as_deref())
            .cloned()
    }

    /// Scroll the list so the cursor's row paints fully, in the scroll store
    /// (design D12, D9).
    fn follow_feeds_cursor(&self, rows: &[FeedsItemRow], cursor: Option<&str>) {
        let held = *self.scrolls.feeds.peek();
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
            let mut feeds_scroll = self.scrolls.feeds;
            feeds_scroll.set(shown);
        }
    }

    /// Settle the cursor once the painted rows may have narrowed (design
    /// D12): the cursor keeps its row while that row still paints, and falls
    /// to the first painted row otherwise. The scroll store then shows the
    /// cursor — the Music filter's own resolve (design D7).
    fn settle_feeds_cursor(&self) {
        let ui = self.feeds_ui.peek().clone();
        let rows = self.feeds_rows();
        let kept = ui
            .cursor
            .as_deref()
            .is_some_and(|cursor| rows.iter().any(|row| row.id == cursor));
        let cursor = if kept {
            ui.cursor.clone()
        } else {
            move_feeds_cursor(&rows, None, ListMove::Home, 1)
        };
        if cursor != ui.cursor {
            let mut feeds_ui = self.feeds_ui;
            feeds_ui.with_mut(|current| current.cursor.clone_from(&cursor));
        }
        self.follow_feeds_cursor(&rows, cursor.as_deref());
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule): the cursor follows the rows.
    pub fn set_feeds_filter(&self, filter: String) {
        let mut feeds_ui = self.feeds_ui;
        feeds_ui.with_mut(|ui| ui.filter = filter);
        self.settle_feeds_cursor();
    }

    /// Close the filter with Esc: the text goes with it, so a closed filter
    /// narrows nothing (design D3, step 2). The cursor follows the widened
    /// rows.
    pub fn close_feeds_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut feeds_ui = self.feeds_ui;
        feeds_ui.with_mut(|ui| ui.filter.clear());
        self.settle_feeds_cursor();
    }

    /// Accept the filter with Enter: the field closes and the list keeps the
    /// text (design D3, step 2).
    pub fn accept_feeds_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
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

    /// Play an entry (design D12): Enter on the cursor row, and a
    /// double-click's play. The queue is replaced, as the TUI's feeds play
    /// replaces it. The row id is the entry's stable identity the frame
    /// carried.
    pub fn play_feeds_row(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::FeedsPlay {
            item_id: item_id.to_owned(),
        });
    }

    /// Add an entry to the queue's tail (design D12): `a` on the cursor row.
    /// There is no play-next on the destination, so `n` is consumed and does
    /// nothing here.
    pub fn queue_feeds_row(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::FeedsQueue {
            item_id: item_id.to_owned(),
        });
    }

    /// Move the list's cursor one movement along (design D12): Up and Down
    /// move a row, Page Up and Page Down move the rows in view, and Home and
    /// End take the list's ends. The scroll offset follows, so the cursor row
    /// stays fully visible.
    pub fn move_feeds_list_cursor(&self, mv: ListMove) {
        let ui = self.feeds_ui.peek().clone();
        let rows = self.feeds_rows();
        let held = *self.scrolls.feeds.peek();
        let view = list_window(&rows, held.scroll_top, held.viewport_height);
        let cursor = move_feeds_cursor(&rows, ui.cursor.as_deref(), mv, view.end - view.start);
        if cursor == ui.cursor {
            return;
        }
        let mut feeds_ui = self.feeds_ui;
        feeds_ui.with_mut(|current| current.cursor.clone_from(&cursor));
        self.follow_feeds_cursor(&rows, cursor.as_deref());
    }

    /// Report the list's measured viewport height (design D12): the rows in
    /// view follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_feeds_list(&self, viewport_height: f32) {
        let rows = self.feeds_rows();
        let held = *self.scrolls.feeds.peek();
        let shown = ListScroll {
            viewport_height,
            scroll_top: clamp_scroll_top(&rows, held.scroll_top, viewport_height),
        };
        if shown != held {
            let mut feeds_scroll = self.scrolls.feeds;
            feeds_scroll.set(shown);
        }
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_feeds_list(&self, scroll_top: f32) {
        let rows = self.feeds_rows();
        let held = *self.scrolls.feeds.peek();
        let shown = ListScroll {
            scroll_top: clamp_scroll_top(&rows, scroll_top, held.viewport_height),
            viewport_height: held.viewport_height,
        };
        if shown != held {
            let mut feeds_scroll = self.scrolls.feeds;
            feeds_scroll.set(shown);
        }
    }

    /// The pill the pill row's focus holds, resolved against the carried
    /// pills so a stale pill focuses nothing: the pill the keys' Space and
    /// Enter select, and the pill the row paints the accent outline on
    /// (design D6, D12).
    #[must_use]
    pub fn focused_feeds_pill<'a>(&self, feeds: &'a GuiFeedsList) -> Option<&'a GuiFeedPill> {
        let focused = self.feeds_ui.peek().focused_pill.clone()?;
        let focused_key = feed_pill_key(&focused);
        feeds
            .pills
            .iter()
            .map(|frame| &frame.pill)
            .find(|pill| feed_pill_key(pill) == focused_key)
    }

    /// Move the focused pill one pill along the row (design D12): Right and
    /// Left walk the pills, and the first of them with no pill focused takes
    /// the end it walks from.
    pub fn move_feeds_pill_focus(&self, step: i32) {
        let list = self.sections.library_list.peek();
        let Some(feeds) = library_feeds_list(list.as_ref()) else {
            return;
        };
        let pills: Vec<GuiFeedPill> = feeds.pills.iter().map(|frame| frame.pill.clone()).collect();
        let focused = self.feeds_ui.peek().focused_pill.clone();
        let focused_key = focused.as_ref().map(feed_pill_key);
        let moved = step_pill_focus(&pills, focused_key.as_deref(), feed_pill_key, step);
        if let Some(pill) = moved {
            let mut feeds_ui = self.feeds_ui;
            feeds_ui.with_mut(|ui| ui.focused_pill = Some(pill));
        }
    }

    /// Ask for the focused pill to be selected: Space and Enter request it
    /// (design D6, D12).
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
