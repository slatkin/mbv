//! The Player-link, queue, and Home facts a Client changes, and the Settings
//! destination write that a reset pairs with the presentation defaults.
//!
//! The Player-link writes name the link and the intent, never the proxy the
//! core holds: a `SwapPrepared` reply travels on the connection the swap
//! arrived on, so the link names it and the core sends.

use crate::app::App;
use crate::app::QueueLink;
use mbv_ctrl::CtrlCmd;

impl App {
    /// Send `swap_prepared` on the connection the pin swap arrived on: the
    /// active Player link, or the suspended home link. The home link carries
    /// the swap whenever one is suspended, and the caller reached here from an
    /// event that link delivered, so a missing home link answers `false` as a
    /// dead connection does.
    pub fn reply_pin_swap_prepared(&mut self, link: QueueLink) -> bool {
        let reply = match link {
            QueueLink::Local => &self.player,
            QueueLink::Peer => match self.suspended_local.as_ref() {
                Some(home) => &home.player,
                None => return false,
            },
        };
        reply.send_ctrl_cmd(CtrlCmd::SwapPrepared)
    }

    /// Hold the cover-paint size of the queue card as the geometry
    /// checkpoint. The card painter supplies the size it painted, and passes
    /// `measure` `false` for a paint through the dimmed backdrop's halfblock
    /// fallback (#451): its hardcoded font grid measures cells differently,
    /// so recording it would move the checkpoint on every modal open and
    /// close.
    pub fn record_card_size(&mut self, height: u16, width: u16, measure: bool) {
        if measure {
            self.images.record_card_size(height, width);
        }
    }

    /// Arm the one-shot cursor re-anchor that follows an explicit jump to the
    /// now-playing row.
    pub fn reanchor_queue_cursor_to_playing(&mut self) {
        self.pending_queue_cursor_reanchor = Some(self.playing_queue_link());
    }

    /// Move the queue cursor to the playing slot at `current_idx`: the user
    /// jumped to the now-playing row, so the cursor follows it.
    pub fn move_queue_cursor_to_playing_slot(&mut self, current_idx: usize) {
        self.playback_queue_mut().set_cursor(current_idx);
    }

    /// Mark Home's Continue Watching snapshot as loading: the launch load is in
    /// flight, so the first frame paints the loading presentation.
    pub fn begin_home_load(&mut self) {
        self.home_content.loading = true;
    }

    /// Settle Home's Continue Watching snapshot: the launch load answered, so no
    /// frame paints it as loading.
    pub fn finish_home_load(&mut self) {
        self.home_content.loading = false;
    }

    /// Drop the in-flight marker of the artist detail for `key` and hold `entry`
    /// in its place: the key then names a settled result.
    pub fn settle_artist_detail(
        &mut self,
        key: mbv_ui_model::music_artist_detail::ArtistDetailKey,
        entry: mbv_ui_model::music_artist_detail::ArtistDetailCacheEntry,
    ) {
        self.artist_detail_loading.remove(&key);
        self.artist_detail_cache.insert(key, entry);
    }

    /// Drop the in-flight marker of the album tracks fetched for `album_id`
    /// without caching them: the fetch ended but holds no tracks, so the next
    /// push may ask for them again.
    pub fn abandon_album_tracks_fetch(&mut self, album_id: &str) {
        self.album_tracks_loading.remove(album_id);
    }
}
