//! The Feeds destination's key routing (`add-gui-library-panel` design D3,
//! D12, row 11.1, the port of `lib/library/feeds-keys.ts`): the Library keys
//! while the Feeds destination is active. [`super::keys`] stays the panel's
//! one routing site and hands a Feeds key here after the window keys and the
//! open-field checks, so this module never installs a router. The pointer
//! decides the control a key acts on: the list, or the Hero — which carries no
//! Workspace and no cursor.
//!
//! Enter, `n`, and `a` act on the row the pointer marks, and the Hero's entry
//! Workspace has its own router, so Enter plays the entry the pointer marks
//! there and `a` queues it (design D2, D6).
//!
//! The routing itself is the shared panel key handler's
//! ([`super::keys_shared`], row 7C.2): the movements and Enter never reach
//! the core, Esc's overlay and menu handling runs before this, `/` opens the
//! list's filter, and Space stays the core's play and pause. This module keeps only the Feeds facts it routes over. There is no
//! play-next on the destination: `n` is consumed and does nothing, and `a`
//! sends the queue command. A Client holds one feed tab, so no library index
//! rides the commands.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEntryRowFrame;
use crate::bridge::keys::{DomKey, KeyTarget};
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget};

use super::keys_shared::{PanelKeys, is_list_and_hero_key, route_panel_list_key};
use super::lettered::{FeedsList, LetteredDestination, LetteredItemRow};
use super::list::ListMove;
use super::parts::{library_feeds_detail, library_feeds_list};
use super::state::LibraryState;

/// The Feeds facts a routed key reads (row 7C.2): the destination is active.
#[derive(Debug, Clone)]
struct FeedsFrame;

impl FeedsFrame {
    /// The facts of the carried Feeds content, or `None` while another
    /// destination holds the window. A keydown copies none of the rows.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        library_feeds_list(list.as_deref())?;
        Some(Self)
    }
}

/// The Feeds facts the shared panel key handler routes over (row 7C.2): the
/// item the play and queue commands name is the entry the pointer marks, and
/// there is no play-next.
struct FeedsKeys<'a> {
    state: &'a LibraryState,
}

impl<'a> FeedsKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self { state }
    }
}

impl PanelKeys for FeedsKeys<'_> {
    type Frame = FeedsFrame;
    type Item = LetteredItemRow<GuiEntryRowFrame>;

    fn frame(&self) -> Option<FeedsFrame> {
        FeedsFrame::carry(self.state)
    }

    fn has_hero(&self, _frame: &FeedsFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_feeds_detail(detail.as_deref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &FeedsFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_feeds_detail(detail.as_deref()).and_then(|detail| detail.selected.clone())
    }

    fn row(&self, pointer: &PointerTarget) -> Option<LetteredItemRow<GuiEntryRowFrame>> {
        if pointer.list != Some(PointerList::Feeds) {
            return None;
        }
        self.state.feeds_row_at(pointer.row.as_deref()?)
    }

    fn item_id(&self, item: &LetteredItemRow<GuiEntryRowFrame>) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool {
        if pointer.list != Some(PointerList::Feeds) {
            return false;
        }
        self.state.step_lettered_scroll::<FeedsList>(mv);
        true
    }

    fn play_item(&self, item_id: &str) {
        FeedsList::play(self.state, item_id);
    }

    fn queue_item(&self, item_id: &str) {
        FeedsList::queue(self.state, item_id);
    }

    fn route_hero(
        &self,
        key: &DomKey,
        _frame: &FeedsFrame,
        pointer: &PointerTarget,
        target: KeyTarget,
    ) -> Option<KeyOutcome> {
        Some(route_workspace_key(self.state, key, pointer, target))
    }

    // A feed entry has no play-next: the shared default consumes `n` and
    // sends nothing.
}

/// Route a key to the Hero's entry Workspace (design D2, D6): Enter plays the
/// entry the pointer marks and `a` queues it. No movement key moves a row,
/// because the Workspace keeps no cursor, and `n` is consumed because Feeds has
/// no play-next.
fn route_workspace_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
    target: KeyTarget,
) -> KeyOutcome {
    match key.key.as_str() {
        "Enter" | "a" => {
            route_workspace_item_key(state, key, pointer);
            KeyOutcome::Handled
        }
        "n" => KeyOutcome::Handled,
        _ if is_list_and_hero_key(&key.key) => KeyOutcome::Handled,
        _ => KeyOutcome::Core(target),
    }
}

/// Run Enter or `a` against the Workspace entry the pointer marks (design D2):
/// the marker carries the entry's stable id, and a marker that names no painted
/// entry sends nothing.
fn route_workspace_item_key(state: &LibraryState, key: &DomKey, pointer: &PointerTarget) {
    let Some(entry) = pointer
        .row
        .as_deref()
        .and_then(|row| state.feeds_hero_row_at(row))
    else {
        return;
    };
    if key.key == "a" {
        state.queue_feeds_row(&entry.id);
        return;
    }
    state.play_feeds_row(&entry.id);
}

/// Route a key to the Feeds keys (row 11.1): the shared handler over the
/// Feeds facts. Enter on the hovered entry plays it — which the Owner replaces
/// the queue with — and `a` queues it.
#[must_use]
pub fn route_feeds_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    route_panel_list_key(key, state, &FeedsKeys::new(state), pointer)
}

#[cfg(test)]
mod tests;
