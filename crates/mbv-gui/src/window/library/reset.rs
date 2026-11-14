//! The front-end half of Reset UI State (`add-gui-settings-screen` design
//! D10, the port of `lib/library/reset.ts`): a new `uiResetEpoch` drops every
//! Library interaction store — the pane focus, the cursors, the filters, the
//! open nodes, the pill focus, the menus, the Hero dismissals, and the scroll —
//! while the core clears the kept collections and picks and the panel widths
//! reset beside these. The content itself is the frame's carry, so nothing here
//! mirrors or resets it; the next frame re-carries it.

use dioxus_native::prelude::*;

use super::emby::EmbyUiState;
use super::home::HomeUiState;
use super::scroll::initial_list_scroll;
use super::state::{LibraryState, LibraryUiState};

/// Reset every Library interaction store to its mounted value, the list scroll
/// stores included (`reset.ts`'s `resetLibraryInteraction`). The Search,
/// Shows, Audiobooks, Podcasts, and Feeds stores reset with their ports
/// (rows 6.5 to 6.8); the stores this build holds are the Music, Home, and
/// Emby interaction states and their lists' scroll stores.
pub fn reset_library_interaction(state: &LibraryState) {
    let mut ui = state.ui;
    ui.set(LibraryUiState::initial());
    let mut music_scroll = state.music_scroll;
    music_scroll.set(initial_list_scroll());
    let mut home_ui = state.home_ui;
    home_ui.set(HomeUiState::initial());
    let mut emby_ui = state.emby_ui;
    emby_ui.set(EmbyUiState::initial());
    let mut home_scroll = state.scrolls.home;
    home_scroll.set(initial_list_scroll());
    let mut emby_scroll = state.scrolls.emby;
    emby_scroll.set(initial_list_scroll());
}
