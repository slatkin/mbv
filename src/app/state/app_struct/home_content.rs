//! The App's Home content writes (add-gui-library-panel row 6.2): the
//! Continue Watching snapshot a `HomeContentRefreshed` event lands in and
//! the clear a `HomeContentCleared` teardown runs. The write rule lives on
//! the `App` so both Clients apply it — the presenting Client's shell drain
//! joins only its projection push, and a Client that keeps no shell state
//! (the GUI) runs the same write through its own drain.

use super::App;
use crate::app::state::playback::HomeContent;

impl App {
    /// Applies one Home content write to the App's stored snapshot
    /// (`openspec/specs/home-latest-sections`): a refresh replaces the
    /// Continue Watching snapshot and a clear empties it — `loading`
    /// intentionally untouched, matching the legacy `clear_emby_memory` —
    /// and either write evicts `Recently added` sources whose Service
    /// library no longer exists (a rebuilt library set, or a removed or
    /// replaced setup). The presenting Client's shell re-projects after the
    /// write returns.
    pub(in crate::app) fn apply_home_content(&mut self, content: Option<HomeContent>) {
        match content {
            Some(content) => self.home_content = content,
            None => self.home_content.continue_items.clear(),
        }
        self.evict_gone_home_recently_added();
    }
}
