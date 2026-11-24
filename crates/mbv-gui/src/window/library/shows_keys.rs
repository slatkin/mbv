//! The Shows destination's key routing (`add-gui-library-panel` design D3,
//! D14, the port of `lib/library/shows-keys.ts`): the Library keys while a
//! Shows library is active. [`super::keys`] stays the panel's one routing site
//! and hands a Shows key here after the window keys and the open-field checks,
//! so this module never installs a router. The pointer decides the control a key
//! acts on: the row it marks in the tree list, or the Hero's Workspace.
//!
//! The routing is the shared panel key handler's ([`super::keys_shared`]): the movements and Enter never
//! reach the core, Right and Left open and close the tree node under the
//! pointer (the tree meaning design D3 gives them), Esc's overlay and menu
//! handling runs before this, `/` opens the list's filter, and Space stays the
//! core's play and pause.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiShowsDetail;
use crate::bridge::keys::{DomKey, KeyTarget};
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget};

use super::keys_shared::{PanelKeys, is_list_and_hero_key, route_panel_list_key};
use super::list::ListMove;
use super::parts::{library_shows_detail, library_shows_list};
use super::shows_list::ShowsListRow;
use super::shows_workspace::{
    play_shows_episode, queue_shows_episode, shows_episode_at, step_shows_season,
};
use super::state::LibraryState;

/// The Shows facts a routed key reads (row 7C.2), copied off the carried
/// frame: the library index the Workspace commands name.
#[derive(Debug, Clone)]
struct ShowsFrame {
    lib_idx: usize,
}

impl ShowsFrame {
    /// The facts of the carried Shows content, or `None` while another
    /// destination holds the window or the detail part has not landed. A
    /// keydown copies none of the rows.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        let shows = library_shows_list(list.as_deref())?;
        let detail = state.sections.library_detail.peek();
        library_shows_detail(detail.as_deref())?;
        Some(Self {
            lib_idx: shows.lib_idx,
        })
    }
}

/// The Shows facts the shared panel key handler routes over (row 7C.2): the
/// list is a tree whose Right, Left, and Enter act on the node the pointer marks
/// (design D14), the Hero routes its own episode Workspace, and the item the
/// play and queue commands name is a flat row or an episode — a series, a
/// season, and a heading are not playable.
struct ShowsKeys<'a> {
    state: &'a LibraryState,
}

impl<'a> ShowsKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self { state }
    }
}

impl PanelKeys for ShowsKeys<'_> {
    type Frame = ShowsFrame;
    type Item = ShowsListRow;

    fn frame(&self) -> Option<ShowsFrame> {
        ShowsFrame::carry(self.state)
    }

    fn has_hero(&self, _frame: &ShowsFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_shows_detail(detail.as_deref()).is_some_and(|detail| detail.hero.is_some())
    }

    /// The Hero routes its own Workspace, so the core's selected row — often
    /// a series, which is not playable — never names an item.
    fn selected_item_id(&self, _frame: &ShowsFrame) -> Option<String> {
        None
    }

    fn row(&self, pointer: &PointerTarget) -> Option<ShowsListRow> {
        if pointer.list != Some(PointerList::Shows) {
            return None;
        }
        self.state.shows_row_at(pointer.row.as_deref()?)
    }

    fn item_id(&self, item: &ShowsListRow) -> Option<String> {
        match item {
            ShowsListRow::Series(_) | ShowsListRow::Season(_) | ShowsListRow::Heading(_) => None,
            ShowsListRow::Episode(episode) => Some(episode.source_id.clone()),
            ShowsListRow::Flat(row) => (!row.display_only).then(|| row.id.clone()),
        }
    }

    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool {
        if pointer.list != Some(PointerList::Shows) {
            return false;
        }
        self.state.step_shows_scroll(mv);
        true
    }

    fn play_item(&self, item_id: &str) {
        self.state.play_shows_row(item_id);
    }

    fn queue_item(&self, item_id: &str) {
        self.state.queue_shows_row(item_id);
    }

    /// Right on `row` (design D14): a node opens. A leaf has no children, so the
    /// key changes nothing.
    fn open_row(&self, row: &ShowsListRow) {
        if let Some(node_id) = row.as_node() {
            self.state.open_shows_node(node_id);
        }
    }

    /// Left on `row` (design D14): an open node closes. A closed node and a leaf
    /// change nothing.
    fn close_row(&self, row: &ShowsListRow) {
        if let Some(node_id) = row.as_node()
            && self.state.shows_node_open(node_id)
        {
            self.state.close_shows_node(node_id);
        }
    }

    /// Enter on `row` (design D14): a series or season node toggles and selects,
    /// and an episode leaf or a flat row plays.
    fn enter_row(&self, row: &ShowsListRow) {
        if let Some(node_id) = row.as_node() {
            self.state.toggle_shows_node(node_id);
            self.state.select_shows_row(row);
            return;
        }
        if let Some(item_id) = self.item_id(row) {
            self.play_item(&item_id);
        }
    }

    fn route_hero(
        &self,
        key: &DomKey,
        frame: &ShowsFrame,
        pointer: &PointerTarget,
        target: KeyTarget,
    ) -> Option<KeyOutcome> {
        Some(route_workspace_key(
            self.state,
            key,
            frame.lib_idx,
            pointer,
            target,
        ))
    }
}

/// Route a key to the Shows keys (design D12): the shared handler over the
/// shows facts. The movement keys scroll the list under the pointer, and Enter
/// opens or closes a series or season node and plays an episode leaf. `a` acts
/// on the item the pointer's control names — a series row in the tree, an
/// episode in the Workspace — and a series, season, or heading row is not
/// playable, so its `a` changes nothing.
#[must_use]
pub fn route_shows_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    route_panel_list_key(key, state, &ShowsKeys::new(state), pointer)
}

/// Route a key to the Hero's Workspace (design D12, D2 of
/// `gui-pointer-target-interaction`): Left and Right step the season, Enter and
/// `a` act on the episode the pointer marks, and the other list keys change
/// nothing but are consumed. The Workspace keeps no cursor, so a movement key
/// moves no row.
fn route_workspace_key(
    state: &LibraryState,
    key: &DomKey,
    lib_idx: usize,
    pointer: &PointerTarget,
    target: KeyTarget,
) -> KeyOutcome {
    let detail_section = state.sections.library_detail.peek();
    let Some(detail) = library_shows_detail(detail_section.as_deref()) else {
        return KeyOutcome::Core(target);
    };
    match key.key.as_str() {
        "ArrowLeft" => {
            step_shows_season(state, -1, lib_idx, detail);
            KeyOutcome::Handled
        }
        "ArrowRight" => {
            step_shows_season(state, 1, lib_idx, detail);
            KeyOutcome::Handled
        }
        "Enter" | "a" => {
            route_workspace_item_key(state, key, lib_idx, detail, pointer);
            KeyOutcome::Handled
        }
        _ if is_list_and_hero_key(&key.key) => KeyOutcome::Handled,
        _ => KeyOutcome::Core(target),
    }
}

/// Run Enter or `a` against the Workspace episode the pointer marks (design
/// D2): the marker carries the episode's stable Service id, and a marker that
/// names no painted episode sends nothing.
fn route_workspace_item_key(
    state: &LibraryState,
    key: &DomKey,
    lib_idx: usize,
    detail: &GuiShowsDetail,
    pointer: &PointerTarget,
) {
    let Some(episode) = pointer
        .row
        .as_deref()
        .and_then(|row_id| shows_episode_at(detail, row_id))
    else {
        return;
    };
    let episode_id = episode.episode_id.as_str();
    if key.key == "a" {
        queue_shows_episode(state, lib_idx, episode_id);
        return;
    }
    play_shows_episode(state, lib_idx, episode_id);
}

#[cfg(test)]
mod tests;
