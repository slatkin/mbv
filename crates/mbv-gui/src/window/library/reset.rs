//! The front-end half of Reset UI State (`add-gui-settings-screen` design
//! D10, the port of `lib/library/reset.ts`): a new `uiResetEpoch` drops every
//! Library interaction store — the pane focus, the cursors, the filters, the
//! open nodes, the pill focus, the menus, the Hero dismissals, and the scroll —
//! while the core clears the kept collections and picks and the panel widths
//! reset beside these. The content itself is the frame's carry, so nothing here
//! mirrors or resets it; the next frame re-carries it.

use dioxus_native::prelude::*;

use super::scroll::initial_list_scroll;
use super::state::{LibraryState, LibraryUiState};

/// Reset every Library interaction store to its mounted value, the list scroll
/// stores included (`reset.ts`'s `resetLibraryInteraction`). The Search, Home,
/// Shows, Emby, Audiobooks, Podcasts, and Feeds stores reset with their ports
/// (rows 6.4 to 6.8); the stores this build holds are the Music interaction
/// state and the Music list's scroll store.
pub fn reset_library_interaction(state: &LibraryState) {
    let mut ui = state.ui;
    ui.set(LibraryUiState::initial());
    let mut music_scroll = state.music_scroll;
    music_scroll.set(initial_list_scroll());
}
