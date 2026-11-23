//! The GUI Client's own state, as one value (design D4 of
//! `decouple-client-core`).
//!
//! Every GUI-only field the `App` used to carry lives here: the library
//! stores the GUI's projections read, the open overlay, the Settings and
//! Playlists screens, and the panel toggles. Only GUI code reads or writes
//! it. Until the GUI fields leave the `App` for `GuiCore` (row 8.1), the
//! `App` holds this value as its one transitional `gui` field.
//!
//! The core never names a field of this struct. The struct is the GUI
//! Client's state, not the core's.

use crate::app::GuiLibraryMenuRow;
use crate::app::gui_client_state::GuiOverlay;
use crate::app::gui_library::{
    GuiAudiobooksSelection, GuiCollectionSelection, GuiEmbyLibrarySelection, GuiEmbyRootItems,
    GuiFeedsSelection, GuiGroupAlbums, GuiHomeRowId, GuiListSort, GuiPodcastsSelection,
    GuiSearchStore, GuiSectionRequest, GuiShowsView, GuiTvShowsSelection,
};
use crate::app::gui_settings::GuiSettingsState;
use mbv_config::TabIdentity;

/// The canvas panel's two toggles: `z` collapses the panel to its rail, and
/// `q` folds the expanded panel's queue to its one-line summary. The Client
/// core holds them, so a window-content reload keeps them.
#[derive(Default)]
pub(in crate::app) struct PanelToggles {
    /// The `z` toggle: the panel collapsed to the rail.
    pub(in crate::app) collapsed: bool,
    /// The `q` toggle: the expanded panel's queue folded to its summary.
    pub(in crate::app) queue_folded: bool,
}

/// The GUI Client's Client-core presentation state: the library stores, the
/// open overlay, the Settings and Playlists screens, and the panel toggles.
/// The TUI Client never reads it.
pub(in crate::app) struct GuiClientState {
    /// Whether the GUI player's controls auto-hide while an item plays and
    /// the pointer is not over the artwork. Persisted as
    /// `gui_player_auto_hide`.
    pub(in crate::app) player_auto_hide: bool,
    /// The library tab's collection selection, by library index.
    pub(in crate::app) library_groups: std::collections::HashMap<usize, GuiCollectionSelection>,
    /// The music library's album-list state, by library index, then by group
    /// key.
    pub(in crate::app) music_albums:
        std::collections::HashMap<usize, std::collections::HashMap<String, GuiGroupAlbums>>,
    /// The chosen collection-group key, by library index and group key.
    pub(in crate::app) library_picks: std::collections::HashMap<(usize, String), String>,
    /// The chosen Music union-group key, by library index.
    pub(in crate::app) music_union_picks: std::collections::HashMap<usize, String>,
    /// The row a library row menu was opened on.
    pub(in crate::app) library_row_menu_target: Option<GuiLibraryMenuRow>,
    /// The artwork rows the GUI last had in view, by row-art scope.
    pub(in crate::app) rows_in_view:
        std::collections::HashMap<crate::app::gui_library::GuiRowArtScope, Vec<String>>,
    /// The Home surface's selected row.
    pub(in crate::app) home_selection: Option<GuiHomeRowId>,
    /// The Shows surface's flat-list selection.
    pub(in crate::app) shows_flat_selection: Option<String>,
    /// The Shows surface's view, by library index.
    pub(in crate::app) shows_views: std::collections::HashMap<usize, GuiShowsView>,
    /// Shows section fetches, by library index and section id.
    pub(in crate::app) shows_new:
        std::collections::HashMap<String, GuiSectionRequest<mbv_emby_model::EmbyItem>>,
    /// The TV surface's selection, by library index.
    pub(in crate::app) tv_shows: std::collections::HashMap<usize, GuiTvShowsSelection>,
    /// The Emby library selection, by library index. Distinct from
    /// `emby_feed_sets`.
    pub(in crate::app) emby_libs: std::collections::HashMap<usize, GuiEmbyLibrarySelection>,
    /// The Emby collection selection, by library index.
    pub(in crate::app) emby_feed_sets: std::collections::HashMap<usize, GuiCollectionSelection>,
    /// The Emby library's root items, by library index.
    pub(in crate::app) emby_roots: std::collections::HashMap<usize, GuiEmbyRootItems>,
    /// The Audiobookshelf library selection, by library index.
    pub(in crate::app) abs_libs: std::collections::HashMap<usize, GuiAudiobooksSelection>,
    /// The podcast library selection, by library index.
    pub(in crate::app) podcast_libs: std::collections::HashMap<usize, GuiPodcastsSelection>,
    /// The Feeds surface's selection.
    pub(in crate::app) feeds_selection: GuiFeedsSelection,
    /// The chosen list sort per destination, keyed by the destination's
    /// stable identity and held in the order the destinations were first
    /// sorted.
    pub(in crate::app) list_sorts: Vec<(TabIdentity, GuiListSort)>,
    /// The search store the GUI fills through `App::drain_gui_search_results`.
    pub(in crate::app) search: GuiSearchStore,
    /// The canvas panel's `z` and `q` toggles.
    pub(in crate::app) panel_toggles: PanelToggles,
    /// The GUI Client's open overlay: the queue row's menu, a track chip's
    /// popover, the playback-target chooser, or the Queue chooser. `None`
    /// while none is open.
    pub(in crate::app) overlay: Option<GuiOverlay>,
    /// The GUI Settings screen's open state: `None` while closed.
    pub(in crate::app) settings_screen: Option<GuiSettingsState>,
    /// Whether the GUI's Playlists screen is open.
    pub(in crate::app) playlists_screen_open: bool,
    /// The UI-reset counter the GUI frame carries: the front end resets its
    /// interaction stores when the value changes.
    pub(in crate::app) ui_reset_epoch: u64,
}

impl GuiClientState {
    /// The state a fresh `App` starts with, with the auto-hide preference it
    /// reads from the Client prefs.
    pub(in crate::app) fn new(player_auto_hide: bool) -> Self {
        Self {
            player_auto_hide,
            library_groups: std::collections::HashMap::new(),
            music_albums: std::collections::HashMap::new(),
            library_picks: std::collections::HashMap::new(),
            music_union_picks: std::collections::HashMap::new(),
            library_row_menu_target: None,
            rows_in_view: std::collections::HashMap::new(),
            home_selection: None,
            shows_flat_selection: None,
            shows_views: std::collections::HashMap::new(),
            shows_new: std::collections::HashMap::new(),
            tv_shows: std::collections::HashMap::new(),
            emby_libs: std::collections::HashMap::new(),
            emby_feed_sets: std::collections::HashMap::new(),
            emby_roots: std::collections::HashMap::new(),
            abs_libs: std::collections::HashMap::new(),
            podcast_libs: std::collections::HashMap::new(),
            feeds_selection: GuiFeedsSelection::default(),
            list_sorts: Vec::new(),
            search: GuiSearchStore::default(),
            panel_toggles: PanelToggles::default(),
            overlay: None,
            settings_screen: None,
            playlists_screen_open: false,
            ui_reset_epoch: 0,
        }
    }

    /// Whether an overlay holds the GUI Client's keyboard (the row menu, a
    /// track popover, the playback-target chooser, or the Queue chooser). The
    /// GUI Client's core reads it to dismiss the overlay on an outside press.
    #[must_use]
    pub fn overlay_is_open(&self) -> bool {
        self.overlay.is_some()
    }
}
