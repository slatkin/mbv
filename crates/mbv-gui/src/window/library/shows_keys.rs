//! The Shows destination's key routing (`add-gui-library-panel` design D3,
//! D14, the port of `lib/library/shows-keys.ts`): the Library keys while a
//! Shows library is active. [`super::keys`] stays the panel's one routing site
//! and hands a Shows key here after the window keys and the open-field checks,
//! so this module never installs a router. The pane focus decides the control
//! a key acts on: the mode pills, the tree list, or the Hero's Workspace.
//!
//! Everything else matches the Music router: the movements and Enter never
//! reach the core, Right and Left open and close the tree node under the
//! cursor (the tree meaning design D3 gives them), Esc's overlay and menu
//! handling runs before this, `/` opens the list's filter, and Space with no
//! focused pill stays the core's play and pause.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiShowsDetail, GuiShowsList};
use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;

use super::keys_shared::{bare, cursor_move, is_list_and_hero_key};
use super::parts::{library_shows_detail, library_shows_list};
use super::shows_list::ShowsListRow;
use super::shows_workspace::{
    move_shows_episode_cursor, play_next_shows_episode, play_shows_episode, queue_shows_episode,
    step_shows_season,
};
use super::state::{LibraryPane, LibraryState};

/// Route a key to the Shows keys (design D12): the pill row walks its modes
/// and selects the focused one, the list's cursor moves and Enter selects a
/// series or plays a flat row, and the Hero's Workspace cursor moves over the
/// season's episodes. `n` and `a` act on the item the focused pane names — a
/// flat row in the list, an episode in the Workspace — and a series row is not
/// playable, so its `n` and `a` change nothing.
#[must_use]
pub fn route_shows_panel_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    let list = state.sections.library_list.peek();
    let Some(shows) = library_shows_list(list.as_ref()) else {
        return KeyOutcome::Core;
    };
    let detail_section = state.sections.library_detail.peek();
    let Some(detail) = library_shows_detail(detail_section.as_ref()) else {
        return KeyOutcome::Core;
    };
    let lib_idx = shows.lib_idx;
    let ui = state.ui.peek().clone();
    // `/` opens the list's filter (design D3, D7): the rail's keyboard path.
    if key.key == "/" && bare(key) {
        state.open_library_filter();
        return KeyOutcome::Handled;
    }
    // Space selects the focused pill (design D6). With no pill focused, it is
    // the core's: Space plays and pauses.
    if key.key == " " && !key.modifiers.ctrl_key && !key.meta_key {
        let pill = state.focused_shows_pill(shows);
        let Some(pill) = pill else {
            return KeyOutcome::Core;
        };
        state.request_shows_pill_select(pill);
        return KeyOutcome::Handled;
    }
    if !bare(key) {
        return KeyOutcome::Core;
    }
    if ui.pane == Some(LibraryPane::Pills) {
        return or_remaining(route_pill_key(state, key, shows), key, state);
    }
    if ui.pane == Some(LibraryPane::List) {
        return route_list_key(state, key);
    }
    if ui.pane == Some(LibraryPane::Hero) && detail.hero.is_some() {
        return or_remaining(route_workspace_key(state, key, lib_idx, detail), key, state);
    }
    // The Hero pane the Upcoming mode does not paint, and any pane that left a
    // key unclaimed, still keep the Library's keys from the core: the
    // movements and Enter change nothing, and `n` and `a` act on the list's
    // cursor row.
    route_remaining_key(state, key)
}

/// The key's answer, or the remaining Library keys' answer when the pane left
/// it unclaimed: a pane that does not act on a key never passes it to the
/// core's playback chords (design D3, step 4).
fn or_remaining(outcome: KeyOutcome, key: &DomKey, state: &LibraryState) -> KeyOutcome {
    match outcome {
        KeyOutcome::Core => route_remaining_key(state, key),
        taken => taken,
    }
}

/// The Library's own keys no Shows pane claims (design D3, step 4): `n` and
/// `a` act on the list's cursor row, as the Music router's item key acts from
/// every pane, and a series row is not playable so its key changes nothing;
/// the movements and Enter change nothing at all. Every one of them is
/// consumed, so a playback chord never runs from a Shows key.
fn route_remaining_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    if key.key == "n" || key.key == "a" {
        route_list_item_key(state, key);
        return KeyOutcome::Handled;
    }
    if is_list_and_hero_key(&key.key) {
        KeyOutcome::Handled
    } else {
        KeyOutcome::Core
    }
}

/// Route a key to the mode pills (design D12): Right and Left walk the pills,
/// the first of them with no pill focused taking the end it walks from, and
/// Enter selects the focused pill as Space does.
fn route_pill_key(state: &LibraryState, key: &DomKey, shows: &GuiShowsList) -> KeyOutcome {
    match key.key.as_str() {
        "ArrowRight" => {
            state.move_shows_pill_focus(1);
            KeyOutcome::Handled
        }
        "ArrowLeft" => {
            state.move_shows_pill_focus(-1);
            KeyOutcome::Handled
        }
        "Enter" => {
            if let Some(pill) = state.focused_shows_pill(shows) {
                state.request_shows_pill_select(pill);
            }
            KeyOutcome::Handled
        }
        _ => KeyOutcome::Core,
    }
}

/// Route a key to the tree list's cursor (design D14): the arrows, Page Up,
/// Page Down, Home, and End move the cursor, Right opens the node under the
/// cursor, Left closes it, and Enter toggles a node or plays an episode leaf.
/// `n` and `a` act on an episode or a flat row, because a series and a season
/// are not playable.
fn route_list_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    if let Some(mv) = cursor_move(&key.key) {
        state.move_shows_list_cursor(mv);
        return KeyOutcome::Handled;
    }
    match key.key.as_str() {
        "ArrowRight" => {
            open_cursor_node(state);
            KeyOutcome::Handled
        }
        "ArrowLeft" => {
            close_cursor_node(state);
            KeyOutcome::Handled
        }
        "Enter" => {
            route_list_enter(state);
            KeyOutcome::Handled
        }
        "n" | "a" => {
            route_list_item_key(state, key);
            KeyOutcome::Handled
        }
        _ if is_list_and_hero_key(&key.key) => KeyOutcome::Handled,
        _ => KeyOutcome::Core,
    }
}

/// Right on the cursor row (design D14): a node opens. A leaf has no
/// children, so the key changes nothing.
fn open_cursor_node(state: &LibraryState) {
    if let Some(row) = state.shows_cursor_row()
        && let Some(node_id) = row.as_node()
    {
        state.open_shows_node(node_id);
    }
}

/// Left on the cursor row (design D14): an open node closes. A closed node
/// and a leaf change nothing.
fn close_cursor_node(state: &LibraryState) {
    if let Some(row) = state.shows_cursor_row()
        && let Some(node_id) = row.as_node()
        && state.shows_node_open(node_id)
    {
        state.close_shows_node(node_id);
    }
}

/// Enter on the cursor row (design D14): a series or season node toggles and
/// selects, and an episode leaf or a flat row plays. A cursor on no row sends
/// nothing.
fn route_list_enter(state: &LibraryState) {
    let Some(row) = state.shows_cursor_row() else {
        return;
    };
    if let Some(node_id) = row.as_node() {
        state.toggle_shows_node(node_id);
        state.select_shows_row(&row);
        return;
    }
    match &row {
        ShowsListRow::Flat(flat) => state.play_shows_row(&flat.row.id),
        ShowsListRow::Episode(episode) => state.play_shows_row(&episode.source_id),
        _ => {}
    }
}

/// Route `n` and `a` to a playable row (design D14): an episode leaf or a
/// flat row. A series or a season is not playable, so its key sends no
/// command.
fn route_list_item_key(state: &LibraryState, key: &DomKey) {
    let Some(row) = state.shows_cursor_row() else {
        return;
    };
    let id = match &row {
        ShowsListRow::Series(_) | ShowsListRow::Season(_) | ShowsListRow::Heading(_) => return,
        ShowsListRow::Flat(flat) => &flat.row.id,
        ShowsListRow::Episode(episode) => &episode.source_id,
    };
    if key.key == "n" {
        state.play_shows_row_next(id);
        return;
    }
    state.queue_shows_row(id);
}

/// Route a key to the Hero's Workspace (design D12): the movement keys move
/// the episode cursor, Left and Right step the season, Enter plays the
/// episode, and `n` and `a` play it next or queue it.
fn route_workspace_key(
    state: &LibraryState,
    key: &DomKey,
    lib_idx: usize,
    detail: &GuiShowsDetail,
) -> KeyOutcome {
    if let Some(mv) = cursor_move(&key.key) {
        move_shows_episode_cursor(state, mv, detail);
        return KeyOutcome::Handled;
    }
    match key.key.as_str() {
        "ArrowLeft" => {
            step_shows_season(state, -1, lib_idx, detail);
            KeyOutcome::Handled
        }
        "ArrowRight" => {
            step_shows_season(state, 1, lib_idx, detail);
            KeyOutcome::Handled
        }
        "Enter" | "n" | "a" => {
            route_workspace_item_key(state, key, lib_idx, detail);
            KeyOutcome::Handled
        }
        _ if is_list_and_hero_key(&key.key) => KeyOutcome::Handled,
        _ => KeyOutcome::Core,
    }
}

/// Run Enter, `n`, or `a` against the Workspace's selected episode (design
/// D12): the Hero owns the episode cursor, so the key names no episode id.
fn route_workspace_item_key(
    state: &LibraryState,
    key: &DomKey,
    lib_idx: usize,
    detail: &GuiShowsDetail,
) {
    if key.key == "n" {
        play_next_shows_episode(state, lib_idx, detail);
        return;
    }
    if key.key == "a" {
        queue_shows_episode(state, lib_idx, detail);
        return;
    }
    play_shows_episode(state, lib_idx, detail);
}

#[cfg(test)]
mod tests;
