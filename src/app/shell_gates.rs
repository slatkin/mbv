//! Precedence-gate sync for the shell `Model` (see
//! `components::playback_gates` module docs).
//!
//! Mirrors the two `App` fields that gate `CONTEXT_STACK` entries above
//! `view_dispatch` on ephemeral state (`skip_intro_end_ticks`, `next_up_item`)
//! into `PlaybackGatesComponent`'s attributes every tick, the same "shell
//! mirrors App state into a mounted component" pattern `sync_home` uses --
//! except via `Application::attr` (attribute storage a future `SubClause`
//! guard can read) rather than a downcast+setter, since this component is
//! never rendered or downcast into.

use super::components::{ComponentId, ATTR_NEXT_UP_PROMPT_VISIBLE, ATTR_SKIP_INTRO_PROMPT_VISIBLE};
use super::shell::Model;
use tuirealm::props::AttrValue;

impl Model {
    pub(super) fn sync_precedence_gates(&mut self) {
        let skip_intro_visible = self.app.skip_intro_end_ticks.is_some();
        let next_up_visible = self.app.next_up_item.is_some();
        let _ = self.application.attr(
            &ComponentId::Playback,
            ATTR_SKIP_INTRO_PROMPT_VISIBLE,
            AttrValue::Flag(skip_intro_visible),
        );
        let _ = self.application.attr(
            &ComponentId::Playback,
            ATTR_NEXT_UP_PROMPT_VISIBLE,
            AttrValue::Flag(next_up_visible),
        );
    }
}
