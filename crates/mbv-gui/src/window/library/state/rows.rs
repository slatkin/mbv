//! The Library list's row model and scroll geometry over the interaction
//! state (`replace-tauri-with-blitz` D15, the seam of `state.ts`'s row-model
//! half): the memoized painted rows, the press rules, the reveal, the filter,
//! and the scroll reports. The interaction state lives in [`super`].

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMusicDetail, GuiMusicList};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::list::{
    LibraryRow, LibraryTree, ListMove, MusicRowsFacts, album_row_id, artist_row_id,
    build_library_rows, jump_to_letter, letter_rail, scroll_step, scroll_top_to_show_cursor,
};
use crate::window::library::parts::library_music_list;
use crate::window::library::scroll::set_scroll_top;

use super::{LibraryState, LibraryUiState};

/// The id list with `id` present when `present` and absent otherwise, keeping
/// the other ids' order.
fn with_id(ids: &mut Vec<String>, id: &str, present: bool) {
    ids.retain(|candidate| candidate != id);
    if present {
        ids.push(id.to_owned());
    }
}

impl LibraryState {
    /// The row model under one interaction state (design D7): the carried
    /// union, the open nodes, and the filter text. The union borrows from the
    /// section store's own storage — a scroll step copies none of it — and the
    /// memo keys it on the section's revision, so its hit test never compares
    /// the union (design D6).
    #[must_use]
    pub fn rows_of(&self, ui: &LibraryUiState) -> Arc<[LibraryRow]> {
        let list = self.sections.library_list.peek();
        let artists =
            library_music_list(list.as_deref()).map_or(&[][..], |music| music.artists.as_slice());
        self.rows_memo.rows(
            self.sections.library_list_revision(),
            MusicRowsFacts {
                open: ui.open_nodes.clone(),
                closed: ui.filter_closed.clone(),
                filter: ui.filter.clone(),
            },
            |facts| {
                build_library_rows(LibraryTree {
                    artists,
                    open: &facts.open,
                    closed: &facts.closed,
                    filter: &facts.filter,
                })
            },
        )
    }

    /// The row the pointer's marker names, resolved against the painted rows
    /// (design D1, D2): the marker carries the row's own id, so the row model
    /// resolves it and a stale or foreign id names no item. `None` while no
    /// painted row carries the id.
    #[must_use]
    pub fn music_row_item_id(&self, row_id: &str) -> Option<String> {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        rows.iter()
            .find(|row| row.id() == row_id)
            .and_then(Self::action_item_id)
    }

    /// The item a row picks (design D7, D5): an album's own id, and a track's album
    /// — a picked track picks its album. An artist row picks nothing: the Hero is
    /// album-scoped (design D5), so an artist's detail would empty it.
    #[must_use]
    fn picked_item_id(row: &LibraryRow) -> Option<String> {
        match row {
            LibraryRow::Letter(_) | LibraryRow::Artist { .. } => None,
            LibraryRow::Album { album_id, .. } | LibraryRow::Track { album_id, .. } => {
                Some(album_id.clone())
            }
        }
    }

    /// The item an action on the hovered row names (design D3, D8): an artist
    /// row's Service item, an album's own id, and a track's id, so the
    /// play-next and add-to-queue keys act on what the pointer marks.
    #[must_use]
    pub fn action_item_id(row: &LibraryRow) -> Option<String> {
        match row {
            LibraryRow::Letter(_) => None,
            LibraryRow::Artist { item_id, .. } => item_id.clone(),
            LibraryRow::Album { album_id, .. } => Some(album_id.clone()),
            LibraryRow::Track { track_id, .. } => Some(track_id.clone()),
        }
    }

    /// Send the detail of the item a row picks (design D7): the core stores it and
    /// the next frame carries the Hero. No command for a row that names no item. A
    /// pick never opens the Narrow arrangement's Hero overlay (only the row menu's
    /// View Detail does) and closes the item menu the previous item opened.
    fn show_detail(&self, item_id: Option<String>) {
        let Some(item_id) = item_id else {
            return;
        };
        self.link.library(GuiLibraryCommand::ShowDetail { item_id });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Open or close one tree node (design D7): opening an album whose tracklist
    /// the carried content does not hold asks the core to load it (design D5). While
    /// the filter narrows the rows, the press overrides the filter's auto-open for
    /// that node instead of the stored open state, so the node the user closed stays
    /// closed until the filter changes.
    fn set_node_open(&self, row: &LibraryRow, open: bool) {
        let row_id = row.id().to_owned();
        // Opening an album whose tracklist the carried content does not hold
        // asks the core to load it (design D5).
        let load_album = match row {
            LibraryRow::Album {
                album_id,
                tracks_loaded: false,
                ..
            } if open => Some(album_id.clone()),
            _ => None,
        };
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if ui.filter.trim().is_empty() {
                with_id(&mut ui.open_nodes, &row_id, open);
            } else {
                // The closed set names the nodes to keep shut, so it takes the id
                // when the press closes the node.
                with_id(&mut ui.filter_closed, &row_id, !open);
            }
        });
        if let Some(album_id) = load_album {
            self.link.library(GuiLibraryCommand::OpenAlbum { album_id });
        }
    }

    /// Reveal the Hero's picked album in the list (`state.ts`'s
    /// `revealPickedAlbum`). The album's artist opens and the list scrolls so the
    /// album's row paints, so a restored pick and a pill's last pick reach the
    /// Hero on a row in view. An album whose tracks are not loaded is opened
    /// through the same command a press sends, which is what loads them.
    pub fn reveal_picked_album(&self, list: &GuiMusicList, detail: &GuiMusicDetail) {
        let Some(album) = detail.hero.as_ref().map(|hero| &hero.album) else {
            return;
        };
        let Some(artist) = list.artists.iter().find(|candidate| {
            candidate
                .albums
                .iter()
                .any(|row| row.album_id == album.album_id)
        }) else {
            return;
        };
        let album_row = album_row_id(&album.album_id);
        let artist_row = artist_row_id(artist);
        let mut ui = self.ui;
        ui.with_mut(|ui| with_id(&mut ui.open_nodes, &artist_row, true));
        if self.music_scroll.peek().viewport_height <= 0.0 {
            // The list is not measured yet, so the row cannot scroll into view.
            // The first measurement takes the scroll.
            let mut pending = self.pending_reveal_row;
            pending.set(Some(album_row));
        } else {
            let ui = self.ui.peek().clone();
            let rows = self.rows_of(&ui);
            let held = *self.music_scroll.peek();
            let top = scroll_top_to_show_cursor(
                &rows,
                Some(&album_row),
                held.scroll_top,
                held.viewport_height,
            );
            set_scroll_top(self.music_scroll, &rows, top);
        }
        if album.tracks.is_none() {
            self.link.library(GuiLibraryCommand::OpenAlbum {
                album_id: album.album_id.clone(),
            });
        }
    }

    /// Play a track (`add-gui-library-panel`, the user's double-click contract):
    /// the track's own stable id, through the same play command Enter sends. The
    /// Owner's queue is replaced, and with the library playback policy's autoload on,
    /// the album's tracks replace it from the played track onward. The list's track
    /// rows and the Hero's tracklist rows both send it, so one gesture cannot drift
    /// from the other.
    pub fn play_library_track(&self, track_id: String) {
        self.link
            .library(GuiLibraryCommand::Play { item_id: track_id });
    }

    /// Press a row (design D7, gui-tree-double-click design D1): a press is the
    /// pick's half only — a track row picks its album, and the pick's album shows
    /// in the Hero. An artist row picks nothing (the Hero is album-scoped, design
    /// D5) and an album row picks itself. A press never opens or closes a tree
    /// node: [`Self::toggle_library_node`] owns that half, on the row's
    /// double-click.
    pub fn press_library_row(&self, row: &LibraryRow) {
        match row {
            LibraryRow::Letter(_) => {}
            LibraryRow::Artist { .. } | LibraryRow::Album { .. } | LibraryRow::Track { .. } => {
                self.show_detail(Self::picked_item_id(row));
            }
        }
    }

    /// Open or close one tree node (gui-tree-double-click design D1): an artist
    /// or album row flips the open state it painted, through the same
    /// [`Self::set_node_open`] that holds the album track load and the filter's
    /// override. The row paints the state the flip read, and the two presses of a
    /// double-click no longer change it, so the double-click reads the current
    /// state. Any other row is no tree node, so its double-click toggles nothing.
    pub fn toggle_library_node(&self, row: &LibraryRow) {
        match row {
            LibraryRow::Artist { open, .. } | LibraryRow::Album { open, .. } => {
                self.set_node_open(row, !*open);
            }
            LibraryRow::Letter(_) | LibraryRow::Track { .. } => {}
        }
    }

    /// Scroll the list by one movement key (design D6): Up and Down move by the
    /// height of the row at the top edge, Page Up and Page Down by the viewport
    /// height, and Home and End go to the list's ends, inside the list's range.
    pub fn step_library_scroll(&self, mv: ListMove) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let held = *self.music_scroll.peek();
        set_scroll_top(self.music_scroll, &rows, scroll_step(&rows, held, mv));
    }

    /// Narrow the list to the names that contain `filter`, without case (design
    /// D7): the scroll stays inside the narrower list, so no offset outlives the
    /// rows it scrolled. New text starts the filter's own open and close over: the
    /// nodes the user closed under the old text open again.
    pub fn set_library_filter(&self, filter: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if filter != ui.filter {
                ui.filter_closed.clear();
            }
            ui.filter = filter;
        });
        let held = *self.music_scroll.peek();
        let state = self.ui.peek().clone();
        let rows = self.rows_of(&state);
        set_scroll_top(self.music_scroll, &rows, held.scroll_top);
    }

    /// Jump the list to a letter of the rail (design D7, D6): the list scrolls to
    /// the first row under the letter and picks nothing. A letter without rows
    /// changes nothing.
    pub fn jump_to_letter(&self, letter: char) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let view = *self.music_scroll.peek();
        let Some((_, scroll_top)) = jump_to_letter(&letter_rail(&rows), &rows, letter, view) else {
            return;
        };
        set_scroll_top(self.music_scroll, &rows, scroll_top);
    }

    /// Report the list's measured viewport height (design D7): the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_library_list(&self, viewport_height: f32) {
        let mut music_scroll = self.music_scroll;
        music_scroll.with_mut(|scroll| scroll.viewport_height = viewport_height);
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let pending = self.pending_reveal_row.peek().clone();
        let held = *music_scroll.peek();
        let Some(row) = pending.filter(|_| viewport_height > 0.0) else {
            set_scroll_top(music_scroll, &rows, held.scroll_top);
            return;
        };
        let mut pending = self.pending_reveal_row;
        pending.set(None);
        let top = scroll_top_to_show_cursor(&rows, Some(&row), held.scroll_top, viewport_height);
        set_scroll_top(music_scroll, &rows, top);
    }

    /// Report the list's native scroll offset (design D7): a user scroll moves the
    /// rows in view, and the offset stays inside the list's range.
    pub fn scroll_library_list(&self, scroll_top: f32) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        set_scroll_top(self.music_scroll, &rows, scroll_top);
    }
}
