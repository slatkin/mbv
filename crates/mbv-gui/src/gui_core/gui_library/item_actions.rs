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
use mbv_ui_model::music_grouping::ArtistKey;
use mbv_ui_model::ui_util::{is_playable, sort_audio_tracks};

use super::GuiGroupAlbums;
use super::collection::GuiCollectionSet;
use super::podcasts_projection::GuiPodcastEpisodeId;
use super::request::GuiSectionRequest;
use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;

/// One GUI Library row's resolved menu target (add-gui-library-panel design
/// D2): the destination's own identity, so the shared builders pick the
/// standard action set for the row and a chosen entry runs against the row the
/// frame painted, never against a cursor.
pub(in crate::gui_core::gui_library) enum GuiLibraryMenuTarget {
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

    /// Show the artist's page in the Hero (gui-music-artist-page design D1,
    /// D2): the page pick replaces any earlier entry of the library, the
    /// artist page's album track loads start through the core's bounded
    /// intent, and navigation is stamped as an album pick stamps it, so the
    /// pick move warms no neighbour art. A key the union does not hold still
    /// stores: the union test in the projection is the one rule that drops
    /// the page, and its album list then loads nothing.
    pub(super) fn show_gui_artist_page(&mut self, lib_idx: usize, artist_key: ArtistKey) {
        self.gui.music_artist_pages.insert(lib_idx, artist_key);
        let album_ids = super::projection::artist_page_album_ids(&self.app, &self.gui, lib_idx);
        self.app.request_albums_tracks(album_ids);
        self.app.stamp_navigation();
    }

    /// Play the artist whose albums are `album_ids` (gui-music-artist-page
    /// design D2): the albums the window content's union holds, played
    /// through the shared album expansion in the given order with shuffle
    /// off — the same blocking walk Shuffle artist runs with shuffle on. A
    /// stale frame's album id is skipped and logged.
    pub(super) fn play_gui_artist(&mut self, album_ids: &[String]) {
        play_artist(&mut self.app, &self.gui, album_ids);
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
/// or several groups are checked. An album pick ends the library's artist
/// page (gui-music-artist-page design D1): the page entry leaves first, also
/// when the pick itself is unchanged, so a press on the album the page hides
/// still ends the page.
fn show_library_detail(app: &mut App, gui: &mut GuiClientState, item_id: &str) {
    let Some((lib_idx, item)) = library_item(app, gui, item_id) else {
        super::log_stale_library_item("showDetail", item_id);
        return;
    };
    gui.music_artist_pages.remove(&lib_idx);
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
    play_artist_albums(app, gui, album_ids, true, "shuffleArtist");
}

/// Play the artist whose albums are `album_ids` in the given order
/// (gui-music-artist-page design D2): the shuffle walk with shuffle off.
fn play_artist(app: &mut App, gui: &GuiClientState, album_ids: &[String]) {
    play_artist_albums(app, gui, album_ids, false, "playArtist");
}

/// The one album walk Shuffle artist and the artist page's Play share (design
/// D8; gui-music-artist-page design D2): the ids resolve in their given order,
/// an id the store no longer holds is skipped and logged as a stale frame's
/// id always is, and the albums play through `play_music_albums` with the
/// caller's shuffle.
fn play_artist_albums(
    app: &mut App,
    gui: &GuiClientState,
    album_ids: &[String],
    shuffle: bool,
    command: &'static str,
) {
    let mut albums: Vec<EmbyItem> = Vec::new();
    for album_id in album_ids {
        match library_item(app, gui, album_id) {
            Some((_lib_idx, album)) => albums.push(album),
            None => super::log_stale_library_item(command, album_id),
        }
    }
    if albums.is_empty() {
        super::log_ignored_library_command(
            command,
            format_args!("the album ids name no album the Service data holds"),
        );
        return;
    }
    app.play_music_albums(albums, shuffle, false);
}

/// Play the artist page's track `track_id` (gui-music-artist-page design
/// D4): the sequence is each album's cached playable tracks in `album_ids`
/// order, every album sorted in disc and track order, concatenated, and
/// playback starts at the track through the shared artist-tracks play path.
/// An album the cache does not hold yet adds nothing to the sequence — its
/// miss logs and the concatenation continues; only a `track_id` the
/// sequence does not hold logs and plays nothing.
pub(in crate::gui_core::gui_library) fn play_gui_artist_track(
    app: &mut App,
    album_ids: &[String],
    track_id: &str,
) {
    let mut tracks: Vec<EmbyItem> = Vec::new();
    for album_id in album_ids {
        let Some(cached) = app.album_tracks(album_id) else {
            super::log_ignored_library_command(
                "playArtistTrack",
                format_args!("the album {album_id} holds no cached tracklist"),
            );
            continue;
        };
        let mut album_tracks: Vec<EmbyItem> = cached
            .iter()
            .filter(|track| is_playable(track))
            .cloned()
            .collect();
        sort_audio_tracks(&mut album_tracks);
        tracks.extend(album_tracks);
    }
    let Some(start_idx) = tracks.iter().position(|track| track.id == track_id) else {
        super::log_ignored_library_command(
            "playArtistTrack",
            format_args!("the track {track_id} is not in the page's cached tracks"),
        );
        return;
    };
    app.play_artist_tracks(tracks, start_idx);
}

/// Play `item` as the GUI Library's explicit play (design D8): the row menu's
/// bare Play entry reaches it through the row's own destination path.
pub(in crate::gui_core::gui_library) fn play_library_item(
    app: &mut App,
    _gui: &GuiClientState,
    lib_idx: usize,
    item: EmbyItem,
) {
    if item.is_folder {
        let Some(ct) = app
            .emby_library_collection_type(lib_idx)
            .map(str::to_string)
        else {
            return;
        };
        app.play_folder_gated(&item.id, ct, false);
        return;
    }
    if !is_playable(&item) {
        return;
    }
    let autoload = app.autoload_enabled();
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
pub(in crate::gui_core::gui_library) fn add_library_item(
    app: &mut App,
    gui: &GuiClientState,
    item_id: &str,
) {
    let Some((lib_idx, item)) = library_item(app, gui, item_id) else {
        super::log_stale_library_item("addToQueue", item_id);
        return;
    };
    let autoload = app.autoload_enabled();
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
pub(in crate::gui_core::gui_library) fn track_album_id(
    app: &App,
    track_id: &str,
) -> Option<String> {
    app.album_of_cached_track(track_id)
}

/// The item a command names, with the library it belongs to: the album
/// lists the GUI store holds, then the plain Emby libraries' root lists,
/// then the navigation levels the TUI also loads, then the Shows New
/// fetch's landed rows, then the picked album's cached tracks (design D5,
/// row 8.1, D9).
pub(in crate::gui_core::gui_library) fn library_item(
    app: &App,
    gui: &GuiClientState,
    item_id: &str,
) -> Option<(usize, EmbyItem)> {
    for (lib_idx, lib) in app.emby_libraries().iter().enumerate() {
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
    app.cached_album_track(item_id)
        .map(|track| (track_lib, track.clone()))
}

/// The Audiobookshelf menu target a Library row's id names, or `None`
/// when no Audiobookshelf store holds it: a book the published catalog
/// carries, or a podcast episode whose `(show id, episode id)` identity
/// decodes and resolves across the podcast browse states.
pub(in crate::gui_core::gui_library) fn audiobookshelf_target(
    app: &App,
    item_id: &str,
) -> Option<AudiobookshelfMenuTarget> {
    let book = app.audiobookshelf_book_states().iter().any(|state| {
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
    let known = app.audiobookshelf_browse_states().iter().any(|state| {
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
pub(in crate::gui_core::gui_library) fn shows_tree_item(
    app: &App,
    item_id: &str,
) -> Option<EmbyItem> {
    app.series_detail_item(item_id)
}

/// The albums the GUI store loaded for `lib_idx`'s selected groups.
pub(in crate::gui_core::gui_library) fn stored_albums(
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
