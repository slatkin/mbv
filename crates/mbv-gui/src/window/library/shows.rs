//! The Shows destination's interaction state (`add-gui-library-panel` design
//! D14, the port of `lib/library/shows.ts`): what the window content owns and
//! the core never reads — the tree's open and closed nodes and the `/` filter's
//! text. The scroll offset and the measured viewport live in the Shows list's
//! scroll store ([`super::scroll`], design D9). The core owns the selection (the
//! frame's `selected`), so nothing here mirrors it: a press and an Enter send the
//! select command, and the frame carries the selection back. The GUI Shows list
//! is the series list alone (gui-collection-dropdown design D8), so no view-mode
//! request lives here. The row model is [`super::shows_list`]'s, and the Hero
//! overlay stays in [`super::state`]'s shared store.
//!
//! The list keeps no cursor: the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), and their movement keys scroll the
//! list (design D6).

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::list::{ListMove, jump_to_letter, scroll_step};
use crate::window::library::parts::library_shows_list;
use crate::window::library::scroll::set_scroll_top;
use crate::window::library::shows_list::{
    ShowsListRow, ShowsRowsFacts, ShowsTreeState, build_shows_rows, shows_letter_rail,
};

use super::state::{LibraryState, RowHoverPresses};

/// The Shows list's interaction state (`shows.ts`'s `ShowsUiState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShowsUiState {
    /// The `/` filter's text: the list paints only the rows whose title
    /// contains it, without case (design D7's filter rule, the rail's
    /// keyboard path).
    pub filter: String,
    /// The tree's open node keys (design D14): the series and season rows the
    /// user opened, the Music tree's rule.
    pub open: Vec<String>,
    /// The node keys the user closed while the filter narrows the rows: they
    /// override the filter's auto-open, so a press still closes a node under a
    /// filter (the Music tree's `closed` rule).
    pub closed: Vec<String>,
}

impl ShowsUiState {
    /// The state a freshly mounted Shows list starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self::default()
    }
}

impl LibraryState {
    /// The Shows library index the Shows commands name, or `None` while
    /// another destination holds the window. The frame is the section's
    /// carried value; a keydown copies only the index.
    fn shows_lib_idx(&self) -> Option<usize> {
        let list = self.sections.library_list.peek();
        library_shows_list(list.as_deref()).map(|shows| shows.lib_idx)
    }

    /// The painted rows of the carried Shows content (design D14), narrowed by
    /// the state's filter text. No carried Shows content paints no rows. The
    /// rows borrow the section's carried frame through the memo's revision
    /// key — a keydown copies none of it.
    #[must_use]
    pub fn shows_rows(&self) -> Arc<[ShowsListRow]> {
        let list = self.sections.library_list.peek();
        let Some(shows) = library_shows_list(list.as_deref()) else {
            return Arc::from([]);
        };
        let revision = self.sections.library_list_revision();
        let ui = self.shows_ui.peek();
        self.shows_rows_memo.rows(
            revision,
            ShowsRowsFacts {
                view: shows.view,
                filter: ui.filter.clone(),
                open: ui.open.clone(),
                closed: ui.closed.clone(),
            },
            |facts| {
                build_shows_rows(
                    facts.view,
                    &shows.series,
                    &shows.rows,
                    &facts.filter,
                    ShowsTreeState {
                        open: &facts.open,
                        closed: &facts.closed,
                    },
                )
            },
        )
    }

    /// The row the pointer's marker names (design D1, D2): the marker carries
    /// the row's own id, so the painted rows resolve it and a stale or foreign
    /// id names no row. The keys read the row's kind to decide whether Enter
    /// selects it or plays it. `None` while no painted row carries the id — a
    /// letter heading carries none at all.
    #[must_use]
    pub fn shows_row_at(&self, row_id: &str) -> Option<ShowsListRow> {
        let rows = self.shows_rows();
        rows.iter().find(|row| row.id() == row_id).cloned()
    }

    /// Narrow the list to the titles that contain `filter`, without case
    /// (design D7's filter rule). The filter narrows the rows and selects
    /// nothing, and the scroll stays inside the narrower list.
    pub fn set_shows_filter(&self, filter: String) {
        let mut shows_ui = self.shows_ui;
        shows_ui.with_mut(|ui| ui.filter = filter);
        self.clamp_shows_scroll();
    }

    /// Close the filter with Esc (design D3, step 2): the text goes with it, so
    /// a closed filter narrows nothing. The scroll follows the widened rows.
    pub fn close_shows_filter(&self) {
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.filter_open = false);
        let mut shows_ui = self.shows_ui;
        shows_ui.with_mut(|ui| ui.filter.clear());
        self.clamp_shows_scroll();
    }

    /// Select the row (design D14): a series sends the series select (the core
    /// loads its detail), a season sends the season select (the core loads its
    /// episodes), and a flat row sends the flat select — the command whose
    /// stored value the Hero paints. An episode leaf changes nothing in the
    /// core: the series Hero owns its episode Workspace. A select never
    /// opens the Narrow arrangement's Hero overlay (only View Detail does) and
    /// closes an open menu, because
    /// the facts they paint belong to the previous row, as a Music pick's do.
    /// A heading names no row, so its select changes nothing.
    pub fn select_shows_row(&self, row: &ShowsListRow) {
        let Some(lib_idx) = self.shows_lib_idx() else {
            return;
        };
        let command = match row {
            // A heading and an episode leaf name no series to select.
            ShowsListRow::Heading(_) | ShowsListRow::Episode(_) => return,
            ShowsListRow::Flat(flat) => {
                if flat.display_only {
                    return;
                }
                GuiLibraryCommand::ShowsSelect {
                    lib_idx,
                    row: flat.id.clone(),
                }
            }
            ShowsListRow::Series(series) => GuiLibraryCommand::ShowsSelect {
                lib_idx,
                row: series.source_id.clone(),
            },
            ShowsListRow::Season(season) => GuiLibraryCommand::ShowsSelectSeason {
                lib_idx,
                season_id: season.source_id.clone(),
            },
        };
        self.link.library(command);
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Whether the state has the node open (design D14).
    #[must_use]
    pub fn shows_node_open(&self, node_id: &str) -> bool {
        let ui = self.shows_ui.peek();
        ui.open.iter().any(|id| id == node_id) && !ui.closed.iter().any(|id| id == node_id)
    }

    /// Open a node (design D14): the row paints its children, and a series or
    /// a season whose children have not landed asks the core for them through
    /// the select the press already sends.
    pub fn open_shows_node(&self, node_id: &str) {
        let mut shows_ui = self.shows_ui;
        shows_ui.with_mut(|ui| {
            if !ui.open.iter().any(|id| id == node_id) {
                ui.open.push(node_id.to_owned());
            }
            ui.closed.retain(|id| id != node_id);
        });
    }

    /// Close a node (design D14).
    pub fn close_shows_node(&self, node_id: &str) {
        let mut shows_ui = self.shows_ui;
        shows_ui.with_mut(|ui| {
            ui.open.retain(|id| id != node_id);
            if !ui.closed.iter().any(|id| id == node_id) {
                ui.closed.push(node_id.to_owned());
            }
        });
    }

    /// Toggle a node (design D14): a double-click on the row and the keys'
    /// Enter, Right, and Left run this (gui-tree-double-click design D1). The
    /// children follow the toggle; the select is the press's own, so a
    /// double-click after its two picks leaves the node the user is looking at
    /// selected.
    pub fn toggle_shows_node(&self, node_id: &str) {
        if self.shows_node_open(node_id) {
            self.close_shows_node(node_id);
        } else {
            self.open_shows_node(node_id);
        }
    }

    /// Open every painted series row (gui-list-header-buttons design D4, spec
    /// "Expand all opens every top-level parent row the list shows"): no
    /// season opens, and the ids leave the filter's closed set, so the series
    /// stay open after the filter clears. The press sends no command and picks
    /// nothing, so the Hero stays as it was.
    pub fn expand_all_shows(&self) {
        let series: Vec<String> = self
            .shows_rows()
            .iter()
            .filter_map(|row| match row {
                ShowsListRow::Series(series) => Some(series.id.clone()),
                ShowsListRow::Heading(_)
                | ShowsListRow::Season(_)
                | ShowsListRow::Episode(_)
                | ShowsListRow::Flat(_) => None,
            })
            .collect();
        let mut shows_ui = self.shows_ui;
        shows_ui.with_mut(|ui| {
            for id in &series {
                if !ui.open.iter().any(|open| open == id) {
                    ui.open.push(id.clone());
                }
                ui.closed.retain(|closed| closed != id);
            }
        });
    }

    /// Close every open series and season row (gui-list-header-buttons design
    /// D4, spec "Collapse all closes every open parent row"): every painted
    /// node also enters the filter's closed set while the filter narrows the
    /// rows, so the rows the filter opened stay shut after it clears. The press
    /// sends no command and picks nothing, so the Hero stays as it was.
    pub fn collapse_all_shows(&self) {
        let rows = self.shows_rows();
        let narrowing = !self.shows_ui.peek().filter.trim().is_empty();
        let nodes: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                ShowsListRow::Series(series) => Some(series.id.clone()),
                ShowsListRow::Season(season) => Some(season.id.clone()),
                ShowsListRow::Heading(_) | ShowsListRow::Episode(_) | ShowsListRow::Flat(_) => None,
            })
            .collect();
        let mut shows_ui = self.shows_ui;
        shows_ui.with_mut(|ui| {
            ui.open.clear();
            if narrowing {
                let absent: Vec<String> = nodes
                    .iter()
                    .filter(|id| !ui.closed.contains(*id))
                    .cloned()
                    .collect();
                ui.closed.extend(absent);
            }
        });
    }

    /// Press a Shows row (design D14): a press picks a node, a flat row, or an
    /// episode leaf, and nothing else. The open state belongs to the
    /// double-click ([`Self::toggle_shows_node`]), so a press on a series or
    /// season node leaves the node as it was (gui-tree-double-click design D1).
    /// A heading names no row, so its press changes nothing.
    pub fn press_shows_row(&self, row: &ShowsListRow) {
        self.select_shows_row(row);
    }

    /// Play a Shows episode (design D12): Enter on a flat row, and a
    /// double-click's play. The row id is the episode's stable Service id.
    pub fn play_shows_row(&self, row_id: &str) {
        let Some(lib_idx) = self.shows_lib_idx() else {
            return;
        };
        if self.shows_row_at(row_id).is_some_and(|row| {
            matches!(
                row,
                ShowsListRow::Flat(super::shows_list::ShowsFlatRow {
                    display_only: true,
                    ..
                })
            )
        }) {
            return;
        }
        self.link.library(GuiLibraryCommand::ShowsPlay {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// Add a Shows episode to the queue's tail (design D12): `a` on a flat
    /// row.
    pub fn queue_shows_row(&self, row_id: &str) {
        let Some(lib_idx) = self.shows_lib_idx() else {
            return;
        };
        if self.shows_row_at(row_id).is_some_and(|row| {
            matches!(
                row,
                ShowsListRow::Flat(super::shows_list::ShowsFlatRow {
                    display_only: true,
                    ..
                })
            )
        }) {
            return;
        }
        self.link.library(GuiLibraryCommand::ShowsQueue {
            lib_idx,
            item_id: row_id.to_owned(),
        });
    }

    /// The hover controls' presses for a Shows row that names `item_id`
    /// (gui-row-hover-actions design D3, D4): Play and Add run the same commands
    /// the keys run, and Shuffle runs the row's own right-click shuffle entry, on
    /// the rows whose menu carries one (`offers_shuffle`: a series and a season,
    /// never an episode or a flat row, which name no collection). A display-only
    /// row takes none of them.
    #[must_use]
    pub fn shows_row_hover_presses(
        &self,
        item_id: String,
        offers_shuffle: bool,
    ) -> RowHoverPresses {
        self.item_hover_presses(
            item_id,
            Self::play_shows_row,
            Self::queue_shows_row,
            offers_shuffle,
        )
    }

    /// Scroll the list by one movement key (design D6): Up and Down move by the
    /// height of the row at the top edge, Page Up and Page Down by the viewport
    /// height, and Home and End go to the list's ends, inside the list's range.
    pub fn step_shows_scroll(&self, mv: ListMove) {
        let rows = self.shows_rows();
        let held = *self.scrolls.shows.peek();
        set_scroll_top(self.scrolls.shows, &rows, scroll_step(&rows, held, mv));
    }

    /// Jump to a letter of the rail (design D12): the list scrolls so the first
    /// series row under the letter paints fully. A letter without rows changes
    /// nothing.
    pub fn jump_shows_to_letter(&self, letter: char) {
        let rows = self.shows_rows();
        let view = *self.scrolls.shows.peek();
        let Some((_, scroll_top)) = jump_to_letter(&shows_letter_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        set_scroll_top(self.scrolls.shows, &rows, scroll_top);
    }

    /// Report the list's measured viewport height (design D12): the rows in
    /// view follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_shows_list(&self, viewport_height: f32) {
        let mut shows = self.scrolls.shows;
        shows.with_mut(|scroll| scroll.viewport_height = viewport_height);
        self.clamp_shows_scroll();
    }

    /// Report the list's native scroll offset (design D12): a user scroll
    /// moves the rows in view, and the offset stays inside the list's range.
    pub fn scroll_shows_list(&self, scroll_top: f32) {
        let rows = self.shows_rows();
        set_scroll_top(self.scrolls.shows, &rows, scroll_top);
    }

    /// Report the open Shows nodes the All view paints, in paint order
    /// (gui-list-header-buttons design D5): the source id of each open series
    /// row and the `(series_id, source_id)` pair of each open season row. The
    /// core starts the bounded in-view loads the report misses, and each report
    /// replaces the last, so a row that leaves the view leaves the report. A
    /// state that paints no Shows library sends nothing, because only a Shows
    /// destination carries the library index the command names.
    pub fn report_shows_open_in_view(&self, series: Vec<String>, seasons: Vec<(String, String)>) {
        let Some(lib_idx) = self.shows_lib_idx() else {
            return;
        };
        self.link.library(GuiLibraryCommand::ShowsOpenInView {
            lib_idx,
            series,
            seasons,
        });
    }

    /// Hold the Shows list's scroll offset inside the rows the filter leaves.
    fn clamp_shows_scroll(&self) {
        let rows = self.shows_rows();
        let held = *self.scrolls.shows.peek();
        set_scroll_top(self.scrolls.shows, &rows, held.scroll_top);
    }
}

#[cfg(test)]
mod tests;
