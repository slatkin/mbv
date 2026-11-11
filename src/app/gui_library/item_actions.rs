//! The GUI Library panel's item actions: the detail, album, shuffle, play,
//! and enqueue entries the commands call, and the item lookups they share.

use mbv_emby_model::EmbyItem;
use mbv_ui_model::ui_util::is_playable;

use super::GuiGroupAlbums;
use crate::app::App;

impl App {
    /// Show `item_id` in the Hero (design D2, D13): the album the window content
    /// picked becomes the selected group's pick, so the pill reopens on it. An
    /// id the current Service data does not hold leaves the Hero's item as it
    /// was, because the frame that carried the id is stale.
    pub(super) fn show_gui_library_detail(&mut self, item_id: &str) {
        let Some((lib_idx, item)) = self.gui_library_item(item_id) else {
            Self::log_stale_library_item("showDetail", item_id);
            return;
        };
        let Some(group_id) = self.gui_selected_group(lib_idx) else {
            return;
        };
        let key = (lib_idx, group_id);
        if self.gui_library_picks.get(&key) == Some(&item.id) {
            return;
        }
        self.gui_library_picks.insert(key, item.id);
        // A pick move is library navigation: it closes the image warm gate, so
        // fast arrow traffic warms no neighbour art (the TUI's own rule).
        self.stamp_navigation();
    }

    /// Open an album node (design D5, D7): the shared album-track fetch fills
    /// the cache the tracklist reads, and it is a no-op once the cache holds
    /// the album.
    pub(super) fn open_gui_library_album(&mut self, album_id: String) {
        self.fetch_album_tracks(album_id);
    }

    /// Shuffle the artist whose albums are `album_ids` (design D8): the
    /// albums the window content's union holds, played through the shared
    /// album expansion with shuffle on, replacing the queue directly — the
    /// explicit Library shuffle behaves like the Library play, whose
    /// replace-confirm is skipped (the saved-playlist prompt still gates the
    /// replacement). Ids the store and the navigation state no longer hold
    /// are skipped.
    pub(super) fn shuffle_gui_artist(&mut self, album_ids: &[String]) {
        let albums: Vec<EmbyItem> = album_ids
            .iter()
            .filter_map(|album_id| self.gui_library_item(album_id))
            .map(|(_lib_idx, album)| album)
            .collect();
        if albums.is_empty() {
            Self::log_ignored_library_command(
                "shuffleArtist",
                format_args!("the album ids name no album the Service data holds"),
            );
            return;
        }
        self.play_music_albums(albums, true, false);
    }

    /// Play the item (design D8), as the GUI Library's explicit play: the
    /// populated-queue replace-confirm is skipped — it is a TUI overlay the
    /// GUI Client never paints, so a play gated behind it was swallowed and
    /// the Hero's Play button did nothing over a populated queue. An album
    /// (folder) replaces the queue with its whole playable content through
    /// the shared folder path; a track replaces the queue with the album's
    /// cached playable tracks in disc/track order starting at the selected
    /// track when autoload is on (the library playback policy's autoload
    /// rule, through the same cache the add path reads), and otherwise takes
    /// the shared select-item path.
    pub(super) fn play_gui_library_item(&mut self, lib_idx: usize, item: EmbyItem) {
        if item.is_folder {
            let ct = self.libs[lib_idx].library.collection_type.clone();
            self.play_folder_gated(&item.id, ct, false);
            return;
        }
        if !is_playable(&item) {
            return;
        }
        let autoload = self.config.lock().unwrap().autoload;
        if autoload
            && let Some(album_id) = self.gui_track_album_id(&item.id)
            && let Some((tracks, start_idx)) =
                self.resolve_playable_album_tracks(&album_id, &item.id)
        {
            self.play_items_routed(tracks, start_idx, mbv_queue::QueueSource::Album);
            return;
        }
        self.select_item(lib_idx, item);
    }

    /// Add the item to the queue (design D8), with the library playback
    /// policy's autoload rule applied to this one route: a track whose album
    /// the track cache holds queues that album's cached playable tracks in
    /// disc/track order starting at the selected track when autoload is on,
    /// and only the selected track when it is off. Every other item — an
    /// album, a folder, an item the cache does not hold — takes the shared
    /// add path unchanged. The TUI's own add paths keep their per-item
    /// appends; only the GUI Library's add applies the rule.
    pub(super) fn add_gui_library_item_to_queue(&mut self, item_id: &str) {
        let Some((lib_idx, item)) = self.gui_library_item(item_id) else {
            Self::log_stale_library_item("addToQueue", item_id);
            return;
        };
        let autoload = self.config.lock().unwrap().autoload;
        if item.is_folder || !is_playable(&item) || !autoload {
            self.enqueue_lib_item(lib_idx, item);
            return;
        }
        let Some(album_id) = self.gui_track_album_id(&item.id) else {
            self.enqueue_lib_item(lib_idx, item);
            return;
        };
        let Some((tracks, start_idx)) = self.resolve_playable_album_tracks(&album_id, &item.id)
        else {
            self.enqueue_lib_item(lib_idx, item);
            return;
        };
        // The autoload block enters as one append in disc/track order, with
        // the admission checks and tail append the shared plural path runs.
        let tracks = tracks.into_iter().skip(start_idx).collect();
        self.enqueue_lib_items(lib_idx, tracks);
    }

    /// The album whose cached tracklist holds `track_id` (design D5): the
    /// tracklist a track row and the Hero's tracklist paint, keyed by the
    /// album the open-album command fetched it for. `None` when no cached
    /// tracklist holds the id — the add falls back to the single item.
    pub(super) fn gui_track_album_id(&self, track_id: &str) -> Option<String> {
        self.album_tracks_cache
            .iter()
            .find_map(|(album_id, tracks)| {
                tracks
                    .iter()
                    .any(|track| track.id == track_id)
                    .then(|| album_id.clone())
            })
    }

    /// The item a command names, with the library it belongs to: the album
    /// lists the GUI store holds, then the plain Emby libraries' root lists,
    /// then the navigation levels the TUI also loads, then the picked album's
    /// cached tracks (design D5, row 8.1).
    pub(super) fn gui_library_item(&self, item_id: &str) -> Option<(usize, EmbyItem)> {
        for (lib_idx, lib) in self.libs.iter().enumerate() {
            let stored = self
                .gui_stored_albums(lib_idx)
                .find(|album| album.id == item_id);
            let emby_root = || {
                self.gui_stored_emby_root_items(lib_idx)
                    .find(|item| item.id == item_id)
            };
            let browsed = || {
                lib.nav_stack
                    .iter()
                    .rev()
                    .find_map(|level| level.items.iter().find(|item| item.id == item_id))
            };
            if let Some(item) = stored.or_else(emby_root).or_else(browsed) {
                return Some((lib_idx, item.clone()));
            }
            // A series tree row names a season or an episode, which only the
            // cached series details hold (design D14).
            if let Some(item) = self.gui_shows_tree_item(item_id) {
                return Some((lib_idx, item));
            }
        }
        // A tracklist row names a track of the picked album, and only an
        // Emby library tab holds one.
        let track_lib = self.context_menu_lib_idx()?;
        self.album_tracks_cache
            .values()
            .flatten()
            .find(|track| track.id == item_id)
            .map(|track| (track_lib, track.clone()))
    }

    /// The season or episode a Shows tree row names (design D14): the cached
    /// series details hold both.
    pub(super) fn gui_shows_tree_item(&self, item_id: &str) -> Option<EmbyItem> {
        self.series_detail_cache.values().find_map(|detail| {
            detail
                .seasons
                .iter()
                .chain(detail.episodes.values().flatten())
                .find(|item| item.id == item_id)
                .cloned()
        })
    }

    /// The albums the GUI store loaded for `lib_idx`'s selected groups.
    pub(super) fn gui_stored_albums(&self, lib_idx: usize) -> impl Iterator<Item = &EmbyItem> {
        self.gui_music_albums
            .iter()
            .filter(move |((store_lib_idx, _group_id), _state)| *store_lib_idx == lib_idx)
            .filter_map(|(_key, state)| match state {
                GuiGroupAlbums::Loaded(albums) => Some(albums.iter()),

                GuiGroupAlbums::Loading | GuiGroupAlbums::Failed => None,
            })
            .flatten()
    }
}
