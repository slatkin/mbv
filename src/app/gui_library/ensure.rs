//! The GUI Library's one destination-load driver (gui-one-destination-ensure
//! design D1, D3): every destination kind resolves to a small site value,
//! and one ordered pass runs that site's root, selection, and dependents
//! steps. The order is fixed for every destination, so a destination cannot
//! start a dependent load before its root list, and a destination with no
//! work for a step still takes the step as a no-op.
//!
//! A step that calls for a load ends the pass (design D3), as the Shows root
//! load returned after `fetch_series_detail`, so no two steps start their
//! requests in one pass. A step reached in a later pass keeps its own
//! idempotent guard, so a repaint never repeats a request.
//!
//! Two callers reach the pass: the `mbv-gui` crate's frame path, through
//! [`GuiCore::ensure_gui_active_destination`], and the destination's open step,
//! after its user-action work (design D2, D4). A collection command can name
//! an Audiobooks or Podcasts library the tab is not on, so the driver itself
//! holds the active-tab guard for those two kinds.

use std::collections::HashSet;

use crate::app::App;
use crate::app::gui_client_state::GuiClientState;
use crate::gui::GuiCore;

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
/// every destination shares (design D1). A step a destination has no work for
/// keeps its default no-op; the driver's exhaustive match on
/// `GuiDestinationKind` is what makes a new destination state its site.
pub(in crate::app) trait GuiDestinationLoads {
    /// Start the destination's root list load, or do nothing when the root is
    /// already loaded, in flight, or owned by another path.
    fn ensure_root(&self, _app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    /// Start the load the root list's selection needs, or do nothing when the
    /// selection is already loaded or in flight.
    fn ensure_selection(&self, _app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        GuiStepOutcome::Continue
    }

    /// Start the content that depends on the selection, or do nothing when the
    /// dependency is already loaded or in flight. It is the last step, so it
    /// reports nothing.
    fn ensure_dependents(&self, _app: &mut App, _gui: &mut GuiClientState) {}
}

impl GuiCore {
    /// Run the one ordered load pass for `kind` (design D1): resolve the
    /// destination to its site value, then run that site's three steps in
    /// order. The match is exhaustive and has no wildcard arm, so a new
    /// destination kind does not compile until it names its site. The shell
    /// loaders of Audiobooks and Podcasts read the active tab's browse state,
    /// and a collection command can carry a stale frame's destination, so
    /// those two kinds run only for a library of their kind that is the active
    /// tab.
    pub(in crate::app) fn ensure_gui_destination(&mut self, kind: GuiDestinationKind) {
        let app = &mut self.app;
        let gui = &mut self.gui;
        match kind {
            GuiDestinationKind::Home => run_gui_destination_loads(app, gui, &HomeLoads),
            GuiDestinationKind::Music { lib_idx } => {
                run_gui_destination_loads(app, gui, &MusicLoads { lib_idx });
            }
            GuiDestinationKind::Shows { lib_idx } => {
                run_gui_destination_loads(app, gui, &ShowsLoads { lib_idx });
            }
            GuiDestinationKind::Library { lib_idx } => {
                run_gui_destination_loads(app, gui, &EmbyLoads { lib_idx });
            }
            GuiDestinationKind::Audiobooks { lib_idx } => {
                if app.active_audiobookshelf_library_index() == Some(lib_idx)
                    && super::audiobooks::abs_book_library(app, lib_idx)
                {
                    run_gui_destination_loads(app, gui, &AudiobooksLoads { lib_idx });
                }
            }
            GuiDestinationKind::Podcasts { lib_idx } => {
                if app.active_audiobookshelf_library_index() == Some(lib_idx)
                    && super::podcasts::podcast_library(app, lib_idx)
                {
                    run_gui_destination_loads(app, gui, &PodcastsLoads { lib_idx });
                }
            }
            GuiDestinationKind::Feeds => run_gui_destination_loads(app, gui, &FeedsLoads),
        }
    }

    /// Run the presented destination's one ordered load pass (design D2): the
    /// `mbv-gui` crate's frame path makes this one call before it builds its
    /// sections, so no section starts a load of its own. A session that
    /// presents no destination loads nothing, and the pass runs for the active
    /// destination only, whose steps then hold their own idempotent guards.
    /// It returns that destination, so the caller does not resolve it again.
    pub fn ensure_gui_active_destination(&mut self) -> Option<GuiDestinationKind> {
        let kind = super::tabs::active_destination(&self.app)?;
        self.ensure_gui_destination(kind);
        Some(kind)
    }
}

/// Run one site's steps in the shared order (design D1): root, then selection,
/// then dependents, stopping at the first step that calls for a load (design
/// D3). This is the only place the order lives; every destination reaches it
/// through [`GuiCore::ensure_gui_destination`]. It takes the `App` the sites
/// step over, so it stays a core driver and never a GUI-core method.
pub(in crate::app) fn run_gui_destination_loads<L: GuiDestinationLoads>(
    app: &mut App,
    gui: &mut GuiClientState,
    loads: &L,
) {
    if loads.ensure_root(app, gui) == GuiStepOutcome::LoadStarted {
        return;
    }
    if loads.ensure_selection(app, gui) == GuiStepOutcome::LoadStarted {
        return;
    }
    loads.ensure_dependents(app, gui);
}

/// Whether an Audiobookshelf catalog (book or show list) is unloaded in a shape
/// a catalog load may start from (design D3, Risks): an empty list, a zero
/// total, no page in flight, no replacement batch collecting, and no failed
/// mark. The in-flight page marks count as unloaded's negation, because the
/// startup chain's first page is already walking and a load beside it would
/// start a second chain.
///
/// The failed mark is load-bearing: a failed fetch records `state.error` and
/// aborts the replacement batch, which leaves the list empty, the total zero,
/// and no page in flight, so without the mark a repaint would retry the failed
/// chain every frame. An open clears the mark (design D4); a repaint never
/// does. Both catalog kinds call this one rule from their root step, so the
/// two starts cannot drift.
pub(in crate::app::gui_library) fn catalog_unloaded(
    list_empty: bool,
    total: usize,
    loading_pages: &HashSet<usize>,
    no_replacement: bool,
    no_error: bool,
) -> bool {
    list_empty && total == 0 && loading_pages.is_empty() && no_replacement && no_error
}

/// The Home destination's site: Home rows arrive with the shared Home data,
/// so every step is a no-op.
#[derive(Debug)]
struct HomeLoads;

impl GuiDestinationLoads for HomeLoads {}

/// The Music destination's site (design D3): the shared startup fetch owns the
/// groups catalog, so root and dependents keep their no-ops and the shown groups'
/// album fetches are the selection step.
#[derive(Debug)]
struct MusicLoads {
    lib_idx: usize,
}

impl GuiDestinationLoads for MusicLoads {
    /// Start the shown groups' album fetches when the store holds no list for
    /// them (design D5, gui-collection-dropdown design D1): the frame path
    /// runs this for the Music destination, so All's and every checked
    /// group's list loads without a pick. One fetch runs at a time per
    /// library: the next group's fetch starts on a later pass, when the
    /// previous one landed. A failed fetch is not retried here: a pass never
    /// re-issues it, so a hard failure does not retry on every paint; a
    /// selection change through [`GuiCore::install_gui_library_collection`] drops
    /// the selection's failed marks, so the next pass starts their fetches
    /// again.
    fn ensure_selection(&self, app: &mut App, gui: &mut GuiClientState) -> GuiStepOutcome {
        let set = super::projection::effective_music_set(app, gui, self.lib_idx).into_owned();
        let group_ids = super::projection::selected_groups(app, gui, self.lib_idx, &set)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if group_ids.iter().any(|group_id| {
            matches!(
                gui.music_albums
                    .get(&self.lib_idx)
                    .and_then(|groups| groups.get(group_id)),
                Some(GuiGroupAlbums::Loading)
            )
        }) {
            return GuiStepOutcome::Continue;
        }
        let Some(group_id) = group_ids.into_iter().find(|group_id| {
            !gui.music_albums
                .get(&self.lib_idx)
                .is_some_and(|groups| groups.contains_key(group_id))
        }) else {
            return GuiStepOutcome::Continue;
        };
        super::store::fetch_group_albums(app, gui, self.lib_idx, group_id);
        GuiStepOutcome::LoadStarted
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
    fn ensure_root(&self, app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        if super::shows_projection::shows_root_needs_load(app, self.lib_idx) {
            super::tv::load_shows_root(app, self.lib_idx);
            return GuiStepOutcome::LoadStarted;
        }
        GuiStepOutcome::Continue
    }

    fn ensure_selection(&self, app: &mut App, gui: &mut GuiClientState) -> GuiStepOutcome {
        let Some(series_id) = super::shows_projection::shows_default_series(app, gui, self.lib_idx)
        else {
            return GuiStepOutcome::Continue;
        };
        if app.series_detail(&series_id).is_some() {
            return GuiStepOutcome::Continue;
        }
        app.fetch_series_detail(&series_id);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_dependents(&self, app: &mut App, gui: &mut GuiClientState) {
        let Some(series_id) = super::shows_projection::shows_default_series(app, gui, self.lib_idx)
        else {
            return;
        };
        let Some(season_id) = super::shows_projection::hero::shows_selected_season(
            app,
            gui,
            self.lib_idx,
            &series_id,
        ) else {
            return;
        };
        app.fetch_series_season_episodes(series_id, season_id);
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
    fn ensure_root(&self, app: &mut App, gui: &mut GuiClientState) -> GuiStepOutcome {
        if !super::emby::emby_library(app, self.lib_idx)
            || app.is_feed_home_video_library(self.lib_idx)
        {
            return GuiStepOutcome::Continue;
        }
        super::emby::fetch_emby_root_items(app, gui, self.lib_idx);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_selection(&self, app: &mut App, gui: &mut GuiClientState) -> GuiStepOutcome {
        if !super::emby::emby_library(app, self.lib_idx)
            || !app.is_feed_home_video_library(self.lib_idx)
        {
            return GuiStepOutcome::Continue;
        }
        super::emby::ensure_emby_feed_fetch(app, gui, self.lib_idx);
        GuiStepOutcome::LoadStarted
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
    fn ensure_root(&self, app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        if !super::audiobooks::abs_catalog_needs_load(app, self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        app.audiobookshelf_book_refresh();
        GuiStepOutcome::LoadStarted
    }

    fn ensure_selection(&self, app: &mut App, gui: &mut GuiClientState) -> GuiStepOutcome {
        if let Some(book_id) = super::audiobooks::abs_effective_pick(app, gui, self.lib_idx) {
            app.start_audiobookshelf_book_detail(book_id);
        }
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
    fn ensure_root(&self, app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        if !super::podcasts::podcast_catalog_needs_load(app, self.lib_idx) {
            return GuiStepOutcome::Continue;
        }
        app.audiobookshelf_refresh(false);
        GuiStepOutcome::LoadStarted
    }

    fn ensure_selection(&self, app: &mut App, gui: &mut GuiClientState) -> GuiStepOutcome {
        super::podcasts::commit_podcast_effective_scope(app, gui, self.lib_idx);
        GuiStepOutcome::Continue
    }

    fn ensure_dependents(&self, app: &mut App, gui: &mut GuiClientState) {
        super::podcasts::ensure_podcast_dependents(app, gui, self.lib_idx);
    }
}

/// The Feeds destination's site (design D3): the shared startup fetch owns the
/// feed entries, so every step is a no-op.
#[derive(Debug)]
struct FeedsLoads;

impl GuiDestinationLoads for FeedsLoads {}
