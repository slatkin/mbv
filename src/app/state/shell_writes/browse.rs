//! The catalogue, playlist, and artwork-log facts a Client changes.
//!
//! Each write keeps its rule in the core: the level a filter clear empties,
//! the resting row the persistence reads, the gate that logs one title-site
//! decision per change. The Client supplies only what it resolved (the
//! library index, the row the active control named, the decision it made).

use crate::app::App;

impl App {
    /// Return the Emby library at `index` to its unfiltered scope after a
    /// letter-filter clear: the level drops its filter, its rows, and its
    /// prefetched snapshot, and paints as loading until the refresh lands.
    /// Answers `false` when the library has no open level to clear.
    pub fn clear_library_letter_filter(&mut self, index: usize) -> bool {
        let Some(level) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.last_mut())
        else {
            return false;
        };
        level.letter_filter = None;
        level.set_resting_cursor(0);
        level.set_resting_scroll(0);
        level.loading = true;
        level.items.clear();
        level.all_items = None;
        true
    }

    /// Forget the deep-selection track a navigated album landing armed: the
    /// landing was rejected, so no album activation may adopt it.
    pub fn abandon_track_selection(&mut self) {
        self.pending_track_selection = None;
    }

    /// Hold the resolved row as the resting cursor of the Emby library's open
    /// level: the persistence-facing value, not a live mirror. The caller
    /// resolved the row from the active control and validated it.
    pub fn rest_level_cursor(&mut self, index: usize, row: usize) {
        if let Some(level) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.nav_stack.last_mut())
        {
            level.set_resting_cursor(row);
        }
    }

    /// Hold the resolved row as the cursor of the Emby library's
    /// feed/home-video group view.
    pub fn rest_feed_home_video_cursor(&mut self, index: usize, row: usize) {
        if let Some(state) = self
            .libs
            .get_mut(index)
            .and_then(|tab| tab.feed_home_video.as_mut())
        {
            state.video_cursor = row;
        }
    }

    /// Close the playlist the Playlists overlay has open and drop its rows: the
    /// overlay stepped back to the playlist list.
    pub fn close_open_playlist(&mut self) {
        self.playlists_open = None;
        self.playlists_open_items.clear();
    }

    /// Hold `entry` as the playlist the Playlists overlay has open, with the
    /// rows it holds.
    pub fn adopt_open_playlist(
        &mut self,
        entry: mbv_emby_model::EmbyItem,
        rows: Vec<mbv_emby_model::EmbyItem>,
    ) {
        self.playlists_open = Some(entry);
        self.playlists_open_items = rows;
    }

    /// Log one queue title-site decision (invariant 16's diagnostic). Both
    /// Clients reach the same decision, so the gate dedupes it in the core.
    pub fn log_queue_title_decision(
        &mut self,
        identity: &str,
        item_kind: &str,
        reason: &'static str,
        base_dimensions: Option<(u32, u32)>,
    ) {
        self.title_log_gate
            .log_decision(identity, item_kind, reason, base_dimensions);
    }
}
