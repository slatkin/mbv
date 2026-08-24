//! Precedence-gate sync for the shell `Model` (see
//! `components::playback` attributes.
//!
//! Mirrors the two `App` fields that gate `CONTEXT_STACK` entries above
//! `view_dispatch` on ephemeral state (`skip_intro_end_ticks`, `next_up_item`)
//! into `PlaybackComponent`'s attributes every tick, the same "shell
//! mirrors App state into a mounted component" pattern `sync_home` uses --
//! except via `Application::attr` (attribute storage a future `SubClause`
//! guard can read) rather than a downcast+setter, since this component is
//! never rendered or downcast into.

use super::components::{
    ComponentId, ATTR_ALBUM_TRACK_FOCUSED, ATTR_BLOCKING_OVERLAY_ACTIVE, ATTR_LIB_SEARCH_ACTIVE,
    ATTR_NEXT_UP_PROMPT_VISIBLE, ATTR_SKIP_INTRO_PROMPT_VISIBLE,
};
use super::shell::Model;
use tuirealm::props::AttrValue;

impl Model {
    pub(super) fn sync_precedence_gates(&mut self) {
        let skip_intro_visible = self.app.skip_intro_end_ticks.is_some();
        let next_up_visible = self.app.next_up_item.is_some();
        let blocking_overlay_active = self.blocking_overlay_active();
        let (lib_search_active, album_track_focused) = match self.app.tab {
            super::TabSelection::EmbyLibrary(index) => self
                .app
                .libs
                .get(index)
                .map(|library| {
                    (
                        library.search.is_some(),
                        library.album_track_focus.is_some(),
                    )
                })
                .unwrap_or((false, false)),
            _ => (false, false),
        };
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
        let _ = self.application.attr(
            &ComponentId::Playback,
            ATTR_BLOCKING_OVERLAY_ACTIVE,
            AttrValue::Flag(blocking_overlay_active),
        );
        let _ = self.application.attr(
            &ComponentId::Playback,
            ATTR_LIB_SEARCH_ACTIVE,
            AttrValue::Flag(lib_search_active),
        );
        let _ = self.application.attr(
            &ComponentId::Playback,
            ATTR_ALBUM_TRACK_FOCUSED,
            AttrValue::Flag(album_track_focused),
        );
    }
}
