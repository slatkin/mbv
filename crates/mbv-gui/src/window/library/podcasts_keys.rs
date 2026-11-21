//! The Podcasts destination's key routing (`add-gui-library-panel` design D3,
//! D12, row 10.1, the port of `lib/library/podcasts-keys.ts`): the Library
//! keys while a Podcasts library is active. [`super::keys`] stays the panel's
//! one routing site and hands a Podcasts key here after the window keys and
//! the open-field checks, so this module never installs a router. The pointer
//! decides the control a key acts on: the list, or the Hero — which carries no
//! Workspace and no cursor.
//!
//! The routing itself is the shared panel key handler's
//! ([`super::keys_shared`], row 7C.2): the movements and Enter never reach
//! the core, Esc's overlay and menu handling runs before this, `/` opens the
//! list's filter, and Space stays the core's play and pause. This module keeps only the Podcasts facts it routes over. There is
//! no play-next on the destination: `n` is consumed and does nothing, and
//! `a` sends the queue command.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEntryRowFrame;
use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;
use crate::window::pointer_target::{PointerList, PointerTarget};

use super::keys_shared::{PanelKeys, route_panel_list_key};
use super::lettered::{LetteredDestination, LetteredItemRow, PodcastsList};
use super::list::ListMove;
use super::parts::{library_podcasts_detail, library_podcasts_list};
use super::state::LibraryState;

/// The Podcasts facts a routed key reads (row 7C.2): the destination is
/// active. The library index rides no key — the item commands read it through
/// the state.
#[derive(Debug, Clone)]
struct PodcastsFrame;

impl PodcastsFrame {
    /// The facts of the carried Podcasts content, or `None` while another
    /// destination holds the window. A keydown copies none of the rows.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        library_podcasts_list(list.as_deref())?;
        Some(Self)
    }
}

/// The Podcasts facts the shared panel key handler routes over (row 7C.2):
/// the item the play and queue commands name is the episode the pointer marks,
/// and there is no play-next.
struct PodcastsKeys<'a> {
    state: &'a LibraryState,
}

impl<'a> PodcastsKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self { state }
    }
}

impl PanelKeys for PodcastsKeys<'_> {
    type Frame = PodcastsFrame;
    type Item = LetteredItemRow<GuiEntryRowFrame>;

    fn frame(&self) -> Option<PodcastsFrame> {
        PodcastsFrame::carry(self.state)
    }

    fn has_hero(&self, _frame: &PodcastsFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_podcasts_detail(detail.as_deref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &PodcastsFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_podcasts_detail(detail.as_deref()).and_then(|detail| detail.selected.clone())
    }

    fn row(&self, pointer: &PointerTarget) -> Option<LetteredItemRow<GuiEntryRowFrame>> {
        if pointer.list != Some(PointerList::Podcasts) {
            return None;
        }
        self.state.podcasts_row_at(pointer.row.as_deref()?)
    }

    fn item_id(&self, item: &LetteredItemRow<GuiEntryRowFrame>) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list(&self, mv: ListMove, pointer: &PointerTarget) -> bool {
        if pointer.list != Some(PointerList::Podcasts) {
            return false;
        }
        self.state.step_lettered_scroll::<PodcastsList>(mv);
        true
    }

    fn play_item(&self, item_id: &str) {
        PodcastsList::play(self.state, item_id);
    }

    fn queue_item(&self, item_id: &str) {
        PodcastsList::queue(self.state, item_id);
    }

    // A podcast episode has no play-next: the shared default consumes `n` and
    // sends nothing.
}

/// Route a key to the Podcasts keys (row 10.1): the shared handler over the
/// Podcasts facts. Enter on the hovered episode plays it — which the Owner
/// replaces the queue with — and `a` queues it.
#[must_use]
pub fn route_podcasts_panel_key(
    state: &LibraryState,
    key: &DomKey,
    pointer: &PointerTarget,
) -> KeyOutcome {
    route_panel_list_key(key, state, &PodcastsKeys::new(state), pointer)
}

#[cfg(test)]
mod tests;
