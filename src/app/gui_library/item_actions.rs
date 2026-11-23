//! The GUI Library panel's item actions: the detail, album, shuffle, play,
//! and enqueue entries the commands call, and the item lookups they share.
//!
//! Each step reads and writes the GUI Client's own state, so it takes the
//! `App` and that state explicitly and the GUI core's `impl GuiCore` block
//! hands it both (design D4): the item lookups the projections also read stay
//! free functions, so no caller needs a GUI core for a plain read.

use mbv_emby_model::EmbyItem;
use mbv_queue::FeedEntry;
use mbv_ui_model::context_menu::AudiobookshelfMenuTarget;
use mbv_ui_model::ui_util::is_playable;

use super::GuiGroupAlbums;
use super::collection::GuiCollectionSet;
use super::podcasts_projection::GuiPodcastEpisodeId;
use super::request::GuiSectionRequest;
use crate::app::App;
use crate::app::gui_client_state::GuiClientState;
use crate::gui::GuiCore;

/// One GUI Library row's resolved menu target (add-gui-library-panel design
/// D2): the destination's own identity, so the shared builders pick the
/// standard action set for the row and a chosen entry runs against the row the
/// frame painted, never against a cursor.
pub(in crate::app::gui_library) enum GuiLibraryMenuTarget {
    /// An Emby Library destination row (Music, Shows, or a plain Emby
    /// library), with the library its item resolves in.
    Library { lib_idx: usize, item: EmbyItem },
    /// A Home Emby row: its item, and whether it is a Continue Watching row,
    /// whose menu adds the Continue Watching removal.
    Home {
        item: EmbyItem,
        continue_watching: bool,
    },
    /// An Audiobookshelf row: a Podcasts episode or an Audiobooks book.
    Audiobookshelf(AudiobookshelfMenuTarget),
    /// A Feeds row.
    Feed(FeedEntry),
}

impl GuiCore {
    /// Show `item_id` in the Hero (design D2, D13; gui-collection-dropdown
    /// design D11): the album the window content picked becomes the
    /// selection's pick — one checked group's kept pick, which the launch
    /// file saves, or the session's union pick while All or several groups
    /// are checked. An id the current Service data does not hold leaves the
    /// Hero's item as it was, because the frame that carried the id is stale.
    pub(super) fn show_gui_library_detail(&mut self, item_id: &str) {
        show_library_detail(&mut self.app, &mut self.gui, item_id);
    }

    /// Open an album node (design D5, D7): the shared album-track fetch fills
    /// the cache the tracklist reads, and it is a no-op once the cache holds
    /// the album.
    pub(super) fn open_gui_library_album(&mut self, album_id: String) {
        self.app.fetch_album_tracks(album_id);
    }

    /// Shuffle the artist whose albums are `album_ids` (design D8): the
    /// albums the window content's union holds, played through the shared
    /// album expansion with shuffle on, replacing the queue directly — the
    /// explicit Library shuffle behaves like the Library play, whose
    /// replace-confirm is skipped (the saved-playlist prompt still gates the
    /// replacement). Ids the store and the navigation state no longer hold
    /// are skipped.
    pub(super) fn shuffle_gui_artist(&mut self, album_ids: &[String]) {
        shuffle_artist(&mut self.app, &self.gui, album_ids);
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
        play_library_item(&mut self.app, &self.gui, lib_idx, item);
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
        add_library_item(&mut self.app, &self.gui, item_id);
    }
}

/// Show `item_id` in the Hero (design D2, D13; gui-collection-dropdown design
/// D11): one checked group's kept pick, or the session's union pick while All
/// or several groups are checked.
fn show_library_detail(app: &mut App, gui: &mut GuiClientState, item_id: &str) {
    let Some((lib_idx, item)) = library_item(app, gui, item_id) else {
        super::log_stale_library_item("showDetail", item_id);
        return;
    };
    let single_group = match super::projection::effective_music_set(app, gui, lib_idx).as_ref() {
        GuiCollectionSet::Checked(keys) if keys.len() == 1 => Some(keys[0].clone()),
        _ => None,
    };
    if let Some(group_id) = single_group {
        let key = (lib_idx, group_id);
        if gui.library_picks.get(&key) == Some(&item.id) {
            return;
        }
        gui.library_picks.insert(key, item.id);
    } else {
        if gui.music_union_picks.get(&lib_idx) == Some(&item.id) {
            return;
        }
        gui.music_union_picks.insert(lib_idx, item.id);
    }
    // A pick move is library navigation: it closes the image warm gate, so
    // fast arrow traffic warms no neighbour art (the TUI's own rule).
    app.stamp_navigation();
}

/// Shuffle the artist whose albums are `album_ids` (design D8).
fn shuffle_artist(app: &mut App, gui: &GuiClientState, album_ids: &[String]) {
    let albums: Vec<EmbyItem> = album_ids
        .iter()
        .filter_map(|album_id| library_item(app, gui, album_id))
        .map(|(_lib_idx, album)| album)
        .collect();
    if albums.is_empty() {
        super::log_ignored_library_command(
            "shuffleArtist",
            format_args!("the album ids name no album the Service data holds"),
        );
        return;
    }
    app.play_music_albums(albums, true, false);
}

/// Play `item` as the GUI Library's explicit play (design D8): the row menu's
/// bare Play entry reaches it through the row's own destination path.
pub(in crate::app::gui_library) fn play_library_item(
    app: &mut App,
    _gui: &GuiClientState,
    lib_idx: usize,
    item: EmbyItem,
) {
    if item.is_folder {
        let ct = app.libs[lib_idx].library.collection_type.clone();
        app.play_folder_gated(&item.id, ct, false);
        return;
    }
    if !is_playable(&item) {
        return;
    }
    let autoload = app.config.lock().unwrap().autoload;
    if autoload
        && let Some(album_id) = track_album_id(app, &item.id)
        && let Some((tracks, start_idx)) = app.resolve_playable_album_tracks(&album_id, &item.id)
    {
        app.play_items_routed(tracks, start_idx, mbv_queue::QueueSource::Album);
        return;
    }
    app.select_item(lib_idx, item);
}

/// Add the item to the queue with the Library's autoload rule (design D8).
pub(in crate::app::gui_library) fn add_library_item(
    app: &mut App,
    gui: &GuiClientState,
    item_id: &str,
) {
    let Some((lib_idx, item)) = library_item(app, gui, item_id) else {
        super::log_stale_library_item("addToQueue", item_id);
        return;
    };
    let autoload = app.config.lock().unwrap().autoload;
    if item.is_folder || !is_playable(&item) || !autoload {
        app.enqueue_lib_item(lib_idx, item);
        return;
    }
    let Some(album_id) = track_album_id(app, &item.id) else {
        app.enqueue_lib_item(lib_idx, item);
        return;
    };
    let Some((tracks, start_idx)) = app.resolve_playable_album_tracks(&album_id, &item.id) else {
        app.enqueue_lib_item(lib_idx, item);
        return;
    };
    // The autoload block enters as one append in disc/track order, with
    // the admission checks and tail append the shared plural path runs.
    let tracks = tracks.into_iter().skip(start_idx).collect();
    app.enqueue_lib_items(lib_idx, tracks);
}

/// The album whose cached tracklist holds `track_id` (design D5): the
/// tracklist a track row and the Hero's tracklist paint, keyed by the
/// album the open-album command fetched it for. `None` when no cached
/// tracklist holds the id — the add falls back to the single item.
pub(in crate::app::gui_library) fn track_album_id(app: &App, track_id: &str) -> Option<String> {
    app.album_tracks_cache
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
/// then the navigation levels the TUI also loads, then the Shows New
/// fetch's landed rows, then the picked album's cached tracks (design D5,
/// row 8.1, D9).
pub(in crate::app::gui_library) fn library_item(
    app: &App,
    gui: &GuiClientState,
    item_id: &str,
) -> Option<(usize, EmbyItem)> {
    for (lib_idx, lib) in app.libs.iter().enumerate() {
        let stored = stored_albums(gui, lib_idx).find(|album| album.id == item_id);
        let emby_root =
            || super::emby::stored_emby_root_items(gui, lib_idx).find(|item| item.id == item_id);
        let browsed = || {
            lib.nav_stack
                .iter()
                .rev()
                .find_map(|level| level.items.iter().find(|item| item.id == item_id))
        };
        // A Shows New row names one of the library's latest episodes, and
        // only its own fetch's landed rows hold that list (design D9).
        let shows_new = || {
            gui.shows_new
                .get(&lib.library.id)
                .into_iter()
                .flat_map(GuiSectionRequest::items)
                .find(|item| item.id == item_id)
        };
        if let Some(item) = stored
            .or_else(emby_root)
            .or_else(browsed)
            .or_else(shows_new)
        {
            return Some((lib_idx, item.clone()));
        }
        // A series tree row names a season or an episode, which only the
        // cached series details hold (design D14).
        if let Some(item) = shows_tree_item(app, item_id) {
            return Some((lib_idx, item));
        }
    }
    // A tracklist row names a track of the picked album, and only an
    // Emby library tab holds one.
    let track_lib = app.context_menu_lib_idx()?;
    app.album_tracks_cache
        .values()
        .flatten()
        .find(|track| track.id == item_id)
        .map(|track| (track_lib, track.clone()))
}

/// The Audiobookshelf menu target a Library row's id names, or `None`
/// when no Audiobookshelf store holds it: a book the published catalog
/// carries, or a podcast episode whose `(show id, episode id)` identity
/// decodes and resolves across the podcast browse states.
pub(in crate::app::gui_library) fn audiobookshelf_target(
    app: &App,
    item_id: &str,
) -> Option<AudiobookshelfMenuTarget> {
    let book = app.audiobookshelf_book_browse.iter().any(|state| {
        state
            .books
            .iter()
            .any(|book| book.library_item_id == item_id)
    });
    if book {
        return Some(AudiobookshelfMenuTarget::Book {
            library_item_id: item_id.to_string(),
        });
    }
    let episode = GuiPodcastEpisodeId::decode(item_id)?;
    let known = app.audiobookshelf_browse.iter().any(|state| {
        state
            .episode_by_identity(episode.library_item_id(), episode.episode_id())
            .is_some()
    });
    known.then(|| AudiobookshelfMenuTarget::Episode {
        library_item_id: episode.library_item_id().to_string(),
        episode_id: episode.episode_id().to_string(),
    })
}

/// The season or episode a Shows tree row names (design D14): the cached
/// series details hold both.
pub(in crate::app::gui_library) fn shows_tree_item(app: &App, item_id: &str) -> Option<EmbyItem> {
    app.series_detail_cache.values().find_map(|detail| {
        detail
            .seasons
            .iter()
            .chain(detail.episodes.values().flatten())
            .find(|item| item.id == item_id)
            .cloned()
    })
}

/// The albums the GUI store loaded for `lib_idx`'s selected groups.
pub(in crate::app::gui_library) fn stored_albums(
    gui: &GuiClientState,
    lib_idx: usize,
) -> impl Iterator<Item = &EmbyItem> {
    gui.music_albums
        .get(&lib_idx)
        .into_iter()
        .flat_map(|groups| groups.values())
        .filter_map(|state| match state {
            GuiGroupAlbums::Loaded(albums) => Some(albums.iter()),
            GuiGroupAlbums::Loading | GuiGroupAlbums::Failed => None,
        })
        .flatten()
}
