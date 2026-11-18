//! The GUI Library's Shows projection (add-gui-library-panel design D14;
//! gui-collection-dropdown design D8): the series list with its rail letters,
//! the selected series, and the Hero. A series' Hero is the Primary poster,
//! the TUI series producer's facts, its season pills, and its Workspace
//! episodes. The projection reads `App` data only, so the TUI's own
//! projections and paint do not change, and the row and sort rules are the
//! TUI's: series by `SortName`, seasons by `SortName` (the server order the
//! detail fetch requests), episodes by `IndexNumber`.

mod hero;

use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_queue::TvContentMode;
use mbv_render::effective_sort_str;
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::GuiLibraryArtwork;
use super::home::GuiArtShape;
use super::projection::sort_letter;
use super::tabs::GuiDestinationKind;
use super::tv::GuiTvShowsSelection;
use crate::app::App;
use mbv_ui_model::meta::MetaSegment;

/// One series node of the All series tree (design D14): the series row, its
/// rail letter, and the seasons the cached detail holds. The tree is the
/// Music tree's shape (artists → albums → tracks), with the TUI's node kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsSeries {
    /// The series' stable Service id: the select command names it, and it
    /// keys the detail cache.
    pub id: String,
    /// The series' display name, the row's label.
    pub title: String,
    /// The letter heading and rail letter above this row.
    pub sort_letter: char,
    /// Whether the node opens: a series whose detail has not landed counts as
    /// expandable (the TUI's optimistic flag, corrected when the detail
    /// lands), and a landed detail is expandable exactly when it holds a
    /// season.
    pub expandable: bool,
    /// The series' seasons in the detail's order; empty while the detail has
    /// not landed.
    pub seasons: Vec<GuiShowsSeasonNode>,
}

/// One season node under a series (design D14): the season row and its
/// episodes, or `None` while that season's episode load has not landed — the
/// Music album node's rule for an uncached tracklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsSeasonNode {
    /// The season's stable Service id.
    pub id: String,
    /// The season's display name.
    pub name: String,
    /// The season's episodes in their fetch order; `None` until the load
    /// lands.
    pub episodes: Option<Vec<GuiShowsEpisodeRow>>,
}

/// One season pill of a series' Hero (design D12): the detail's seasons in
/// their fetch order, the selected one the core's season selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsSeasonPill {
    /// The season's stable Service id: the select-season command carries it.
    pub season_id: String,
    /// The pill's label.
    pub name: String,
    /// Whether this is the Workspace's selected season.
    pub selected: bool,
}

/// One episode row of the Hero's Workspace (design D12): the number, name,
/// runtime, and played/progress state the TUI's episode rows paint
/// (`crates/mbv-components/src/tv_content/episode_rows.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsEpisodeRow {
    /// The episode's stable Service id: a play, play-next, or add command
    /// carries it.
    pub episode_id: String,
    /// The number the row paints: the Service's index number, or the
    /// episode's position when it holds none.
    pub number: i64,
    /// The episode's display name.
    pub name: String,
    /// The episode's runtime, `M:SS` or `H:MM:SS`; empty when the Service
    /// holds none.
    pub duration: String,
    /// The played/progress state, the shared media-list semantic state's
    /// arms.
    pub state: MediaSemanticState,
}

/// A series Hero's Workspace (design D12): the season pills and the selected
/// season's episodes. The season pills start on the first season — the core
/// never keeps a season across a series change — so the selected season is
/// the stored choice while the detail still holds it, else the first season.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiShowsWorkspace {
    /// The season pills in the detail's order.
    pub seasons: Vec<GuiShowsSeasonPill>,
    /// The selected season's id; `None` while the detail holds no season.
    pub selected_season: Option<String>,
    /// The selected season's episodes in their fetch order (the season
    /// fetch's `IndexNumber` ascending); empty while that fetch has not
    /// landed, which the projection's `workspace_loading` names.
    pub episodes: Vec<GuiShowsEpisodeRow>,
}

/// The Hero's facts for the selected series (design D12, D8): the shared
/// series producer's facts, the series' `Primary` poster, and the series Hero's
/// Workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsHero {
    /// The hero's title.
    pub title: String,
    /// The name of the largest unit the item belongs to (the invariant 30 header
    /// title rule): the show, the feed, or the series. `None` when the item is
    /// its own largest unit. The meta rows never repeat it.
    pub parent: Option<String>,
    /// The meta rows in display order.
    pub meta_rows: Vec<Vec<MetaSegment>>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does. The shared
    /// Emby hero producer fills none, so a Shows hero carries `None` exactly
    /// as the TUI's does.
    pub progress_row: Option<usize>,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the series' declared portrait arm,
    /// re-armed by the fetched image's own aspect.
    pub art_shape: GuiArtShape,
    /// The Hero's artwork, fetched through the image cache at the `Primary`
    /// chain. `None` when the item declares no image source; the window content
    /// paints its own placeholder there.
    pub art: Option<GuiLibraryArtwork>,
    /// The series Hero's Workspace; `None` while the selected series' detail
    /// has not landed.
    pub workspace: Option<GuiShowsWorkspace>,
}

/// The Shows projection the GUI Library panel paints (design D12, D8): the
/// series list, the core's selected series, and its Hero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiShowsProjection {
    /// Whether the series list has not landed yet.
    pub loading: bool,
    /// Whether the selected series' detail or the selected season's episodes
    /// fetch has not landed yet. The window paints the Workspace's loading
    /// state with it.
    pub workspace_loading: bool,
    /// The series list's tree (design D14): the series nodes with their
    /// seasons and loaded episodes.
    pub series: Vec<GuiShowsSeries>,
    /// The core's selected series (design D2, D12): the series the select
    /// command last named while the list still carries it, else the first row
    /// — the Hero default, so a series Hero paints as soon as the series list
    /// lands. `None` while the list holds no row.
    pub selected: Option<String>,
    /// The selected series' Hero facts; `None` while no series is selected.
    pub hero: Option<GuiShowsHero>,
}

impl App {
    /// The active destination's Shows library index (design D12): the
    /// destination whose Shows content the GUI Library panel paints. `None`
    /// for every other destination, which presents no Shows section.
    #[must_use]
    pub fn gui_shows_destination(&self) -> Option<usize> {
        match self.gui_active_destination() {
            Some(GuiDestinationKind::Shows { lib_idx }) => Some(lib_idx),
            _ => None,
        }
    }

    /// The Shows projection the GUI Library panel paints (design D12, D8): the
    /// series list, the selected series, and its Hero. A `lib_idx` that names
    /// no Shows library projects the empty projection, as the frame's gate
    /// excludes that destination anyway.
    #[must_use]
    pub fn gui_shows_projection(&mut self, lib_idx: usize) -> GuiShowsProjection {
        if !self.gui_tv_library(lib_idx) {
            return GuiShowsProjection::default();
        }
        let series = self.gui_shows_series_tree(lib_idx);
        let loading = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.last())
            .is_none_or(|level| level.loading);
        let selected = self.gui_shows_default_series(lib_idx);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.gui_shows_hero(lib_idx, selected));
        let workspace_loading = selected
            .as_ref()
            .is_some_and(|series_id| self.gui_shows_workspace_loading(lib_idx, series_id));
        let projection = GuiShowsProjection {
            loading,
            workspace_loading,
            series,
            selected,
            hero,
        };
        self.warm_gui_shows_hero_neighbours(lib_idx, &projection);
        projection
    }

    /// The series list's tree (design D14): every root-level series with
    /// its cached detail's seasons, and each season's episodes when their
    /// load has landed. The Music tree's shape over the TUI's node kinds; a
    /// series with no cached detail paints closed and expandable, and its
    /// open asks the core to load it.
    fn gui_shows_series_tree(&self, lib_idx: usize) -> Vec<GuiShowsSeries> {
        let Some(level) = self.libs.get(lib_idx).and_then(|lib| lib.nav_stack.last()) else {
            return Vec::new();
        };
        level
            .items
            .iter()
            .filter(|item| item.item_type == "Series")
            .map(|item| {
                let detail = self.series_detail_cache.get(&item.id);
                let seasons = detail
                    .map(|detail| {
                        detail
                            .seasons
                            .iter()
                            .map(|season| GuiShowsSeasonNode {
                                id: season.id.clone(),
                                name: season.name.clone(),
                                episodes: detail.episodes.get(&season.id).map(|episodes| {
                                    episodes
                                        .iter()
                                        .enumerate()
                                        .map(|(position, episode)| episode_row(episode, position))
                                        .collect()
                                }),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                GuiShowsSeries {
                    id: item.id.clone(),
                    title: item.name.clone(),
                    sort_letter: sort_letter(effective_sort_str(item)),
                    expandable: detail.is_none_or(|detail| !detail.seasons.is_empty()),
                    seasons,
                }
            })
            .collect()
    }

    /// The series the Shows Hero shows (design D12): the stored selection
    /// while the root list still carries it as a Series, else the root list's
    /// first series row — the Hero default, so the Hero paints as soon as the
    /// series list lands and the detail load can start for it before any
    /// select does. `None` while the root list holds no series.
    fn gui_shows_default_series(&self, lib_idx: usize) -> Option<String> {
        let in_root = |series_id: &str| {
            self.libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .is_some_and(|level| {
                    level
                        .items
                        .iter()
                        .any(|item| item.id == series_id && item.item_type == "Series")
                })
        };
        self.gui_shows_selection(lib_idx)
            .series_id
            .filter(|stored| in_root(stored))
            .or_else(|| {
                self.libs
                    .get(lib_idx)
                    .and_then(|lib| lib.nav_stack.last())
                    .and_then(|level| level.items.iter().find(|item| item.item_type == "Series"))
                    .map(|item| item.id.clone())
            })
    }

    /// Start the Shows root load and the selected series' detail and season
    /// loads when their caches do not hold them (design D14): the frame path
    /// calls this for the Shows destination, so a launch restored onto Shows
    /// — which never runs the `OpenDestination` click — still paints the series
    /// list, and the default first series' Hero paints its season pills and
    /// episodes without a select. `gui_tv_open` owns the root step; this call
    /// only starts it once, when no root level or no settled All list exists.
    /// Both series fetches deduplicate what is in flight or cached.
    pub fn ensure_gui_shows_workspace(&mut self, lib_idx: usize) {
        if self.gui_shows_root_needs_load(lib_idx) {
            self.gui_tv_open(lib_idx);
        }
        let Some(series_id) = self.gui_shows_default_series(lib_idx) else {
            return;
        };
        if !self.series_detail_cache.contains_key(&series_id) {
            self.fetch_series_detail(&series_id);
            return;
        }
        if let Some(season_id) = self.gui_shows_selected_season(lib_idx, &series_id) {
            self.fetch_series_season_episodes(series_id, season_id);
        }
    }

    /// Whether the frame path must start the Shows root load: no root level
    /// yet, or an empty settled root that is not yet the All series list. A
    /// root already loading, and an empty All list whose load has settled, are
    /// left alone so a repaint never starts a second request.
    fn gui_shows_root_needs_load(&self, lib_idx: usize) -> bool {
        if !self.gui_tv_library(lib_idx) {
            return false;
        }
        let Some(level) = self.libs.get(lib_idx).and_then(|lib| lib.nav_stack.last()) else {
            return true;
        };
        level.items.is_empty()
            && !level.loading
            && self.libs[lib_idx].tv_content_mode.as_ref() != Some(&TvContentMode::All)
    }

    /// The Shows library's selection, as the TUI's empty selection reads.
    pub(super) fn gui_shows_selection(&self, lib_idx: usize) -> GuiTvShowsSelection {
        self.gui_tv_shows.get(&lib_idx).cloned().unwrap_or_default()
    }

    /// The Emby item a Shows play command names (design D12): the root
    /// level's series, then the cached series details' episode lists, which
    /// the Workspace's rows name. `None` when no cached row carries the id, so
    /// a stale command plays nothing.
    pub(in crate::app::gui_library) fn gui_shows_item(
        &self,
        lib_idx: usize,
        item_id: &str,
    ) -> Option<EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if let Some(item) = lib
            .nav_stack
            .iter()
            .rev()
            .find_map(|level| level.items.iter().find(|item| item.id == item_id))
        {
            return Some(item.clone());
        }
        // A season is a folder item the Hero's season buttons name: the shared
        // folder play, play-next, and enqueue expand it, as they do a series.
        let details = self.series_detail_cache.values();
        let season = details
            .clone()
            .flat_map(|detail| detail.seasons.iter())
            .find(|season| season.id == item_id)
            .cloned();
        season.or_else(|| {
            details
                .flat_map(|detail| detail.episodes.values().flatten())
                .find(|episode| episode.id == item_id)
                .cloned()
        })
    }
}

/// One episode row of a season, as the tree and the Workspace paint it
/// (design D14): the number, the name, the runtime, and the played/progress
/// state the TUI's episode rows carry
/// (`crates/mbv-components/src/tv_content/episode_rows.rs`).
pub(super) fn episode_row(episode: &EmbyItem, position: usize) -> GuiShowsEpisodeRow {
    GuiShowsEpisodeRow {
        episode_id: episode.id.clone(),
        number: episode_number(episode, position),
        name: episode.name.clone(),
        duration: if episode.runtime_ticks > 0 {
            fmt_duration_short(episode.runtime_ticks / TICKS_PER_SECOND)
        } else {
            String::new()
        },
        state: MediaSemanticState::from_emby(episode),
    }
}

/// The episode number a Workspace row paints: the Service's index number, or
/// the episode's position when it holds none (the TUI's row rule).
fn episode_number(episode: &EmbyItem, position: usize) -> i64 {
    if episode.index_number > 0 {
        episode.index_number
    } else {
        i64::try_from(position)
            .unwrap_or(i64::MAX)
            .saturating_add(1)
    }
}
