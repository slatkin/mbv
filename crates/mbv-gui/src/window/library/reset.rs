//! The front-end half of Reset UI State (`add-gui-settings-screen` design
//! D10, the port of `lib/library/reset.ts`): a new `uiResetEpoch` drops every
//! Library interaction store — the pane focus, the cursors, the filters, the
//! open nodes, the pill focus, the menus, the Hero dismissals, and the scroll —
//! while the core clears the kept collections and picks and the panel widths
//! reset beside these. The content itself is the frame's carry, so nothing here
//! mirrors or resets it; the next frame re-carries it.

use dioxus_native::prelude::*;

use super::audiobooks::AudiobooksUiState;
use super::emby::EmbyUiState;
use super::home::HomeUiState;
use super::scroll::initial_list_scroll;
use super::shows::ShowsUiState;
use super::state::{LibraryState, LibraryUiState};

/// Reset every Library interaction store to its mounted value, the list scroll
/// stores included (`reset.ts`'s `resetLibraryInteraction`). The Search,
/// Podcasts, and Feeds stores reset with their ports (rows 6.7 and 6.8); the
/// stores this build holds are the Music, Home, Emby, Shows, and Audiobooks
/// interaction states and their lists' scroll stores. The two Workspaces'
/// cursors reset with the Hero components' selection changes, as
/// `resetShowsEpisodeCursor` and `resetAbsChapterCursor` do.
pub fn reset_library_interaction(state: &LibraryState) {
    let mut ui = state.ui;
    ui.set(LibraryUiState::initial());
    let mut music_scroll = state.music_scroll;
    music_scroll.set(initial_list_scroll());
    let mut home_ui = state.home_ui;
    home_ui.set(HomeUiState::initial());
    let mut emby_ui = state.emby_ui;
    emby_ui.set(EmbyUiState::initial());
    let mut shows_ui = state.shows_ui;
    shows_ui.set(ShowsUiState::initial());
    let mut audiobooks_ui = state.audiobooks_ui;
    audiobooks_ui.set(AudiobooksUiState::initial());
    let mut home_scroll = state.scrolls.home;
    home_scroll.set(initial_list_scroll());
    let mut emby_scroll = state.scrolls.emby;
    emby_scroll.set(initial_list_scroll());
    let mut shows_scroll = state.scrolls.shows;
    shows_scroll.set(initial_list_scroll());
    let mut audiobooks_scroll = state.scrolls.audiobooks;
    audiobooks_scroll.set(initial_list_scroll());
}
