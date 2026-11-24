//! The hover controls a Library row offers (`gui-row-hover-actions` design D3,
//! D4): the set of presses one row paints, and the one builder the row painters
//! call, so a row's control and the key or menu entry it mirrors cannot
//! drift apart.

use dioxus_native::prelude::*;

use crate::gui_core::GuiLibraryMenuRow;

use super::LibraryState;

/// The hover controls a Library row offers (gui-row-hover-actions design D3,
/// D4): one press per control the row has, and `None` for a control its row
/// does not offer. A painter passes the set to the shared
/// `RowHoverActions` part, which paints only the controls it receives.
#[derive(Debug, Clone, Default)]
pub struct RowHoverPresses {
    /// Play the row.
    pub play: Option<Callback<()>>,
    /// Add the row to the queue.
    pub queue: Option<Callback<()>>,
    /// Shuffle the row's items.
    pub shuffle: Option<Callback<()>>,
}

impl LibraryState {
    /// The press that runs the row's own row-menu shuffle entry (gui-row-
    /// hover-actions design D4): the hover Shuffle control and the menu entry
    /// run one path, so the two cannot drift apart. A row whose menu carries no
    /// shuffle entry changes nothing, so a painter offers the control only where
    /// the menu has the entry.
    #[must_use]
    pub fn shuffle_press(&self, row: GuiLibraryMenuRow) -> Callback<()> {
        let shuffle = self.clone();
        Callback::new(move |()| shuffle.shuffle_library_row(row.clone()))
    }

    /// The hover controls of one item row (gui-row-hover-actions design D3,
    /// D4): Play and Add run the destination's own play and queue paths — the
    /// same ones its keys and its double-click run — and Shuffle runs the row's
    /// own menu entry on a row whose menu carries one (`offers_shuffle`).
    #[must_use]
    pub fn item_hover_presses(
        &self,
        item_id: String,
        play: fn(&Self, &str),
        queue: fn(&Self, &str),
        offers_shuffle: bool,
    ) -> RowHoverPresses {
        let play_state = self.clone();
        let play_id = item_id.clone();
        let queue_state = self.clone();
        let queue_id = item_id.clone();
        RowHoverPresses {
            play: Some(Callback::new(move |()| play(&play_state, &play_id))),
            queue: Some(Callback::new(move |()| queue(&queue_state, &queue_id))),
            shuffle: offers_shuffle.then(|| self.shuffle_press(GuiLibraryMenuRow::Item(item_id))),
        }
    }
}
