//! The GUI Library's one destination-load driver (gui-one-destination-ensure
//! design D1, D3): every destination kind resolves to a small site value,
//! and one ordered pass runs that site's root, selection, and dependents
//! steps. The order is fixed for every destination, so a destination cannot
//! start a dependent load before its root list, and a destination with no
//! work for a step still takes the step as a no-op.
//!
//! This unit adds the contract only. The frame path and the open command
//! still call their own per-destination ensures, and the sites here delegate
//! to those same methods, so a driver run changes no behaviour. The later
//! units move each body into its steps and point the callers at the driver.

use crate::app::App;

use super::GuiDestinationKind;

/// The three load steps one GUI Library destination takes, in the one order
/// every destination shares (design D1). The methods are required, so a new
/// destination must state its work for each step rather than skip one.
pub(in crate::app) trait GuiDestinationLoads {
    /// Start the destination's root list load, or do nothing when the root is
    /// already loaded, in flight, or owned by another path.
    fn ensure_root(&self, app: &mut App);

    /// Start the load the root list's selection needs, or do nothing when the
    /// selection is already loaded or in flight.
    fn ensure_selection(&self, app: &mut App);

    /// Start the content that depends on the selection, or do nothing when the
    /// dependency is already loaded or in flight.
    fn ensure_dependents(&self, app: &mut App);
}

impl App {
    /// Run the one ordered load pass for `kind` (design D1): resolve the
    /// destination to its site value, then run that site's three steps in
    /// order. The match is exhaustive and has no wildcard arm, so a new
    /// destination kind does not compile until it names its site.
    pub(in crate::app) fn ensure_gui_destination(&mut self, kind: GuiDestinationKind) {
        match kind {
            GuiDestinationKind::Home => self.run_gui_destination_loads(&HomeLoads),
            GuiDestinationKind::Music { lib_idx } => {
                self.run_gui_destination_loads(&MusicLoads { lib_idx });
            }
            GuiDestinationKind::Shows { lib_idx } => {
                self.run_gui_destination_loads(&ShowsLoads { lib_idx });
            }
            GuiDestinationKind::Library { lib_idx } => {
                self.run_gui_destination_loads(&EmbyLoads { lib_idx });
            }
            GuiDestinationKind::Audiobooks { lib_idx } => {
                self.run_gui_destination_loads(&AudiobooksLoads { lib_idx });
            }
            GuiDestinationKind::Podcasts { lib_idx } => {
                self.run_gui_destination_loads(&PodcastsLoads { lib_idx });
            }
            GuiDestinationKind::Feeds => self.run_gui_destination_loads(&FeedsLoads),
        }
    }

    /// Run one site's steps in the shared order (design D1). This is the only
    /// place the order lives; every destination reaches it through
    /// [`Self::ensure_gui_destination`].
    pub(in crate::app) fn run_gui_destination_loads<L: GuiDestinationLoads>(&mut self, loads: &L) {
        loads.ensure_root(self);
        loads.ensure_selection(self);
        loads.ensure_dependents(self);
    }
}

/// The Home destination's site: Home rows arrive with the shared Home data,
/// so every step is a no-op.
#[derive(Debug)]
struct HomeLoads;

impl GuiDestinationLoads for HomeLoads {
    fn ensure_root(&self, _app: &mut App) {}

    fn ensure_selection(&self, _app: &mut App) {}

    fn ensure_dependents(&self, _app: &mut App) {}
}

/// The Music destination's site (design D3): the shared startup fetch owns the
/// groups catalog, so root and dependents are no-ops and the shown groups'
/// album fetches are the selection step.
#[derive(Debug)]
struct MusicLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for MusicLoads {
    fn ensure_root(&self, _app: &mut App) {}

    fn ensure_selection(&self, app: &mut App) {
        app.ensure_gui_music_selection(self.lib_idx);
    }

    fn ensure_dependents(&self, _app: &mut App) {}
}

/// The Shows destination's site. This unit keeps the existing workspace pass
/// whole in the root step, so a driver run matches today's one call; the later
/// unit splits the default series' detail and the selected season's episodes
/// into the selection and dependents steps.
#[derive(Debug)]
struct ShowsLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for ShowsLoads {
    fn ensure_root(&self, app: &mut App) {
        app.ensure_gui_shows_workspace(self.lib_idx);
    }

    fn ensure_selection(&self, _app: &mut App) {}

    fn ensure_dependents(&self, _app: &mut App) {}
}

/// The plain Emby library destination's site. This unit keeps the existing
/// selection pass — the plain library's root fetch and the feed-view library's
/// first feed fetch — whole in the root step; the later unit splits the
/// feed-view fetch into the selection step.
#[derive(Debug)]
struct EmbyLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for EmbyLoads {
    fn ensure_root(&self, app: &mut App) {
        app.ensure_gui_emby_selection(self.lib_idx);
    }

    fn ensure_selection(&self, _app: &mut App) {}

    fn ensure_dependents(&self, _app: &mut App) {}
}

/// The Audiobooks destination's site. The catalog refresh still belongs to
/// `gui_abs_open` in this unit, so the root is a no-op; the effective pick's
/// detail load is the selection step.
#[derive(Debug)]
struct AudiobooksLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for AudiobooksLoads {
    fn ensure_root(&self, _app: &mut App) {}

    fn ensure_selection(&self, app: &mut App) {
        app.ensure_gui_abs_selection(self.lib_idx);
    }

    fn ensure_dependents(&self, _app: &mut App) {}
}

/// The Podcasts destination's site. The show-list refresh still belongs to
/// `gui_podcast_open` in this unit, so the root is a no-op; the existing
/// selection pass — the scope commit, All's shelf, and the fan-out — runs
/// whole in the selection step, and the later unit splits it.
#[derive(Debug)]
struct PodcastsLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for PodcastsLoads {
    fn ensure_root(&self, _app: &mut App) {}

    fn ensure_selection(&self, app: &mut App) {
        app.ensure_gui_podcast_selection(self.lib_idx);
    }

    fn ensure_dependents(&self, _app: &mut App) {}
}

/// The Feeds destination's site (design D3): the shared startup fetch owns the
/// feed entries, so every step is a no-op.
#[derive(Debug)]
struct FeedsLoads;

impl GuiDestinationLoads for FeedsLoads {
    fn ensure_root(&self, _app: &mut App) {}

    fn ensure_selection(&self, _app: &mut App) {}

    fn ensure_dependents(&self, _app: &mut App) {}
}
