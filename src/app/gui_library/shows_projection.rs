//! The GUI Library's Shows projection (add-gui-library-panel design D14;
//! gui-collection-dropdown design D8): the series list with its rail letters,
//! the selected series, and the Hero. A series' Hero is the Primary poster,
//! the TUI series producer's facts, its season pills, and its Workspace
//! episodes. The projection reads `App` data only, so the TUI's own
//! projections and paint do not change, and the row and sort rules are the
//! TUI's: series by `SortName`, seasons by `SortName` (the server order the
//! detail fetch requests), episodes by `IndexNumber`.

mod hero;

use mbv_components::tv_content::premiere_number_label;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_queue::{QueueItem, TvContentMode};
use mbv_render::effective_sort_str;
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::ui_util::{fmt_duration_short, row_date};

use super::GuiLibraryArtwork;
use super::GuiListSort;
use super::GuiListSortFrame;
use super::GuiSortTarget;
use super::artwork::{GuiRowArt, GuiRowMediaKind, emby_row_source, upcoming_row_source};
use super::home::GuiArtShape;
use super::list_sort::gui_title_sort_key;
use super::projection::sort_letter;
use super::request::GuiSectionRequest;
use super::shows_view::GuiShowsView;
use super::tabs::GuiDestinationKind;
use super::tv::GuiTvShowsSelection;
use crate::app::App;
use mbv_ui_model::meta::MetaSegment;

/// One flat row of the New or Upcoming list (gui-collection-dropdown design
/// D9): the row's stable identity, the two title lines the old Home sections
/// carried, and its gutter date. An Upcoming row is display-only: a press, a
/// play, and an enqueue name it but act on nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsRow {
    /// The stable Service id: an episode's item id or a premiere's stable key.
    pub id: String,
    /// The primary label, the show's name on a flat row.
    pub title: String,
    /// The secondary label, the episode's own title.
    pub subtitle: Option<String>,
    /// The gutter date, the provider date as `day month` text.
    pub date: Option<String>,
    /// The row's rail letter (design D6): the first letter of the series name
    /// a New row shows, or `#` for a digit- or symbol-led name. `Some` exactly
    /// on a New row while the list's sort is a name sort; an Upcoming row
    /// carries none.
    pub sort_letter: Option<char>,
    /// Whether the row takes no select, play, or enqueue.
    pub display_only: bool,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArt,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKind,
}

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
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArt,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKind,
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
    /// The one active view (design D8): All, New, or Upcoming. All is the
    /// default for a library with no stored choice.
    pub view: GuiShowsView,
    /// Whether the active list has not landed yet.
    pub loading: bool,
    /// The New or Upcoming list's flat rows in paint order (design D9); empty
    /// in All, where the `series` tree paints.
    pub rows: Vec<GuiShowsRow>,
    /// The New list's sort control (design D7): the target a press names, its
    /// active sort, and the choices the Shows library offers. `Some` only
    /// while the view is New, whose list is the one the sort orders.
    pub sort: Option<GuiListSortFrame>,
    /// Whether the selected series' detail or the selected season's episodes
    /// fetch has not landed yet. The window paints the Workspace's loading
    /// state with it.
    pub workspace_loading: bool,
    /// The All series list's tree (design D14): the series nodes with their
    /// seasons and loaded episodes; empty in the flat views.
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

    /// The Shows projection the GUI Library panel paints (design D12, D8,
    /// D9): the active view's rows, the selected row, and its Hero. All paints
    /// the series tree; New and Upcoming paint the merged flat lists. A
    /// `lib_idx` that names no Shows library projects the empty projection.
    #[must_use]
    pub fn gui_shows_projection(&mut self, lib_idx: usize) -> GuiShowsProjection {
        if !self.gui_tv_library(lib_idx) {
            return GuiShowsProjection::default();
        }
        let view = self.gui_shows_view(lib_idx);
        let (series, rows, selected, hero, workspace_loading, loading) = match view {
            GuiShowsView::All => {
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
                (
                    series,
                    Vec::new(),
                    selected,
                    hero,
                    workspace_loading,
                    loading,
                )
            }
            GuiShowsView::New => {
                let rows =
                    self.gui_shows_new_rows(self.gui_list_sort(GuiSortTarget::ShowsNew(lib_idx)));
                let selected = self
                    .gui_shows_flat_selection
                    .clone()
                    .filter(|id| rows.iter().any(|row| row.id == *id))
                    .or_else(|| rows.first().map(|row| row.id.clone()));
                let hero = selected
                    .as_ref()
                    .and_then(|id| self.gui_shows_new_item(id))
                    .map(|item| self.gui_shows_flat_hero(&item));
                let loading = self.libs.get(lib_idx).is_some_and(|lib| {
                    !self
                        .gui_shows_new
                        .get(&lib.library.id)
                        .is_some_and(GuiSectionRequest::is_landed)
                });
                (Vec::new(), rows, selected, hero, false, loading)
            }
            GuiShowsView::Upcoming => {
                // An Upcoming row is display-only: the Hero keeps the series
                // the All view showed, so the detail part stays meaningful.
                let selected = self.gui_shows_default_series(lib_idx);
                let hero = selected
                    .as_ref()
                    .and_then(|selected| self.gui_shows_hero(lib_idx, selected));
                (
                    Vec::new(),
                    self.gui_shows_upcoming_rows(),
                    selected,
                    hero,
                    false,
                    false,
                )
            }
        };
        let projection = GuiShowsProjection {
            view,
            loading,
            rows,
            sort: (view == GuiShowsView::New)
                .then(|| self.gui_list_sort_frame(GuiSortTarget::ShowsNew(lib_idx))),
            workspace_loading,
            series,
            selected,
            hero,
        };
        if view == GuiShowsView::All {
            self.warm_gui_shows_hero_neighbours(lib_idx, &projection);
        }
        projection
    }

    /// The New list's merged flat rows (design D9, D6): every TV library's
    /// latest episodes as one list, de-duplicated by item id, in the list's
    /// sort — the series name, then the season and episode numbers under
    /// Title, else newest `date_added` first. Every order's last tie-break is
    /// the item id, and the sort runs before the de-duplication, so a
    /// duplicated item keeps the row its order puts first. The split
    /// show/episode title and the provider-date gutter are the old `Latest
    /// episodes` section's.
    fn gui_shows_new_rows(&self, sort: GuiListSort) -> Vec<GuiShowsRow> {
        let mut items: Vec<EmbyItem> = self
            .gui_shows_new
            .values()
            .flat_map(GuiSectionRequest::items)
            .cloned()
            .collect();
        if sort.is_name_sort() {
            items.sort_by_cached_key(|item| {
                (
                    gui_title_sort_key(shows_new_series_name(item)),
                    item.parent_index_number,
                    item.index_number,
                    item.id.clone(),
                )
            });
        } else {
            items.sort_by_cached_key(|item| {
                (
                    std::cmp::Reverse(mbv_ui_model::home_latest::emby_date_added_secs(item)),
                    item.id.clone(),
                )
            });
        }
        let mut seen = std::collections::HashSet::new();
        items
            .into_iter()
            .filter(|item| seen.insert(item.id.clone()))
            .map(|episode| {
                let item = QueueItem::Emby(Box::new(episode.clone()));
                let parts = item.playback_title_parts(None);
                let (title, subtitle) = match &parts.context {
                    Some(context) => (context.text.clone(), Some(parts.title.text.clone())),
                    None => (parts.title.text.clone(), None),
                };
                let art = self.gui_row_art(emby_row_source(&episode).as_ref());
                GuiShowsRow {
                    sort_letter: sort.is_name_sort().then(|| sort_letter(&title)),
                    id: episode.id,
                    title,
                    subtitle,
                    date: row_date(mbv_ui_model::home_latest::provider_timestamp_secs(&item)),
                    display_only: false,
                    art,
                    media: GuiRowMediaKind::Video,
                }
            })
            .collect()
    }

    /// The Upcoming list's merged flat rows (design D9): every TV library's
    /// premieres, soonest first, with the same labels the old Home section
    /// carried. Every row is display-only.
    fn gui_shows_upcoming_rows(&self) -> Vec<GuiShowsRow> {
        let mut premieres: Vec<&mbv_emby_model::UpcomingPremiere> =
            self.tv_upcoming_snapshots.values().flatten().collect();
        premieres.sort_by_cached_key(|premiere| {
            let secs = mbv_feed::parse_pub_date_secs(&premiere.premiere_date);
            (
                secs.is_none(),
                secs.unwrap_or_default(),
                premiere.stable_key(),
            )
        });
        premieres
            .into_iter()
            .map(|premiere| {
                let secs = mbv_feed::parse_pub_date_secs(&premiere.premiere_date);
                let series = self.gui_shows_series_item(premiere.series_id.as_str());
                let art = self.gui_row_art(Some(&upcoming_row_source(premiere, series.as_ref())));
                GuiShowsRow {
                    id: premiere.stable_key(),
                    title: premiere.series_name.clone(),
                    subtitle: Some(format!(
                        "{} — {}",
                        premiere_number_label(premiere),
                        premiere.title
                    )),
                    date: row_date(secs),
                    sort_letter: None,
                    display_only: true,
                    art,
                    media: GuiRowMediaKind::Video,
                }
            })
            .collect()
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
                    art: self.gui_row_art(emby_row_source(item).as_ref()),
                    media: GuiRowMediaKind::Video,
                }
            })
            .collect()
    }

    /// The series the Shows Hero shows (design D12): the stored selection
    /// while the root list still carries it as a Series, else the root list's
    /// first series row — the Hero default, so the Hero paints as soon as the
    /// series list lands and the detail load can start for it before any
    /// select does. `None` while the root list holds no series.
    pub(in crate::app::gui_library) fn gui_shows_default_series(
        &self,
        lib_idx: usize,
    ) -> Option<String> {
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
        self.gui_tv_shows
            .get(&lib_idx)
            .and_then(|selection| selection.series_id.clone())
            .filter(|stored| in_root(stored))
            .or_else(|| {
                self.libs
                    .get(lib_idx)
                    .and_then(|lib| lib.nav_stack.last())
                    .and_then(|level| level.items.iter().find(|item| item.item_type == "Series"))
                    .map(|item| item.id.clone())
            })
    }

    /// Whether the frame path must start the Shows root load: no root level
    /// yet, or a root whose mode is not the All series list. A root loading as
    /// All is left alone so a repaint never starts a second request, and a
    /// settled All list is left alone — landed rows or not — because the
    /// All series list is exactly what the destination presents. Any other
    /// mode must be reloaded as All, loading or settled, because the mode is
    /// what the destination presents and not the level's progress: the tab's
    /// activation pushes a root with no mode yet and its own browse request in
    /// flight, so a loading root without the All mode would otherwise land a
    /// list the destination never presents. The shared landing path's
    /// default-mode resolution (`maybe_capture_library_total_and_apply_default_pill`)
    /// replaces the first landed root with the Latest episodes list
    /// (`resolve_tv_content_mode`'s any-size Latest default), whose episodes
    /// project no series row and leave the tree blank (the live defect of
    /// 2026-11-18: a launch restored onto Shows painted nothing until a
    /// destination click forced the All list).
    pub(in crate::app::gui_library) fn gui_shows_root_needs_load(&self, lib_idx: usize) -> bool {
        if !self.gui_tv_library(lib_idx) {
            return false;
        }
        if self.libs[lib_idx].nav_stack.is_empty() {
            return true;
        }
        self.libs[lib_idx].tv_content_mode.as_ref() != Some(&TvContentMode::All)
    }

    /// The Shows library's selection, as the TUI's empty selection reads.
    pub(super) fn gui_shows_selection(&self, lib_idx: usize) -> GuiTvShowsSelection {
        self.gui_tv_shows.get(&lib_idx).cloned().unwrap_or_default()
    }

    /// The Emby item a Shows play command names (design D12, D9): a New row
    /// resolves through the merged episodes; an Upcoming row resolves nothing,
    /// because it is display-only; All resolves the series, season, and
    /// Workspace episode rows. A stale command plays nothing.
    pub(in crate::app) fn gui_shows_item(&self, lib_idx: usize, item_id: &str) -> Option<EmbyItem> {
        match self.gui_shows_view(lib_idx) {
            GuiShowsView::New => return self.gui_shows_new_item(item_id),
            GuiShowsView::Upcoming => return None,
            GuiShowsView::All => {}
        }
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

/// The series name a New row leads with, and the key its Title sort compares
/// (design D6): the Service's series name, else the episode's own name for an
/// item that belongs to no series.
fn shows_new_series_name(item: &EmbyItem) -> &str {
    if item.series_name.is_empty() {
        &item.name
    } else {
        &item.series_name
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
