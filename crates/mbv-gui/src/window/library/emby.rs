//! The plain Emby-library destination's interaction state
//! (`add-gui-library-panel` design D12, row 8.1, the port of
//! `lib/library/emby.ts`): what the window content owns and the core never
//! reads — the cursor and the feed-group pill row's focus. The scroll offset
//! and the measured viewport live in the Emby list's scroll store
//! ([`super::scroll`], design D9). The core owns the selected row and the
//! feed-group selection (the frame's `selected` and `pills`), so nothing here
//! mirrors them: a press and an Enter send the command, and the frame carries
//! the choice back. The row model is [`super::emby_list`]'s; the panel's pane
//! focus and the Hero overlay stay in [`super::state`]'s shared store, and the
//! Hero carries no Workspace and no cursor of its own.
//!
//! The state, the filter, the cursor, the scroll store, and the pill focus are
//! the pill-list controller's ([`super::pill_list`], row 7C.1); this module
//! keeps the Emby frame, the Emby rows and their letter headings, the letter
//! rail, the play-next command, the Emby command shapes, and the Emby names.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiEmbyFeedGroupFrame, GuiEmbyList};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::emby_list::{
    EmbyItemRow, EmbyListRow, emby_group_key, emby_letter_rail, move_emby_cursor,
};
use crate::window::library::list::{ListMove, jump_to_letter};
use crate::window::library::parts::library_emby_list;

use super::pill_list::{ListControl, PillListUiState, move_pill_focus};
use super::state::LibraryState;

/// The Emby list's interaction state: the shared pill-list state over the
/// feed-group pill, whose focus the pill row keeps as the group's stable key
/// (`embyGroupKey`) rather than the group itself (design D6, row 8.1).
pub type EmbyUiState = PillListUiState<GuiEmbyFeedGroupFrame, String>;

/// The state a freshly mounted Emby list starts with.
#[must_use]
pub fn initial_emby_ui() -> EmbyUiState {
    PillListUiState::initial()
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

    /// The row the Emby cursor rests on, or `None` (row 8.1): the keys read it
    /// to play the row, play it next, or queue it.
    #[must_use]
    pub fn emby_cursor_row(&self) -> Option<EmbyItemRow> {
        let rows = self.emby_rows();
        let cursor = self.emby_ui.peek().cursor.clone();
        rows.iter().find_map(|row| match row {
            EmbyListRow::Item(item) if Some(item.id.as_str()) == cursor.as_deref() => {
                Some(item.clone())
            }
            _ => None,
        })
    }

    /// The Emby list under the shared list controller (row 8.1).
    fn emby_list(
        &self,
    ) -> ListControl<EmbyUiState, EmbyListRow, impl Fn() -> Arc<[EmbyListRow]> + '_> {
        ListControl {
            ui: self.emby_ui,
            scroll: self.scrolls.emby,
            rows: || self.emby_rows(),
            move_over: move_emby_cursor,
        }
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule): the cursor follows the rows.
    pub fn set_emby_filter(&self, filter: String) {
        self.emby_list().set_filter(filter);
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it.
    pub fn close_emby_filter(&self) {
        self.emby_list().close_filter(self.ui);
    }

    /// Select the row (row 8.1): the select command the core stores, whose
    /// frame paints the Hero. A select repaints the Narrow arrangement's Hero
    /// overlay and closes an open menu, because the facts they paint belong to
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
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press a row (row 8.1): the press selects the row the Hero shows, as a
    /// Home press does. A heading names no row, so its press changes nothing.
    pub fn press_emby_row(&self, row: &EmbyListRow) {
        self.select_emby_row(row);
    }

    /// Play a row (row 8.1): Enter on the cursor row, and a double-click's
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

    /// Play a row next (row 8.1): `n` on the cursor row.
    pub fn play_emby_row_next(&self, row_id: &str) {
        let Some(lib_idx) = self.emby_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::EmbyPlayNext {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// Add a row to the queue's tail (row 8.1): `a` on the cursor row.
    pub fn queue_emby_row(&self, row_id: &str) {
        let Some(lib_idx) = self.emby_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::EmbyQueue {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// Move the list's cursor one movement along (row 8.1): Up and Down move a
    /// row, Page Up and Page Down move the rows in view, and Home and End take
    /// the list's ends, stepping over the letter headings. The scroll offset
    /// follows, so the cursor row stays fully visible.
    pub fn move_emby_list_cursor(&self, mv: ListMove) {
        self.emby_list().move_cursor(mv);
    }

    /// Jump to a letter of the rail (row 8.1, design D7): the cursor moves to
    /// the first row under the letter and that row scrolls into view. A letter
    /// without rows changes nothing.
    pub fn jump_emby_to_letter(&self, letter: char) {
        let rows = self.emby_rows();
        let view = *self.scrolls.emby.peek();
        let Some((cursor, scroll_top)) =
            jump_to_letter(&emby_letter_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        let mut emby_ui = self.emby_ui;
        emby_ui.with_mut(|ui| ui.cursor = Some(cursor));
        let mut emby_scroll = self.scrolls.emby;
        emby_scroll.with_mut(|scroll| scroll.scroll_top = scroll_top);
    }

    /// Report the list's measured viewport height (row 8.1): the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_emby_list(&self, viewport_height: f32) {
        self.emby_list().measure(viewport_height);
    }

    /// Report the list's native scroll offset (row 8.1): a user scroll moves
    /// the rows in view, and the offset stays inside the list's range.
    pub fn scroll_emby_list(&self, scroll_top: f32) {
        self.emby_list().scroll_to(scroll_top);
    }

    /// The pill the pill row's focus holds, resolved against the carried pills
    /// so a stale group focuses nothing (row 8.1).
    #[must_use]
    pub fn focused_emby_pill<'a>(
        &self,
        emby: &'a GuiEmbyList,
    ) -> Option<&'a GuiEmbyFeedGroupFrame> {
        let focused = self.emby_ui.peek().focused_pill.clone()?;
        emby.pills
            .iter()
            .map(|pill| &pill.group)
            .find(|group| emby_group_key(group) == focused)
    }

    /// Move the focused pill one pill along the row (row 8.1): Right and Left
    /// walk the pills, and the first of them with no pill focused takes the end
    /// it walks from.
    pub fn move_emby_pill_focus(&self, step: i32) {
        let list = self.sections.library_list.peek();
        let Some(emby) = library_emby_list(list.as_deref()) else {
            return;
        };
        let pills: Vec<GuiEmbyFeedGroupFrame> =
            emby.pills.iter().map(|pill| pill.group.clone()).collect();
        move_pill_focus(
            self.emby_ui,
            &pills,
            emby_group_key,
            emby_group_key,
            String::clone,
            step,
        );
    }

    /// Ask for the focused pill to be selected: Space and Enter request it
    /// (row 8.1).
    pub fn request_emby_pill_select(&self, group: GuiEmbyFeedGroupFrame) {
        let mut emby_ui = self.emby_ui;
        emby_ui.with_mut(|ui| ui.pill_select = Some(group));
    }

    /// Select the feed group `group` of the plain Emby library `lib_idx` (row
    /// 8.1): one group is always selected, and a press on a pill loads that
    /// group's items. The core stores the choice, and the pills paint it back,
    /// so the row and the list cannot disagree.
    pub fn select_emby_pill(&self, lib_idx: usize, group: GuiEmbyFeedGroupFrame) {
        self.link.library(GuiLibraryCommand::EmbySetFeedGroup {
            lib_idx,
            group: group.into(),
        });
    }

    /// Turn the keys' pill request into the set-feed-group command the pill
    /// sends (row 8.1). The intent is consumed here, so one key sends one
    /// command.
    pub fn apply_emby_pill_select(&self, lib_idx: usize) {
        let group = self.emby_ui.peek().pill_select.clone();
        if let Some(group) = group {
            let mut emby_ui = self.emby_ui;
            emby_ui.with_mut(|ui| ui.pill_select = None);
            self.select_emby_pill(lib_idx, group);
        }
    }
}
