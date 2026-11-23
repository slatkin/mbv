//! The GUI Client's own state, as one value (design D4 of
//! `decouple-client-core`).
//!
//! Every GUI-only field the `App` used to carry lives here: the library
//! stores the GUI's projections read, the open overlay, the Settings and
//! Playlists screens, and the panel toggles. Only GUI code reads or writes
//! it. `GuiCore` holds it beside the `App` it owns (design D4).
//!
//! The core never names a field of this struct. The struct is the GUI
//! Client's state, not the core's. It is its own `ClientLibraryStores`
//! implementor, so the core's library-event seam reaches the state through
//! that trait and nothing else.

use crate::app::GuiLibraryMenuRow;
use crate::app::dispatch::library::stores::{ClientLibraryEffect, ClientLibraryStores};
use crate::app::gui_client_state::GuiOverlay;
use crate::app::gui_library::emby_feed_wants_target_in_set;
use crate::app::gui_library::group_albums_mark;
use crate::app::gui_library::{
    GuiAudiobooksSelection, GuiCollectionSelection, GuiEmbyLibrarySelection, GuiEmbyRootItems,
    GuiFeedsSelection, GuiGroupAlbums, GuiHomeRowId, GuiListSort, GuiPodcastsSelection,
    GuiSearchStore, GuiSectionRequest, GuiShowsView, GuiTvShowsSelection,
};
use crate::app::gui_settings::GuiSettingsState;
use crate::app::{App, FeedHomeVideoSelection};
use mbv_config::TabIdentity;

/// The canvas panel's two toggles: `z` collapses the panel to its rail, and
/// `q` folds the expanded panel's queue to its one-line summary. The Client
/// core holds them, so a window-content reload keeps them.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::app) struct PanelToggles {
    /// The `z` toggle: the panel collapsed to the rail.
    pub(in crate::app) collapsed: bool,
    /// The `q` toggle: the expanded panel's queue folded to its summary.
    pub(in crate::app) queue_folded: bool,
}

/// The GUI Client's Client-core presentation state: the library stores, the
/// open overlay, the Settings and Playlists screens, and the panel toggles.
/// The TUI Client never reads it.
///
/// It is `Clone` so the core can mirror it beside an `App` a dependent
/// Client's test drives (row 8.1 of `decouple-client-core`): the tick host
/// adopts the sidecar's state at a step's start and publishes its own back at
/// the step's end, which needs a copy and not the one value.
#[derive(Clone)]
pub struct GuiClientState {
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

impl std::fmt::Debug for GuiClientState {
    /// The state names itself and whether an overlay holds the keyboard: its
    /// stores carry no fact a log line needs, and the label keeps the debug
    /// line as short as the `App`'s own.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiClientState")
            .field("overlay", &self.overlay.as_ref().map(|_| "open"))
            .field("panel_collapsed", &self.panel_toggles.collapsed)
            .field("queue_folded", &self.panel_toggles.queue_folded)
            .finish_non_exhaustive()
    }
}

impl Default for GuiClientState {
    /// The lands a caller that swaps one in for the length of a core call
    /// leaves empty: every map, every screen, and the toggles start as a
    /// fresh state's do.
    fn default() -> Self {
        Self::new(false)
    }
}

impl GuiClientState {
    /// The state a fresh `App` starts with, with the auto-hide preference it
    /// reads from the Client prefs.
    pub(crate) fn new(player_auto_hide: bool) -> Self {
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

    /// Write the auto-hide preference (design D4): the GUI owns the
    /// `gui_player_auto_hide` key, so it persists it through the core's
    /// Client-key writer.
    pub(in crate::app) fn save_player_auto_hide(&self) {
        App::save_client_pref(
            "gui_player_auto_hide",
            serde_json::json!(self.player_auto_hide),
        );
    }

    /// Apply one effect a library event raised (design D4a (c)): this state
    /// does the store work on its own fields, in the order the event raised
    /// it, and the core `app` serves the work that stays the core's.
    pub(in crate::app) fn apply_library_effect(
        &mut self,
        app: &mut App,
        effect: ClientLibraryEffect,
    ) {
        match effect {
            // The rows in view changed, so their artwork sources are stale:
            // the GUI re-queues the artwork the in-view rows lack (the call
            // this effect replaced, design D4a's table).
            ClientLibraryEffect::RowArtSourcesChanged => {
                crate::app::gui_library::queue_in_view_row_fetches(app, self);
            }
            // A landed target retires its terminal failure mark, so a later
            // cache miss can fetch it again.
            ClientLibraryEffect::FeedTargetLanded { lib_idx, selection } => {
                if let Some(claim) = self
                    .emby_libs
                    .get_mut(&lib_idx)
                    .and_then(|stored| stored.feed_claim.as_mut())
                {
                    claim.failed.remove(&selection);
                }
            }
            // A failed target records the mark, so the shown pass skips it
            // until a user press clears it and no repaint retries it.
            ClientLibraryEffect::FeedTargetFailed { lib_idx, selection } => {
                if let Some(claim) = self
                    .emby_libs
                    .get_mut(&lib_idx)
                    .and_then(|stored| stored.feed_claim.as_mut())
                {
                    claim.failed.insert(selection);
                }
            }
        }
    }
}

/// The GUI Client's library stores, as the core's library event seam takes
/// them (design D4a (c)): the state reads and writes its own fields, through
/// the `app` the core passes for the work that stays the core's.
impl ClientLibraryStores for GuiClientState {
    fn wants_feed_target(
        &self,
        app: &App,
        lib_idx: usize,
        target: &FeedHomeVideoSelection,
    ) -> bool {
        // The claim this state holds still wants the target, checked against
        // the effective set the core's root listing resolves.
        self.emby_libs
            .get(&lib_idx)
            .is_some_and(|selection| selection.feed_claim.is_some())
            && emby_feed_wants_target_in_set(app, self, lib_idx, target)
    }

    fn apply(&mut self, app: &mut App, effect: ClientLibraryEffect) {
        self.apply_library_effect(app, effect);
    }

    fn install_group_albums(&mut self, event: crate::app::gui_library::GuiGroupAlbumsEvent) {
        let lib_idx = event.lib_idx;
        let group_id = event.group_id;
        self.music_albums
            .entry(lib_idx)
            .or_default()
            .insert(group_id, group_albums_mark(event.result));
    }

    fn install_emby_roots(
        &mut self,
        lib_idx: usize,
        items: crate::app::gui_library::GuiEmbyRootItems,
    ) {
        self.emby_roots.insert(lib_idx, items);
    }

    fn install_shows_new(
        &mut self,
        library_id: String,
        request: crate::app::gui_library::GuiSectionRequest<mbv_emby_model::EmbyItem>,
    ) {
        self.shows_new.insert(library_id, request);
    }
}
