//! The GUI Library panel's Client-core half (add-gui-library-panel design D2,
//! D5): the semantic commands the window content sends, the public `App` entry
//! that applies each one, the Music collection store beside it, and the Music
//! projection the window content paints. The window content sends intent, and
//! every entry calls the existing `App` action for it. The TUI Client never
//! calls any of them.

mod artwork;
mod projection;
mod store;

pub use self::artwork::GuiLibraryArtwork;
pub use self::projection::{
    GuiMusicAlbum, GuiMusicArtist, GuiMusicHero, GuiMusicPill, GuiMusicProjection, GuiMusicTrack,
    GuiMusicTrackTotal,
};
pub(in crate::app) use self::store::{GuiGroupAlbums, GuiGroupAlbumsEvent};

use mbv_emby_model::EmbyItem;
use mbv_ui_model::settings::PanelFocus;

use crate::app::App;

/// One request from the GUI Library panel's window content (design D2): the
/// `library` command carries it into the Client core, and
/// [`App::apply_gui_library_command`] applies it. Item identities are stable
/// Service ids, never row positions, so a command built from a stale frame
/// addresses the item that frame painted or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiLibraryCommand {
    /// Open a destination: `position` is the tab position the frame's
    /// destination tabs carry, where 0 is Home.
    OpenDestination { position: usize },
    /// Replace the selected collections of the Grouped Music library
    /// `lib_idx` with `group_ids`, in pill order (design D5, D6).
    SetCollections {
        lib_idx: usize,
        group_ids: Vec<String>,
    },
    /// Show the item's detail in the Hero (design D2).
    ShowDetail { item_id: String },
    /// Open an album node: load its tracks when the cache does not hold them
    /// (design D5).
    OpenAlbum { album_id: String },
    /// Take Panel focus for the Library panel (design D4).
    TakeFocus,
    /// Play the item (design D8).
    Play { item_id: String },
    /// Play the item next (design D8): the shared play-next admission, so the
    /// item enters directly after the active slot.
    PlayNext { item_id: String },
    /// Add the item to the queue (design D8).
    AddToQueue { item_id: String },
    /// Shuffle the artist whose albums are `album_ids` (design D8).
    ShuffleArtist { album_ids: Vec<String> },
    /// Run the item menu's entry at `index` (design D8). The menu's builder
    /// and executor widen with row 3.6, which replaces this documented no-op.
    MenuEntry { item_id: String, index: usize },
}

impl App {
    /// Apply one GUI Library command (design D2): each arm calls the existing
    /// `App` action for the intent, so the TUI's own paths do not change.
    pub fn apply_gui_library_command(&mut self, command: GuiLibraryCommand) {
        match command {
            GuiLibraryCommand::OpenDestination { position } => self.set_library_tab(position),
            GuiLibraryCommand::SetCollections { lib_idx, group_ids } => {
                self.set_gui_library_collections(lib_idx, group_ids);
            }
            GuiLibraryCommand::ShowDetail { item_id } => self.show_gui_library_detail(&item_id),
            GuiLibraryCommand::OpenAlbum { album_id } => self.open_gui_library_album(album_id),
            GuiLibraryCommand::TakeFocus => self.set_gui_panel_focus(PanelFocus::Library),
            GuiLibraryCommand::Play { item_id } => {
                if let Some((lib_idx, item)) = self.gui_library_item(&item_id) {
                    self.play_or_activate_lib_item(lib_idx, item);
                }
            }
            GuiLibraryCommand::PlayNext { item_id } => {
                if let Some((lib_idx, item)) = self.gui_library_item(&item_id) {
                    self.play_lib_item_next(lib_idx, item);
                }
            }
            GuiLibraryCommand::AddToQueue { item_id } => {
                if let Some((lib_idx, item)) = self.gui_library_item(&item_id) {
                    self.enqueue_lib_item(lib_idx, item);
                }
            }
            GuiLibraryCommand::ShuffleArtist { album_ids } => {
                self.shuffle_gui_artist(&album_ids);
            }
            // Row 3.6 widens the menu builder and executor; until then the
            // intent is accepted and runs nothing, so no value of `index` can
            // reach the TUI's context-menu dispatch.
            GuiLibraryCommand::MenuEntry { item_id, index } => drop((item_id, index)),
        }
    }

    /// Set Panel focus for the GUI Library panel (design D4): the take-focus
    /// effect, sent by a press inside the panel and by a Tab across the panel
    /// boundary. The core thread sets its `FocusStack` with the same value, so
    /// the frame's focus and the App's key gates cannot disagree.
    pub fn set_gui_panel_focus(&mut self, focus: PanelFocus) {
        self.set_panel_focus(focus);
    }

    /// Show `item_id` in the Hero (design D2): the album the window content
    /// picked. An id the current Service data does not hold leaves the Hero's
    /// item as it was, because the frame that carried the id is stale.
    fn show_gui_library_detail(&mut self, item_id: &str) {
        if let Some((_lib_idx, item)) = self.gui_library_item(item_id) {
            self.gui_library_detail = Some(item);
        }
    }

    /// Open an album node (design D5, D7): the shared album-track fetch fills
    /// the cache the tracklist reads, and it is a no-op once the cache holds
    /// the album.
    fn open_gui_library_album(&mut self, album_id: String) {
        self.fetch_album_tracks(album_id);
    }

    /// Shuffle the artist whose albums are `album_ids` (design D8): the
    /// albums the window content's union holds, played through the shared
    /// album expansion with shuffle on. Ids the store and the navigation
    /// state no longer hold are skipped.
    fn shuffle_gui_artist(&mut self, album_ids: &[String]) {
        let albums: Vec<EmbyItem> = album_ids
            .iter()
            .filter_map(|album_id| self.gui_library_item(album_id))
            .map(|(_lib_idx, album)| album)
            .collect();
        if albums.is_empty() {
            return;
        }
        self.play_music_albums(albums, true);
    }

    /// The item a command names, with the library it belongs to: the album
    /// lists the GUI store holds, then the navigation levels the TUI also
    /// loads, then the picked album's cached tracks (design D5).
    fn gui_library_item(&self, item_id: &str) -> Option<(usize, EmbyItem)> {
        for (lib_idx, lib) in self.libs.iter().enumerate() {
            let stored = self
                .gui_stored_albums(lib_idx)
                .find(|album| album.id == item_id);
            let browsed = || {
                lib.nav_stack
                    .iter()
                    .rev()
                    .find_map(|level| level.items.iter().find(|item| item.id == item_id))
            };
            if let Some(item) = stored.or_else(browsed) {
                return Some((lib_idx, item.clone()));
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

    /// The albums the GUI store loaded for `lib_idx`'s selected groups.
    fn gui_stored_albums(&self, lib_idx: usize) -> impl Iterator<Item = &EmbyItem> {
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
