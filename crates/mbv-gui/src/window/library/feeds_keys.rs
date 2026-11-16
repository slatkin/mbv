//! The Feeds destination's key routing (`add-gui-library-panel` design D3,
//! D12, row 11.1, the port of `lib/library/feeds-keys.ts`): the Library keys
//! while the Feeds destination is active. [`super::keys`] stays the panel's
//! one routing site and hands a Feeds key here after the window keys and the
//! open-field checks, so this module never installs a router. The pointer
//! decides the control a key acts on: the list, or the Hero — which carries no
//! Workspace and no cursor.
//!
//! The routing itself is the shared panel key handler's
//! ([`super::keys_shared`], row 7C.2): the movements and Enter never reach
//! the core, Esc's overlay and menu handling runs before this, `/` opens the
//! list's filter, and Space stays the core's play and pause. This module keeps only the Feeds facts it routes over. There is no
//! play-next on the destination: `n` is consumed and does nothing, and `a`
//! sends the queue command. A Client holds one feed tab, so no library index
//! rides the commands.

use dioxus_native::prelude::*;

use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget};

use super::feeds_list::FeedsItemRow;
use super::keys_shared::{PanelKeys, route_panel_list_key};
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
    type Item = FeedsItemRow;

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

    fn row(&self, pointer: &PointerTarget) -> Option<FeedsItemRow> {
        if pointer.list != Some(PointerList::Feeds) {
            return None;
        }
        self.state.feeds_row_at(pointer.row.as_deref()?)
    }

    fn item_id(&self, item: &FeedsItemRow) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool {
        if pointer.list != Some(PointerList::Feeds) {
            return false;
        }
        self.state.step_feeds_scroll(mv);
        true
    }

    fn play_item(&self, item_id: &str) {
        self.state.play_feeds_row(item_id);
    }

    fn queue_item(&self, item_id: &str) {
        self.state.queue_feeds_row(item_id);
    }

    // A feed entry has no play-next: the shared default consumes `n` and
    // sends nothing.
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
