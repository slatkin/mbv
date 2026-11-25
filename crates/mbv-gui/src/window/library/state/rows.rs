//! The Library list's row model and scroll geometry over the interaction
//! state (`replace-tauri-with-blitz` D15, the seam of `state.ts`'s row-model
//! half): the memoized painted rows, the press rules, the reveal, the filter,
//! and the scroll reports. The interaction state lives in [`super`].

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiMusicDetail, GuiMusicHeroSubjectFrame, GuiMusicList};
use crate::bridge::library::GuiLibraryCommand;
use crate::gui_core::GuiLibraryMenuRow;
use crate::window::library::list::{
    LibraryRow, LibraryTree, ListMove, MusicRowsFacts, album_row_id, artist_row_id,
    build_library_rows, jump_to_letter, letter_rail, list_window, scroll_step,
    scroll_top_to_show_cursor,
};
use crate::window::library::parts::library_music_list;
use crate::window::library::scroll::set_scroll_top;

use super::{LibraryState, LibraryUiState, RowHoverPresses};

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
    /// — a picked track picks its album. An artist row names no item: its pick is
    /// the artist page, which [`Self::press_library_row`] sends apart.
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
    /// add-to-queue key acts on what the pointer marks.
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

    /// Reveal the Hero's picked row in the list (`state.ts`'s
    /// `revealPickedAlbum`; gui-music-artist-page design D6). The reveal follows
    /// the subject: an album pick opens its artist and scrolls so the album's
    /// row paints, and an artist pick scrolls the artist row into view and opens
    /// nothing, so a restored pick and a pill's last pick reach the Hero on a
    /// row in view. An album whose tracks are not loaded is opened through the
    /// same command a press sends, which is what loads them.
    pub fn reveal_picked_album(&self, list: &GuiMusicList, detail: &GuiMusicDetail) {
        let Some(hero) = detail.hero.as_ref() else {
            return;
        };
        match &hero.subject {
            GuiMusicHeroSubjectFrame::Album(album) => self.reveal_album_pick(list, album),
            GuiMusicHeroSubjectFrame::Artist(_) => {
                self.scroll_row_into_view(hero.picked_row_id());
            }
        }
    }

    /// The album subject's reveal (design D6): the album's artist opens and the
    /// list scrolls so the album's row paints.
    fn reveal_album_pick(
        &self,
        list: &GuiMusicList,
        album: &crate::bridge::frame::library::GuiMusicAlbumFrame,
    ) {
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
        self.scroll_row_into_view(album_row);
        if album.tracks.is_none() {
            self.link.library(GuiLibraryCommand::OpenAlbum {
                album_id: album.album_id.clone(),
            });
        }
    }

    /// Scroll the list so the row `row_id` paints (design D6): while the list
    /// is not measured yet the row cannot scroll into view, so the first
    /// measurement takes the scroll.
    fn scroll_row_into_view(&self, row_id: String) {
        if self.music_scroll.peek().viewport_height <= 0.0 {
            // The list is not measured yet, so the row cannot scroll into view.
            // The first measurement takes the scroll.
            let mut pending = self.pending_reveal_row;
            pending.set(Some(row_id));
        } else {
            let ui = self.ui.peek().clone();
            let rows = self.rows_of(&ui);
            let held = *self.music_scroll.peek();
            let top = scroll_top_to_show_cursor(
                &rows,
                Some(&row_id),
                held.scroll_top,
                held.viewport_height,
            );
            set_scroll_top(self.music_scroll, &rows, top);
        }
    }

    /// Play a track (`add-gui-library-panel`, the user's double-click contract):
    /// the track's own stable id, through the same play command Enter sends. The
    /// Owner's queue is replaced, and with the library playback policy's autoload on,
    /// the album's tracks replace it from the played track onward. The list's track
    /// rows and the Hero's tracklist rows both send it, so one gesture cannot drift
    /// from the other.
    pub fn play_library_track(&self, track_id: &str) {
        self.play_library_item(track_id);
    }

    /// Play the item a row names (`add-gui-library-panel` design D2's Enter):
    /// the item's own stable id. The Music list's Enter, its row double-click,
    /// and its hover Play all send it (gui-row-hover-actions design D4).
    pub fn play_library_item(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::Play {
            item_id: item_id.to_owned(),
        });
    }

    /// Add the item a row names to the queue (`add-gui-library-panel` design D2's
    /// `a`): the item's own stable id. The Music list's `a` and its hover Add
    /// both send it (gui-row-hover-actions design D4).
    pub fn queue_library_item(&self, item_id: &str) {
        self.link.library(GuiLibraryCommand::AddToQueue {
            item_id: item_id.to_owned(),
        });
    }

    /// Shuffle the row (gui-row-hover-actions design D4): the core runs the
    /// row's own right-click shuffle entry, so the hover Shuffle control and the
    /// menu entry cannot drift apart.
    pub fn shuffle_library_row(&self, row: GuiLibraryMenuRow) {
        self.link.library(GuiLibraryCommand::Shuffle { row });
    }

    /// The hover controls' presses for a Music row that names `item_id`
    /// (gui-row-hover-actions design D3, D4): Play, Add, and Shuffle run the same
    /// commands the keys and the row menu run, and `offers_shuffle` names whether
    /// the row's menu carries a shuffle entry — only a collection's does. A row
    /// that names no item offers none of them, so an artist row without an item
    /// id keeps its Expand control alone.
    #[must_use]
    pub fn row_hover_presses(
        &self,
        item_id: Option<String>,
        offers_shuffle: bool,
    ) -> RowHoverPresses {
        item_id.map_or_else(RowHoverPresses::default, |item_id| {
            self.item_hover_presses(
                item_id,
                Self::play_library_item,
                Self::queue_library_item,
                offers_shuffle,
            )
        })
    }

    /// Pick the artist of an artist row (gui-music-artist-page design D6): the
    /// Hero shows the artist's page. The command names the artist's stable key
    /// and the Music library the row paints in, and the press closes the item
    /// menu and the row menu as `show_detail` does. It opens and closes no
    /// node.
    fn show_artist(&self, artist_key: &crate::bridge::frame::library::GuiArtistKey) {
        let list = self.sections.library_list.peek();
        let Some(music) = library_music_list(list.as_deref()) else {
            return;
        };
        self.link.library(GuiLibraryCommand::ShowArtist {
            lib_idx: music.lib_idx,
            artist_key: artist_key.clone(),
        });
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.item_menu_open = false;
            ui.row_menu = None;
        });
    }

    /// Press a row (design D7, gui-tree-double-click design D1; gui-music-artist-page
    /// design D6): a press is the pick's half only — a track row picks its album,
    /// an album row picks itself, and an artist row picks the artist, whose page
    /// the Hero shows. A press never opens or closes a tree
    /// node: [`Self::toggle_library_node`] owns that half, on the row's
    /// double-click.
    pub fn press_library_row(&self, row: &LibraryRow) {
        match row {
            LibraryRow::Letter(_) => {}
            LibraryRow::Artist { artist_key, .. } => {
                self.show_artist(artist_key);
            }
            LibraryRow::Album { .. } | LibraryRow::Track { .. } => {
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

    /// Open every painted artist row (gui-list-header-buttons design D4, spec
    /// "Expand all opens every top-level parent row the list shows"): the
    /// album rows' open state stays as it was, so the press opens no album. The
    /// ids also leave the filter's closed set, so the artists stay open after
    /// the filter clears. The press sends no command and picks nothing.
    pub fn expand_all_music(&self) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let artists: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                LibraryRow::Artist { base, .. } => Some(base.id.clone()),
                LibraryRow::Letter(_) | LibraryRow::Album { .. } | LibraryRow::Track { .. } => None,
            })
            .collect();
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            for id in &artists {
                with_id(&mut ui.open_nodes, id, true);
                with_id(&mut ui.filter_closed, id, false);
            }
        });
    }

    /// Close every open parent row (gui-list-header-buttons design D4, spec
    /// "Collapse all closes every open parent row"): every artist and album
    /// row. While the filter narrows the rows, the painted parent rows also
    /// enter the filter's closed set, so the rows the filter opened stay shut
    /// after it clears. The press sends no command and picks nothing.
    pub fn collapse_all_music(&self) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let narrowing = !ui.filter.trim().is_empty();
        let parents: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                LibraryRow::Artist { base, .. } | LibraryRow::Album { base, .. } => {
                    Some(base.id.clone())
                }
                LibraryRow::Letter(_) | LibraryRow::Track { .. } => None,
            })
            .collect();
        let mut ui = self.ui;
        ui.with_mut(|ui| {
            ui.open_nodes.clear();
            if narrowing {
                for id in &parents {
                    with_id(&mut ui.filter_closed, id, true);
                }
            }
        });
    }

    /// Refresh the Music destination (gui-list-header-buttons design D7): the
    /// tree refresh reloads the children of the open parent rows in view, so
    /// the command names the album ids of the open album rows the painted
    /// window holds (spec "A refresh of an open album in view"). An open album
    /// scrolled out of the window names nothing (spec "An open album out of
    /// view"). The window keeps its pick, filter, scroll, and open state.
    pub fn refresh_music_list(&self) {
        let ui = self.ui.peek().clone();
        let rows = self.rows_of(&ui);
        let view = *self.music_scroll.peek();
        let window = list_window(&rows, view.scroll_top, view.viewport_height);
        let album_ids: Vec<String> = rows[window.start..window.end]
            .iter()
            .filter_map(|row| match row {
                LibraryRow::Album {
                    album_id,
                    open: true,
                    ..
                } => Some(album_id.clone()),
                LibraryRow::Letter(_)
                | LibraryRow::Artist { .. }
                | LibraryRow::Album { .. }
                | LibraryRow::Track { .. } => None,
            })
            .collect();
        self.link.library(GuiLibraryCommand::Refresh { album_ids });
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
