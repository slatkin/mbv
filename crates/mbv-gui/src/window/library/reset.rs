//! The front-end half of Reset UI State (`add-gui-settings-screen` design
//! D10, the port of `lib/library/reset.ts`): a new `uiResetEpoch` drops every
//! Library interaction store — the filters, the pill requests, the open nodes,
//! the menus, the Hero dismissals, and the scroll —
//! while the core clears the kept collections and picks and the panel widths
//! reset beside these. The content itself is the frame's carry, so nothing here
//! mirrors or resets it; the next frame re-carries it.

use dioxus_native::prelude::*;

use super::audiobooks::AudiobooksUiState;
use super::emby::EmbyUiState;
use super::feeds::FeedsUiState;
use super::home::HomeUiState;
use super::podcasts::PodcastsUiState;
use super::scroll::initial_list_scroll;
use super::shows::ShowsUiState;
use super::state::{LibraryState, LibraryUiState};

/// Reset every Library interaction store to its mounted value, the list scroll
/// stores included (`reset.ts`'s `resetLibraryInteraction`). The Search store
/// resets with its port (row 6.8); the stores this build holds are the Music,
/// Home, Emby, Shows, Audiobooks, Podcasts, and Feeds interaction states and
/// their lists' scroll stores. The lists and the Workspaces keep no cursor, so
/// no cursor resets here.
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
    let mut podcasts_ui = state.podcasts_ui;
    podcasts_ui.set(PodcastsUiState::initial());
    let mut feeds_ui = state.feeds_ui;
    feeds_ui.set(FeedsUiState::initial());
    let mut home_scroll = state.scrolls.home;
    home_scroll.set(initial_list_scroll());
    let mut emby_scroll = state.scrolls.emby;
    emby_scroll.set(initial_list_scroll());
    let mut shows_scroll = state.scrolls.shows;
    shows_scroll.set(initial_list_scroll());
    let mut audiobooks_scroll = state.scrolls.audiobooks;
    audiobooks_scroll.set(initial_list_scroll());
    let mut podcasts_scroll = state.scrolls.podcasts;
    podcasts_scroll.set(initial_list_scroll());
    let mut feeds_scroll = state.scrolls.feeds;
    feeds_scroll.set(initial_list_scroll());
    let mut search = state.search;
    search.set(super::search::initial_library_search());
}
