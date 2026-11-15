//! The Podcasts destination's key routing (`add-gui-library-panel` design D3,
//! D12, row 10.1, the port of `lib/library/podcasts-keys.ts`): the Library
//! keys while a Podcasts library is active. [`super::keys`] stays the panel's
//! one routing site and hands a Podcasts key here after the window keys and
//! the open-field checks, so this module never installs a router. The pane
//! focus decides the control a key acts on: the episode pills, the list, or
//! the Hero — which carries no Workspace and no cursor.
//!
//! The routing itself is the shared panel key handler's
//! ([`super::keys_shared`], row 7C.2): the movements and Enter never reach
//! the core, Esc's overlay and menu handling runs before this, `/` opens the
//! list's filter, and Space with no focused pill stays the core's play and
//! pause. This module keeps only the Podcasts facts it routes over. There is
//! no play-next on the destination: `n` is consumed and does nothing, and
//! `a` sends the queue command.

use dioxus_native::prelude::*;

use crate::bridge::keys::DomKey;
use crate::bridge::library::GuiPodcastPill;
use crate::window::input::KeyOutcome;

use super::keys_shared::{PanelKeys, PillKeys, route_panel_list_key};
use super::list::ListMove;
use super::parts::{library_podcasts_detail, library_podcasts_list};
use super::podcasts_list::PodcastsItemRow;
use super::state::LibraryState;

/// The Podcasts facts a routed key reads (row 7C.2): the pill row's pills.
/// The library index rides no key — the item commands read it through the
/// state.
#[derive(Debug, Clone)]
struct PodcastsFrame {
    pills: Vec<GuiPodcastPill>,
}

impl PodcastsFrame {
    /// The facts of the carried Podcasts content, or `None` while another
    /// destination holds the window. A keydown copies the pills — never the
    /// rows.
    fn carry(state: &LibraryState) -> Option<Self> {
        let list = state.sections.library_list.peek();
        let podcasts = library_podcasts_list(list.as_deref())?;
        Some(Self {
            pills: podcasts
                .pills
                .iter()
                .map(|frame| frame.pill.clone())
                .collect(),
        })
    }
}

/// The pill row's facts (row 7C.2): the pills are always painted, and the
/// focus is the pill itself ([`super::podcasts`]'s `podcast_pill_key`).
struct PodcastsPillKeys<'a> {
    state: &'a LibraryState,
}

impl PillKeys for PodcastsPillKeys<'_> {
    type Frame = PodcastsFrame;
    type Pill = GuiPodcastPill;

    fn available(&self, _frame: &PodcastsFrame) -> bool {
        true
    }

    fn focused(&self, frame: &PodcastsFrame) -> Option<GuiPodcastPill> {
        let focused = self.state.podcasts_ui.peek().focused_pill.clone()?;
        let focused_key = super::podcasts::podcast_pill_key(&focused);
        frame
            .pills
            .iter()
            .find(|pill| super::podcasts::podcast_pill_key(pill) == focused_key)
            .cloned()
    }

    fn request(&self, pill: GuiPodcastPill) {
        self.state.request_podcasts_pill_select(pill);
    }

    fn move_focus(&self, step: i32) {
        self.state.move_podcasts_pill_focus(step);
    }
}

/// The Podcasts facts the shared panel key handler routes over (row 7C.2):
/// the episode pills are always painted, the item the play and queue commands
/// name is the episode the cursor row carries, and there is no play-next.
struct PodcastsKeys<'a> {
    state: &'a LibraryState,
    pills: PodcastsPillKeys<'a>,
}

impl<'a> PodcastsKeys<'a> {
    fn new(state: &'a LibraryState) -> Self {
        Self {
            state,
            pills: PodcastsPillKeys { state },
        }
    }
}

impl PanelKeys for PodcastsKeys<'_> {
    type Frame = PodcastsFrame;
    type Item = PodcastsItemRow;
    type Pill = GuiPodcastPill;

    fn frame(&self) -> Option<PodcastsFrame> {
        PodcastsFrame::carry(self.state)
    }

    fn pills(&self) -> Option<&dyn PillKeys<Frame = PodcastsFrame, Pill = GuiPodcastPill>> {
        Some(&self.pills)
    }

    fn has_hero(&self, _frame: &PodcastsFrame) -> bool {
        let detail = self.state.sections.library_detail.peek();
        library_podcasts_detail(detail.as_deref()).is_some_and(|detail| detail.hero.is_some())
    }

    fn selected_item_id(&self, _frame: &PodcastsFrame) -> Option<String> {
        let detail = self.state.sections.library_detail.peek();
        library_podcasts_detail(detail.as_deref()).and_then(|detail| detail.selected.clone())
    }

    fn cursor_row(&self) -> Option<PodcastsItemRow> {
        self.state.podcasts_cursor_row()
    }

    fn item_id(&self, item: &PodcastsItemRow) -> Option<String> {
        Some(item.row.id.clone())
    }

    fn move_list_cursor(&self, mv: ListMove) {
        self.state.move_podcasts_list_cursor(mv);
    }

    fn play_item(&self, item_id: &str) {
        self.state.play_podcasts_row(item_id);
    }

    fn queue_item(&self, item_id: &str) {
        self.state.queue_podcasts_row(item_id);
    }

    // A podcast episode has no play-next: the shared default consumes `n` and
    // sends nothing.
}

/// Route a key to the Podcasts keys (row 10.1): the shared handler over the
/// Podcasts facts. Enter on the cursor row plays the episode it rests on —
/// which the Owner replaces the queue with — and `a` queues it.
#[must_use]
pub fn route_podcasts_panel_key(state: &LibraryState, key: &DomKey) -> KeyOutcome {
    route_panel_list_key(key, state, &PodcastsKeys::new(state))
}

#[cfg(test)]
mod tests;
