//! The GUI Library's one destination-load driver (gui-one-destination-ensure
//! design D1, D3): every destination kind resolves to a small site value,
//! and one ordered pass runs that site's root, selection, and dependents
//! steps. The order is fixed for every destination, so a destination cannot
//! start a dependent load before its root list, and a destination with no
//! work for a step still takes the step as a no-op.
//!
//! A step that calls for a load ends the pass (design D3), as
//! `ensure_gui_shows_workspace` returned after `fetch_series_detail`, so no
//! two steps start their requests in one pass. A step reached in a later pass
//! keeps its own idempotent guard, so a repaint never repeats a request.
//!
//! The per-destination `App` ensure methods still exist, because the
//! `mbv-gui` crate's frame path calls them; each delegates here, so both
//! paths run the same code.

use crate::app::App;

use super::GuiDestinationKind;
use super::store::GuiGroupAlbums;

/// What one load step leaves for the rest of the pass (design D3): the step
/// that calls for a load ends the pass, so the driver never starts two
/// requests in one pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum GuiStepOutcome {
    /// The step called for a load, so the pass ends here.
    LoadStarted,
    /// The step called for no load, so the next step runs.
    Continue,
}

/// The three load steps one GUI Library destination takes, in the one order
/// every destination shares (design D1). The methods are required, so a new
/// destination must state its work for each step rather than skip one.
pub(in crate::app) trait GuiDestinationLoads {
    /// Start the destination's root list load, or do nothing when the root is
    /// already loaded, in flight, or owned by another path.
    fn ensure_root(&self, app: &mut App) -> GuiStepOutcome;

    /// Start the load the root list's selection needs, or do nothing when the
    /// selection is already loaded or in flight.
    fn ensure_selection(&self, app: &mut App) -> GuiStepOutcome;

    /// Start the content that depends on the selection, or do nothing when the
    /// dependency is already loaded or in flight.
    fn ensure_dependents(&self, app: &mut App) -> GuiStepOutcome;
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

    /// Run one site's steps in the shared order (design D1): root, then
    /// selection, then dependents, stopping at the first step that calls for a
    /// load (design D3). This is the only place the order lives; every
    /// destination reaches it through [`Self::ensure_gui_destination`].
    pub(in crate::app) fn run_gui_destination_loads<L: GuiDestinationLoads>(&mut self, loads: &L) {
        if loads.ensure_root(self) == GuiStepOutcome::LoadStarted {
            return;
        }
        if loads.ensure_selection(self) == GuiStepOutcome::LoadStarted {
            return;
        }
        loads.ensure_dependents(self);
    }
}

/// The active-destination contract the Audiobooks and Podcasts steps share
/// (design D5): the shelf loaders read the active tab's browse state, and the
/// driver runs only for `gui_active_destination()`, which is that tab, so the
/// repeated test the open steps carried becomes this assertion instead of a
/// silent no-op. A `lib_idx` that names no library of the kind is still a
/// no-op, through the steps' own destination guards.
///
/// The assertion states the premise; the app-facing entries hold it. See
/// [`App::gui_abs_destination_active`] for the runtime half.
fn debug_assert_active_destination(app: &App, lib_idx: usize) {
    debug_assert_eq!(
        app.tab.audiobookshelf_index(),
        Some(lib_idx),
        "the destination's load pass runs for the active tab only"
    );
}

impl App {
    /// Whether the Audiobookshelf library `lib_idx` is the active tab — the one
    /// rule the app-facing Audiobooks and Podcasts entries share (design D5;
    /// review of units 2.4 and 2.5). The steps' loaders read the active tab's
    /// browse state, so every entry must test the tab before it enters the
    /// driver: the frame path only ever passes the active destination, but a
    /// collection command carries the destination a stale frame painted, which
    /// can name a podcast library the tab is not on. Testing it here rather
    /// than in each entry keeps the rule in one place, and the steps'
    /// `debug_assert_active_destination` then states the premise this guard
    /// upholds.
    pub(in crate::app::gui_library) fn gui_abs_destination_active(&self, lib_idx: usize) -> bool {
        self.tab.audiobookshelf_index() == Some(lib_idx)
    }
}

/// The Home destination's site: Home rows arrive with the shared Home data,
/// so every step is a no-op.
#[derive(Debug)]
struct HomeLoads;

impl GuiDestinationLoads for HomeLoads {
    fn ensure_root(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    fn ensure_selection(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    fn ensure_dependents(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }
}

/// The Music destination's site (design D3): the shared startup fetch owns the
/// groups catalog, so root and dependents are no-ops and the shown groups'
/// album fetches are the selection step.
#[derive(Debug)]
struct MusicLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for MusicLoads {
    fn ensure_root(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    /// Start the shown groups' album fetches when the store holds no list for
    /// them (design D5, gui-collection-dropdown design D1): the frame path
    /// runs this for the Music destination, so All's and every checked
    /// group's list loads without a pick. One fetch runs at a time per
    /// library: the next group's fetch starts on a later pass, when the
    /// previous one landed. A failed fetch is not retried here: a pass never
    /// re-issues it, so a hard failure does not retry on every paint; a
    /// selection change through [`App::install_gui_library_collection`] drops
    /// the selection's failed marks, so the next pass starts their fetches
    /// again.
    fn ensure_selection(&self, app: &mut App) -> GuiStepOutcome {
        let set = app.gui_effective_music_set(self.lib_idx).into_owned();
        let group_ids = app
            .gui_selected_groups(self.lib_idx, &set)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if group_ids.iter().any(|group_id| {
            matches!(
                app.gui_music_albums
                    .get(&self.lib_idx)
                    .and_then(|groups| groups.get(group_id)),
                Some(GuiGroupAlbums::Loading)
            )
        }) {
            return GuiStepOutcome::Continue;
        }
        let Some(group_id) = group_ids.into_iter().find(|group_id| {
            !app.gui_music_albums
                .get(&self.lib_idx)
                .is_some_and(|groups| groups.contains_key(group_id))
        }) else {
            return GuiStepOutcome::Continue;
        };
        app.fetch_gui_group_albums(self.lib_idx, group_id);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_dependents(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }
}

/// The Shows destination's site (design D3): the root step starts the All
/// series list when the library holds none, the selection step starts the
/// default series' detail, and the dependents step starts the selected
/// season's episodes once that detail is cached.
#[derive(Debug)]
struct ShowsLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for ShowsLoads {
    fn ensure_root(&self, app: &mut App) -> GuiStepOutcome {
        if app.gui_shows_root_needs_load(self.lib_idx) {
            app.gui_tv_open(self.lib_idx);
            return GuiStepOutcome::LoadStarted;
        }
        GuiStepOutcome::Continue
    }

    fn ensure_selection(&self, app: &mut App) -> GuiStepOutcome {
        let Some(series_id) = app.gui_shows_default_series(self.lib_idx) else {
            return GuiStepOutcome::Continue;
        };
        if app.series_detail_cache.contains_key(&series_id) {
            return GuiStepOutcome::Continue;
        }
        app.fetch_series_detail(&series_id);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_dependents(&self, app: &mut App) -> GuiStepOutcome {
        let Some(series_id) = app.gui_shows_default_series(self.lib_idx) else {
            return GuiStepOutcome::Continue;
        };
        let Some(season_id) = app.gui_shows_selected_season(self.lib_idx, &series_id) else {
            return GuiStepOutcome::Continue;
        };
        app.fetch_series_season_episodes(series_id, season_id);
        GuiStepOutcome::LoadStarted
    }
}

/// The plain Emby library destination's site (design D3): a plain library's
/// root step fetches its root list, and a feed-view library's selection step
/// fetches the first target of its checked set. A library is one kind or the
/// other, so the two steps never both have work.
#[derive(Debug)]
struct EmbyLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for EmbyLoads {
    fn ensure_root(&self, app: &mut App) -> GuiStepOutcome {
        if !app.gui_emby_library(self.lib_idx) || app.is_feed_home_video_library(self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        app.fetch_gui_emby_root_items(self.lib_idx);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_selection(&self, app: &mut App) -> GuiStepOutcome {
        if !app.gui_emby_library(self.lib_idx) || !app.is_feed_home_video_library(self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        app.ensure_gui_emby_feed_fetch(self.lib_idx);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_dependents(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }
}

/// The Audiobooks destination's site (design D3): the root step starts the
/// book catalog's refresh when the catalog is unloaded in a loadable shape,
/// the selection step starts the effective pick's detail load, and the
/// dependents step has no work.
#[derive(Debug)]
struct AudiobooksLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for AudiobooksLoads {
    fn ensure_root(&self, app: &mut App) -> GuiStepOutcome {
        debug_assert_active_destination(app, self.lib_idx);
        if !app.gui_abs_book_library(self.lib_idx) || !app.gui_abs_catalog_needs_load(self.lib_idx)
        {
            return GuiStepOutcome::Continue;
        }
        app.audiobookshelf_book_refresh();
        GuiStepOutcome::LoadStarted
    }

    fn ensure_selection(&self, app: &mut App) -> GuiStepOutcome {
        debug_assert_active_destination(app, self.lib_idx);
        if !app.gui_abs_book_library(self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        if let Some(book_id) = app.gui_abs_effective_pick(self.lib_idx) {
            app.start_audiobookshelf_book_detail(book_id);
        }
        GuiStepOutcome::Continue
    }

    fn ensure_dependents(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }
}

/// The Podcasts destination's site (design D3): the root step starts the show
/// list's refresh when the list is unloaded in a loadable shape, the selection
/// step commits the checked shows' scope, and the dependents step starts All's
/// shelf fetch and the bounded per-show fan-out.
#[derive(Debug)]
struct PodcastsLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for PodcastsLoads {
    fn ensure_root(&self, app: &mut App) -> GuiStepOutcome {
        debug_assert_active_destination(app, self.lib_idx);
        if !app.gui_podcast_library(self.lib_idx)
            || !app.gui_podcast_catalog_needs_load(self.lib_idx)
        {
            return GuiStepOutcome::Continue;
        }
        app.audiobookshelf_refresh(false);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_selection(&self, app: &mut App) -> GuiStepOutcome {
        debug_assert_active_destination(app, self.lib_idx);
        if !app.gui_podcast_library(self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        app.commit_gui_podcast_effective_scope(self.lib_idx);
        GuiStepOutcome::Continue
    }

    fn ensure_dependents(&self, app: &mut App) -> GuiStepOutcome {
        debug_assert_active_destination(app, self.lib_idx);
        if !app.gui_podcast_library(self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        app.ensure_gui_podcast_dependents(self.lib_idx);
        GuiStepOutcome::Continue
    }
}

/// The Feeds destination's site (design D3): the shared startup fetch owns the
/// feed entries, so every step is a no-op.
#[derive(Debug)]
struct FeedsLoads;

impl GuiDestinationLoads for FeedsLoads {
    fn ensure_root(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    fn ensure_selection(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    fn ensure_dependents(&self, _app: &mut App) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }
}
