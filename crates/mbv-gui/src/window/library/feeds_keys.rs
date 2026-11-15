//! The Feeds destination's key routing (`add-gui-library-panel` design D3,
//! D12, row 11.1, the port of `lib/library/feeds-keys.ts`): the Library keys
//! while the Feeds destination is active. [`super::keys`] stays the panel's
//! one routing site and hands a Feeds key here after the window keys and the
//! open-field checks, so this module never installs a router. The pane focus
//! decides the control a key acts on: the view pills, the list, or the Hero —
//! which carries no Workspace and no cursor.
//!
//! The routing itself is the shared panel key handler's
//! ([`super::keys_shared`], row 7C.2): the movements and Enter never reach
//! the core, Esc's overlay and menu handling runs before this, `/` opens the
//! list's filter, and Space with no focused pill stays the core's play and
//! pause. This module keeps only the Feeds facts it routes over. There is no
//! play-next on the destination: `n` is consumed and does nothing, and `a`
//! sends the queue command. A Client holds one feed tab, so no library index
//! rides the commands.

use dioxus_native::prelude::*;

use crate::bridge::keys::DomKey;
use crate::bridge::library::GuiFeedPill;
use crate::window::input::KeyOutcome;

use super::feeds_list::FeedsItemRow;
use super::keys_shared::{PanelKeys, PillKeys, route_panel_list_key};
use super::list::ListMove;
use super::parts::{library_feeds_detail, library_feeds_list};
use super::state::LibraryState;

/// The Feeds facts a routed key reads (row 7C.2): the pill row's pills.
#[derive(Debug, Clone)]
struct FeedsFrame {
    pills: Vec<GuiFeedPill>,
}

impl FeedsFrame {
    /// The facts of the carried Feeds content, or `None` while another
    /// destination holds the window. A keydown copies the pills — never the
    /// rows.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        let feeds = library_feeds_list(list.as_ref())?;
        Some(Self {
            pills: feeds.pills.iter().map(|frame| frame.pill.clone()).collect(),
        })
    }
}

/// The pill row's facts (row 7C.2): the pills are always painted, and the
/// focus is the pill itself ([`super::feeds`]'s `feed_pill_key`).
struct FeedsPillKeys<'a> {
    state: &'a LibraryState,
}

impl PillKeys for FeedsPillKeys<'_> {
    type Frame = FeedsFrame;
    type Pill = GuiFeedPill;

    fn available(&self, _frame: &FeedsFrame) -> bool {
        true
    }

    fn focused(&self, frame: &FeedsFrame) -> Option<GuiFeedPill> {
        let focused = self.state.feeds_ui.peek().focused_pill.clone()?;
        let focused_key = super::feeds::feed_pill_key(&focused);
        frame
            .pills
            .iter()
            .find(|pill| super::feeds::feed_pill_key(pill) == focused_key)
            .cloned()
    }

    fn request(&self, pill: GuiFeedPill) {
        self.state.request_feeds_pill_select(pill);
    }

    fn move_focus(&self, step: i32) {
        self.state.move_feeds_pill_focus(step);
    }
}

/// The Feeds facts the shared panel key handler routes over (row 7C.2): the
/// view pills are always painted, the item the play and queue commands name
/// is the entry row itself, and there is no play-next.
struct FeedsKeys<'a> {
    state: &'a LibraryState,
    pills: FeedsPillKeys<'a>,
}

impl<'a> FeedsKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self {
            state,
            pills: FeedsPillKeys { state },
        }
    }
}

impl PanelKeys for FeedsKeys<'_> {
    type Frame = FeedsFrame;
    type Item = FeedsItemRow;
    type Pill = GuiFeedPill;

    fn frame(&self) -> Option<FeedsFrame> {
        FeedsFrame::carry(self.state)
    }

    fn pills(&self) -> Option<&dyn PillKeys<Frame = FeedsFrame, Pill = GuiFeedPill>> {
        Some(&self.pills)
    }

    fn has_hero(&self, _frame: &FeedsFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_feeds_detail(detail.as_ref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &FeedsFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_feeds_detail(detail.as_ref()).and_then(|detail| detail.selected.clone())
    }

    fn cursor_row(&self) -> Option<FeedsItemRow> {
        self.state.feeds_cursor_row()
    }

    fn item_id(&self, item: &FeedsItemRow) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list_cursor(&self, mv: ListMove) {
        self.state.move_feeds_list_cursor(mv);
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
/// Feeds facts. Enter on the cursor row plays the entry it rests on — which
/// the Owner replaces the queue with — and `a` queues it.
#[must_use]
pub fn route_feeds_panel_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    route_panel_list_key(key, state, &FeedsKeys::new(state))
}

#[cfg(test)]
mod tests;
