//! The plain Emby-library destination's key routing (`add-gui-library-panel`
//! design D3, D12, row 8.1, the port of `lib/library/emby-keys.ts`): the
//! Library keys while an Emby library is active. [`super::keys`] stays the
//! panel's one routing site and hands an Emby key here after the window keys
//! and the open-field checks, so this module never installs a router. The
//! pointer decides the control a key acts on: the row it marks, or the Hero —
//! which carries no Workspace and no cursor.
//!
//! The routing itself is the shared panel key handler's ([`super::keys_shared`],
//! row 7C.2): the movements and Enter never reach the core, Esc's overlay and
//! menu handling runs before this, `/` opens the list's filter, and Space stays
//! the core's play and pause. This module keeps only the Emby facts it routes
//! over: the hovered row, `n`'s play-next, and the list's scroll.

use dioxus_native::prelude::*;

use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget};

use super::emby_list::EmbyItemRow;
use super::keys_shared::{PanelKeys, route_panel_list_key};
use super::list::ListMove;
use super::parts::{library_emby_detail, library_emby_list};
use super::state::LibraryState;

/// The Emby facts a routed key reads (row 7C.2): the destination is active. The
/// rows stay behind — the pointer's row resolves through the state, and a
/// keydown copies none of them (design D6).
#[derive(Debug, Clone)]
struct EmbyFrame;

impl EmbyFrame {
    /// The facts of the carried Emby content, or `None` while another
    /// destination holds the window.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        library_emby_list(list.as_deref())?;
        Some(Self)
    }
}

/// The Emby facts the shared panel key handler routes over (row 7C.2): the
/// item the play and queue commands name is the row the pointer marks, and `n`
/// plays it next.
struct EmbyKeys<'a> {
    state: &'a LibraryState,
}

impl<'a> EmbyKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self { state }
    }
}

impl PanelKeys for EmbyKeys<'_> {
    type Frame = EmbyFrame;
    type Item = EmbyItemRow;

    fn frame(&self) -> Option<EmbyFrame> {
        EmbyFrame::carry(self.state)
    }

    fn has_hero(&self, _frame: &EmbyFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_emby_detail(detail.as_deref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &EmbyFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_emby_detail(detail.as_deref()).and_then(|detail| detail.selected.clone())
    }

    fn row(&self, pointer: &PointerTarget) -> Option<EmbyItemRow> {
        if pointer.list != Some(PointerList::Emby) {
            return None;
        }
        self.state.emby_row_at(pointer.row.as_deref()?)
    }

    fn item_id(&self, item: &EmbyItemRow) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool {
        if pointer.list != Some(PointerList::Emby) {
            return false;
        }
        self.state.step_emby_scroll(mv);
        true
    }

    fn play_item(&self, item_id: &str) {
        self.state.play_emby_row(item_id);
    }

    fn queue_item(&self, item_id: &str) {
        self.state.queue_emby_row(item_id);
    }

    fn play_next_item(&self, item_id: &str) -> bool {
        self.state.play_emby_row_next(item_id);
        true
    }
}

/// Route a key to the Emby library's keys (row 8.1): the shared handler over
/// the Emby facts. Enter on the hovered row plays it — a folder or a boxset too,
/// through the core's folder-play rule — and `n` and `a` act on the same row.
#[must_use]
pub fn route_emby_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    route_panel_list_key(key, state, &EmbyKeys::new(state), pointer)
}

#[cfg(test)]
mod tests;
