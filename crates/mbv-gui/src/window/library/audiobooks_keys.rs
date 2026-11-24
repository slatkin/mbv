//! The Audiobooks destination's key routing (`add-gui-library-panel` design
//! D3, D12, row 9.1, the port of `lib/library/audiobooks-keys.ts`): the
//! Library keys while an Audiobooks library is active. [`super::keys`] stays
//! the panel's one routing site and hands an Audiobooks key here after the
//! window keys and the open-field checks, so this module never installs a
//! router. The pointer decides the control a key acts on: the book list, or the
//! Hero's chapter Workspace.
//!
//! The routing itself is the shared panel key handler's ([`super::keys_shared`],
//! row 7C.2): the movements and Enter never reach the core, Esc's overlay and
//! menu handling runs before this, `/` opens the list's filter, and Space
//! stays the core's play and pause — Audiobooks paints no pill row. This
//! module keeps only the Audiobooks facts it routes over: Enter on the hovered
//! chapter, and `a`'s queue command. `n` is not a Library key, so it goes to
//! the core's router.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiAudiobooksRowFrame;
use crate::bridge::keys::{DomKey, KeyTarget};
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget};

use super::audiobooks_workspace::{abs_chapter_at, play_abs_chapter, queue_abs_book};
use super::keys_shared::{PanelKeys, is_list_and_hero_key, route_panel_list_key};
use super::lettered::{AudiobooksList, LetteredDestination, LetteredItemRow};
use super::list::ListMove;
use super::parts::{library_audiobooks_detail, library_audiobooks_list};
use super::state::LibraryState;

/// The Audiobooks facts a routed key reads (row 7C.2), copied off the carried
/// frame: the library index the book commands name.
#[derive(Debug, Clone, Copy)]
struct AudiobooksFrame {
    lib_idx: usize,
}

impl AudiobooksFrame {
    /// The facts of the carried Audiobooks content, or `None` while another
    /// destination holds the window.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        library_audiobooks_list(list.as_deref()).map(|audiobooks| Self {
            lib_idx: audiobooks.lib_idx,
        })
    }
}

/// The Audiobooks facts the shared panel key handler routes over (row 7C.2):
/// the item the play and queue commands name is the book the pointer marks, and
/// the Hero routes its own chapter Workspace.
struct AudiobooksKeys<'a> {
    state: &'a LibraryState,
}

impl PanelKeys for AudiobooksKeys<'_> {
    type Frame = AudiobooksFrame;
    type Item = LetteredItemRow<GuiAudiobooksRowFrame>;

    fn frame(&self) -> Option<AudiobooksFrame> {
        AudiobooksFrame::carry(self.state)
    }

    fn has_hero(&self, _frame: &AudiobooksFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_audiobooks_detail(detail.as_deref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &AudiobooksFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_audiobooks_detail(detail.as_deref()).and_then(|detail| detail.selected.clone())
    }

    fn row(&self, pointer: &PointerTarget) -> Option<LetteredItemRow<GuiAudiobooksRowFrame>> {
        if pointer.list != Some(PointerList::Audiobooks) {
            return None;
        }
        self.state.audiobooks_row_at(pointer.row.as_deref()?)
    }

    fn item_id(&self, item: &LetteredItemRow<GuiAudiobooksRowFrame>) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool {
        if pointer.list != Some(PointerList::Audiobooks) {
            return false;
        }
        self.state.step_lettered_scroll::<AudiobooksList>(mv);
        true
    }

    fn play_item(&self, item_id: &str) {
        AudiobooksList::play(self.state, item_id);
    }

    fn queue_item(&self, item_id: &str) {
        AudiobooksList::queue(self.state, item_id);
    }

    fn route_hero(
        &self,
        key: &DomKey,
        frame: &AudiobooksFrame,
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

/// Route a key to the Hero's chapter Workspace (row 9.1): Enter plays or seeks
/// from the chapter the pointer marks, and `a` queues the selected book. No
/// movement key moves a row, because the Workspace keeps no cursor; `n` goes to
/// the core.
fn route_workspace_key(
    state: &LibraryState,
    key: &DomKey,
    lib_idx: usize,
    pointer: &PointerTarget,
    target: KeyTarget,
) -> KeyOutcome {
    let detail_section = state.sections.library_detail.peek();
    let Some(detail) = library_audiobooks_detail(detail_section.as_deref()) else {
        return KeyOutcome::Core(target);
    };
    match key.key.as_str() {
        "Enter" => {
            let chapter = pointer
                .row
                .as_deref()
                .and_then(|row| abs_chapter_at(detail, row))
                .map(|row| row.chapter);
            if let Some(chapter) = chapter {
                play_abs_chapter(state, lib_idx, detail, chapter);
            }
            KeyOutcome::Handled
        }
        "a" => {
            queue_abs_book(state, lib_idx, detail);
            KeyOutcome::Handled
        }
        _ if is_list_and_hero_key(&key.key) => KeyOutcome::Handled,
        _ => KeyOutcome::Core(target),
    }
}

/// Route a key to the Audiobooks keys (row 9.1): the shared handler over the
/// Audiobooks facts. Enter on the book list's hovered book plays it — which the
/// Owner replaces the queue with at the book's saved position.
#[must_use]
pub fn route_audiobooks_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    route_panel_list_key(key, state, &AudiobooksKeys { state }, pointer)
}

#[cfg(test)]
mod tests;
