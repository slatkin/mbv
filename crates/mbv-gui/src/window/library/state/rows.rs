//! The Library list's row model and scroll geometry over the interaction
//! state (`replace-tauri-with-blitz` D15, the seam of `state.ts`'s row-model
//! half): the memoized painted rows, the cursor's settle, the press and
//! movement rules, the reveal, the filter, and the scroll reports. The
//! interaction state and the pane walk live in [`super`].

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMusicDetail, GuiMusicList};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::list::{
    LibraryRow, ListMove, album_row_id, artist_row_id, clamp_scroll_top, jump_to_letter,
    letter_rail, list_window, move_list_cursor, scroll_top_to_show_cursor,
};
use crate::window::library::parts::library_music_list;
use crate::window::library::scroll::ListScroll;

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
    /// section store's own storage — a cursor or scroll step copies none of it
    /// — and the memo keys it on the section's revision, so its hit test never
    /// compares the union (design D6).
    pub(super) fn rows_of(&self, ui: &LibraryUiState) -> Arc<[LibraryRow]> {
        let list = self.sections.library_list.peek();
        let artists =
            library_music_list(list.as_ref()).map_or(&[][..], |music| music.artists.as_slice());
        self.rows_memo.rows(
            self.sections.library_list_revision(),
            artists,
            &ui.open_nodes,
            &ui.filter_closed,
            &ui.filter,
        )
    }

    /// Settle the cursor and the scroll once the painted rows may have changed
    /// (design D7): the cursor keeps its row while that row still paints, and falls
    /// to the first painted row otherwise. The scroll store then shows the cursor. A
    /// filter's text change and its clear both run this, so the cursor never rests on
    /// a row that no longer paints.
    pub(super) fn settle_cursor(&self) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let kept = ui
            .cursor
            .as_deref()
            .is_some_and(|cursor| rows.iter().any(|row| row.id() == cursor));
        let cursor = if kept {
            ui.cursor.clone()
        } else {
            move_list_cursor(&rows, None, ListMove::Home, 1)
        };
        if cursor != ui.cursor {
            let mut current = self.ui;
            current.with_mut(|current| current.cursor.clone_from(&cursor));
        }
        self.follow_cursor(&rows, cursor.as_deref());
    }

    /// Scroll the list so the cursor's row paints fully, in the scroll store (design
    /// D7, D9).
    fn follow_cursor(&self, rows: &[LibraryRow], cursor: Option<&str>) {
        let scroll = *self.music_scroll.peek();
        let shown = ListScroll {
            scroll_top: scroll_top_to_show_cursor(
                rows,
                cursor,
                scroll.scroll_top,
                scroll.viewport_height,
            ),
            viewport_height: scroll.viewport_height,
        };
        if shown != scroll {
            let mut music_scroll = self.music_scroll;
            music_scroll.set(shown);
        }
    }

    /// The cursor's row, or `None` while no painted row carries the cursor.
    #[must_use]
    pub(super) fn cursor_row(&self, ui: &LibraryUiState) -> Option<LibraryRow> {
        let rows = self.rows_of(ui);
        rows.iter()
            .find(|row| Some(row.id()) == ui.cursor.as_deref())
            .cloned()
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

    /// The item an action on the cursor names (design D3, D8): an artist row's
    /// Service item, an album's own id, and a track's id, so the play-next and
    /// add-to-queue keys act on what the cursor rests on.
    #[must_use]
    pub(super) fn action_item_id(row: &LibraryRow) -> Option<String> {
        match row {
            LibraryRow::Letter(_) => None,
            LibraryRow::Artist { item_id, .. } => item_id.clone(),
            LibraryRow::Album { album_id, .. } => Some(album_id.clone()),
            LibraryRow::Track { track_id, .. } => Some(track_id.clone()),
        }
    }

    /// Send the detail of the item a row picks (design D7): the core stores it and
    /// the next frame carries the Hero. No command for a row that names no item. A
    /// pick paints the Narrow arrangement's Hero overlay again (design D4), closes
    /// the item menu the previous item opened, and starts the tracklist's cursor
    /// over, because the tracks it rested on belong to the previous album.
    fn show_detail(&self, item_id: Option<String>) {
        let Some(item_id) = item_id else {
            return;
        };
        self.link.library(GuiLibraryCommand::ShowDetail { item_id });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.hero_dismissed = false;
            ui.item_menu_open = false;
            ui.row_menu = None;
            ui.track_cursor = None;
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
    /// `revealPickedAlbum`). The cursor takes the album's row and its artist
    /// opens, so the row shows. An album whose tracks are not loaded is opened
    /// through the same command a press sends, which is what loads them. A
    /// restored pick and a pill's last pick reach the Hero this way.
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
        ui.with_mut(|ui| {
            with_id(&mut ui.open_nodes, &artist_row, true);
            ui.cursor = Some(album_row.clone());
        });
        if self.music_scroll.peek().viewport_height <= 0.0 {
            // The list is not measured yet, so the row cannot scroll into view.
            // The first measurement takes the scroll.
            let mut pending = self.pending_reveal_row;
            pending.set(Some(album_row));
        } else {
            let ui = self.ui.peek().clone();
            let rows = self.rows_of(&ui);
            self.follow_cursor(&rows, Some(&album_row));
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

    /// Press a track row in the Hero's tracklist: the cursor moves onto it, as the
    /// list's track press does.
    pub fn press_library_track(&self, track_id: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if ui.track_cursor.as_deref() != Some(track_id.as_str()) {
                ui.track_cursor = Some(track_id);
            }
        });
    }

    /// Press a row (design D7): an artist or album row opens or closes and is
    /// picked; a track row moves the cursor onto itself and is picked — the pick's
    /// album shows in the Hero, and the cursor is the press's visible half, as the
    /// Queue panel's click moves the queue cursor. The row's painted open state is
    /// the half to flip, so a press under a filter closes the node the filter
    /// opened. A press on an artist or album row moves no cursor — that is the
    /// keys' work.
    pub fn press_library_row(&self, row: &LibraryRow) {
        match row {
            LibraryRow::Letter(_) => {}
            LibraryRow::Track { .. } => {
                let mut ui = self.ui;
                ui.with_mut(|ui| ui.cursor = Some(row.id().to_owned()));
                let ui = self.ui.peek().clone();
                let rows = self.rows_of(&ui);
                self.follow_cursor(&rows, Some(row.id()));
                self.show_detail(Self::picked_item_id(row));
            }
            LibraryRow::Artist { open, .. } | LibraryRow::Album { open, .. } => {
                self.set_node_open(row, !*open);
                self.show_detail(Self::picked_item_id(row));
            }
        }
    }

    /// Move the list's cursor one movement along (design D3, D7): Up and Down
    /// move a row, Page Up and Page Down move the rows in view, and Home and End
    /// take the list's ends. The scroll offset follows, so the cursor row stays
    /// fully visible.
    pub fn move_library_cursor(&self, mv: ListMove) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let scroll = *self.music_scroll.peek();
        let view = list_window(&rows, scroll.scroll_top, scroll.viewport_height);
        let cursor = move_list_cursor(&rows, ui.cursor.as_deref(), mv, view.end - view.start);
        let mut current = self.ui;
        current.with_mut(|current| current.cursor.clone_from(&cursor));
        self.follow_cursor(&rows, cursor.as_deref());
    }

    /// Open or close the cursor's node (design D7's Right and Left).
    pub fn set_library_cursor_open(&self, open: bool) {
        let ui = self.ui.peek().clone();
        let row = self.cursor_row(&ui);
        if let Some(row) = row
            && matches!(row, LibraryRow::Artist { .. } | LibraryRow::Album { .. })
        {
            self.set_node_open(&row, open);
        }
    }

    /// Narrow the list to the names that contain `filter`, without case (design
    /// D7). The cursor keeps its row while that row stays painted, and otherwise
    /// falls to the first painted row; the scroll offset follows the narrower list.
    /// New text starts the filter's own open and close over: the nodes the user
    /// closed under the old text open again.
    pub fn set_library_filter(&self, filter: String) {
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            if filter != ui.filter {
                ui.filter_closed.clear();
            }
            ui.filter = filter;
        });
        self.settle_cursor();
    }

    /// Jump to a letter of the rail (design D7): the cursor moves to the first row
    /// under the letter and that row scrolls into view. A letter without rows
    /// changes nothing.
    pub fn jump_to_letter(&self, letter: char) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let view = *self.music_scroll.peek();
        let Some((cursor, scroll_top)) = jump_to_letter(&letter_rail(&rows), &rows, letter, view)
        else {
            return;
        };
        let mut ui = self.ui;
        ui.with_mut(|ui| ui.cursor = Some(cursor));
        let mut music_scroll = self.music_scroll;
        music_scroll.with_mut(|scroll| scroll.scroll_top = scroll_top);
    }

    /// Report the list's measured viewport height (design D7): the rows in view
    /// follow it, and the scroll offset stays inside the narrower range.
    pub fn measure_library_list(&self, viewport_height: f32) {
        let mut music_scroll = self.music_scroll;
        music_scroll.with_mut(|scroll| scroll.viewport_height = viewport_height);
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let pending = self.pending_reveal_row.peek().clone();
        if let Some(row) = pending.filter(|_| viewport_height > 0.0) {
            let mut pending = self.pending_reveal_row;
            pending.set(None);
            self.follow_cursor(&rows, Some(&row));
            return;
        }
        let mut music_scroll = self.music_scroll;
        music_scroll.with_mut(|scroll| {
            scroll.scroll_top = clamp_scroll_top(&rows, scroll.scroll_top, scroll.viewport_height);
        });
    }

    /// Report the list's native scroll offset (design D7): a user scroll moves the
    /// rows in view, and the offset stays inside the list's range.
    pub fn scroll_library_list(&self, scroll_top: f32) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let mut music_scroll = self.music_scroll;
        let held = *music_scroll.peek();
        let clamped = ListScroll {
            scroll_top: clamp_scroll_top(&rows, scroll_top, held.viewport_height),
            viewport_height: held.viewport_height,
        };
        if clamped != held {
            music_scroll.set(clamped);
        }
    }
}
