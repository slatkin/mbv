//! The plain Emby-library destination's key routing (`add-gui-library-panel`
//! design D3, D12, row 8.1, the port of `lib/library/emby-keys.ts`): the
//! Library keys while an Emby library is active. [`super::keys`] stays the
//! panel's one routing site and hands an Emby key here after the window keys
//! and the open-field checks, so this module never installs a router. The pane
//! focus decides the control a key acts on: the feed-group pills, the list, or
//! the Hero — which carries no Workspace and no cursor.
//!
//! The routing itself is the shared panel key handler's ([`super::keys_shared`],
//! row 7C.2): the movements and Enter never reach the core, Esc's overlay and
//! menu handling runs before this, `/` opens the list's filter, and Space with
//! no focused pill — and in plain mode, which paints none — stays the core's
//! play and pause. This module keeps only the Emby facts it routes over: the
//! feed-group pills that exist in feed-group mode only, and `n`'s play-next.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyFeedGroupFrame;
use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;

use super::emby_list::EmbyItemRow;
use super::keys_shared::{PanelKeys, PillKeys, route_panel_list_key};
use super::list::ListMove;
use super::parts::{library_emby_detail, library_emby_list};
use super::state::LibraryState;

/// The Emby facts a routed key reads (row 7C.2): the mode and the feed-group
/// pills the pill row routes over, read off the carried frame. The rows stay
/// behind — the list cursor reads them through the state, and a keydown copies
/// none of them (design D6).
#[derive(Debug, Clone)]
struct EmbyFrame {
    /// Whether the frame paints the feed-group pill row.
    feed_group: bool,
    /// The carried pills' groups, in paint order.
    pills: Vec<GuiEmbyFeedGroupFrame>,
}

impl EmbyFrame {
    /// The facts of the carried Emby content, or `None` while another
    /// destination holds the window.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        let emby = library_emby_list(list.as_ref())?;
        Some(Self {
            feed_group: emby.feed_group,
            pills: emby.pills.iter().map(|pill| pill.group.clone()).collect(),
        })
    }
}

/// The feed-group pill row's facts (row 8.1): the pills exist in feed-group
/// mode only, and the focus is the group's stable key ([`super::emby_list`]'s
/// `emby_group_key`).
struct EmbyPillKeys<'a> {
    state: &'a LibraryState,
}

impl PillKeys for EmbyPillKeys<'_> {
    type Frame = EmbyFrame;
    type Pill = GuiEmbyFeedGroupFrame;

    fn available(&self, frame: &EmbyFrame) -> bool {
        frame.feed_group
    }

    fn focused(&self, frame: &EmbyFrame) -> Option<GuiEmbyFeedGroupFrame> {
        let focused = self.state.emby_ui.peek().focused_pill.clone()?;
        frame
            .pills
            .iter()
            .find(|group| super::emby_list::emby_group_key(group) == focused)
            .cloned()
    }

    fn request(&self, pill: GuiEmbyFeedGroupFrame) {
        self.state.request_emby_pill_select(pill);
    }

    fn move_focus(&self, step: i32) {
        self.state.move_emby_pill_focus(step);
    }
}

/// The Emby facts the shared panel key handler routes over (row 7C.2): the
/// feed-group pills are painted in feed-group mode only, the item the play
/// and queue commands name is the row's item, and `n` plays it next.
struct EmbyKeys<'a> {
    state: &'a LibraryState,
    pills: EmbyPillKeys<'a>,
}

impl<'a> EmbyKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self {
            state,
            pills: EmbyPillKeys { state },
        }
    }
}

impl PanelKeys for EmbyKeys<'_> {
    type Frame = EmbyFrame;
    type Item = EmbyItemRow;
    type Pill = GuiEmbyFeedGroupFrame;

    fn frame(&self) -> Option<EmbyFrame> {
        EmbyFrame::carry(self.state)
    }

    fn pills(&self) -> Option<&dyn PillKeys<Frame = EmbyFrame, Pill = GuiEmbyFeedGroupFrame>> {
        Some(&self.pills)
    }

    fn has_hero(&self, _frame: &EmbyFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_emby_detail(detail.as_ref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &EmbyFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_emby_detail(detail.as_ref()).and_then(|detail| detail.selected.clone())
    }

    fn cursor_row(&self) -> Option<EmbyItemRow> {
        self.state.emby_cursor_row()
    }

    fn item_id(&self, item: &EmbyItemRow) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list_cursor(&self, mv: ListMove) {
        self.state.move_emby_list_cursor(mv);
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
/// the Emby facts. Enter on the cursor row plays it — a folder or a boxset
/// too, through the core's folder-play rule — and `n` and `a` act on the item
/// the focused pane names.
#[must_use]
pub fn route_emby_panel_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    route_panel_list_key(key, state, &EmbyKeys::new(state))
}

#[cfg(test)]
mod tests;
