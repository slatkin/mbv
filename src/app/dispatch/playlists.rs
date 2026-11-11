//! Playlist actions shared by the TUI shell and the GUI Settings screen
//! (add-gui-settings-screen design D7): keyed by playlist id and item id,
//! never by a caller's row index. The TUI shell resolves its row index to
//! the id before it calls.

use super::super::App;
use super::super::state::playback::{PendingQueueAction, ReplacementExecutor};
use mbv_emby_model::EmbyItem;
use mbv_queue::{QueueItem, QueueSource};
use mbv_ui_msg::MusicTreeAction;
use rand::seq::SliceRandom;

impl App {
    /// Enter/`s`/`a` on a saved playlist in the list view (design D2): Play and
    /// Shuffle replace the queue with its playable items; Enqueue appends them.
    pub(in crate::app) fn playlist_action(&mut self, id: &str, action: MusicTreeAction) {
        let Some(playlist) = self.playlists.iter().find(|p| p.id == id) else {
            return;
        };
        let (id, name) = (playlist.id.clone(), playlist.name.clone());
        let Some(items) = self.playlist_playable_items(&id) else {
            return;
        };
        match action {
            MusicTreeAction::Play => {
                let source = QueueSource::Playlist { id: Some(id), name };
                self.replace_queue_from_playlist(items, 0, source);
            }
            MusicTreeAction::Shuffle => self.shuffle_queue_from_playlist(items),
            MusicTreeAction::Enqueue => self.enqueue_playlist_items(items),
        }
    }

    /// Enter/`s`/`a` on an item row of the open playlist (design D2): Play
    /// starts the playlist at that row, Shuffle replaces the queue with the
    /// whole playlist shuffled, and Enqueue appends just that row.
    pub(in crate::app) fn playlist_row_action(&mut self, item_id: &str, action: MusicTreeAction) {
        let Some(row) = self
            .playlists_open_items
            .iter()
            .find(|item| item.id == item_id)
            .cloned()
        else {
            return;
        };
        let Some(playlist) = self.playlists_open.clone() else {
            return;
        };
        let items: Vec<_> = self
            .playlists_open_items
            .iter()
            .filter(|item| !item.is_folder)
            .cloned()
            .collect();
        match action {
            MusicTreeAction::Play => {
                let start_idx = items.iter().position(|item| item.id == row.id).unwrap_or(0);
                let source = QueueSource::Playlist {
                    id: Some(playlist.id),
                    name: playlist.name,
                };
                self.replace_queue_from_playlist(items, start_idx, source);
            }
            MusicTreeAction::Shuffle => self.shuffle_queue_from_playlist(items),
            MusicTreeAction::Enqueue if !row.is_folder => self.enqueue_playlist_items(vec![row]),
            MusicTreeAction::Enqueue => {}
        }
    }

    fn shuffle_queue_from_playlist(&mut self, mut items: Vec<EmbyItem>) {
        items.shuffle(&mut rand::rng());
        self.replace_queue_from_playlist(items, 0, QueueSource::Shuffle);
    }

    /// Replaces the queue (gated by the populated-queue confirmation), then
    /// closes the Playlists sidebar so the new queue is visible.
    fn replace_queue_from_playlist(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: QueueSource,
    ) {
        if items.is_empty() {
            return;
        }
        self.request_queue_replacement(
            PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
            },
            ReplacementExecutor::PlaylistsSidebar,
        );
    }

    /// Appends to the viewed queue once and leaves the sidebar open.
    fn enqueue_playlist_items(&mut self, items: Vec<EmbyItem>) {
        let items = items
            .into_iter()
            .map(|item| QueueItem::Emby(Box::new(item)))
            .collect();
        let link = self.playing_queue_link();
        self.append_on_owner(link, items, None);
    }
}
