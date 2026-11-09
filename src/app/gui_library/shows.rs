//! The GUI Library's Shows projection (add-gui-library-panel design D12, the
//! USER-APPROVED GUI Shows design): the three view-mode pills, the active
//! mode's rows — the series list with its rail letters, or the flat Latest
//! and Upcoming episode lists — the selected row, and the Hero. A series'
//! Hero is the landscape art, its facts, its season pills, and its Workspace
//! episodes; the Latest mode's Hero is the selected episode's facts; the
//! Upcoming mode paints no Hero at all. The projection reads `App` data
//! only, so the TUI's own projections and paint do not change, and the row
//! and sort rules are the TUI's: series by `SortName`, seasons by `SortName`
//! (the server order the detail fetch requests), episodes by `IndexNumber`.

use mbv_components::library_panel::content::ArtworkSource;
use mbv_components::library_panel::hero::hero_content_emby;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_queue::{QueueItem, TvContentMode};
use mbv_render::components::home_video::format_release_date;
use mbv_render::effective_sort_str;
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::ui_util::{fmt_duration_short, fmt_publish_date_short};

use super::GuiLibraryArtwork;
use super::home::{GuiArtShape, GuiHomeRowState, artwork_shape, media_row_state};
use super::projection::sort_letter;
use super::tabs::GuiDestinationKind;
use super::tv::GuiTvShowsSelection;
use crate::app::App;

/// One view mode of the Shows destination's pill row (design D12): the three
/// single-select pills the design names. The TUI's letter-range buckets are
/// not pills here — the All series list's letter rail replaces them — so a
/// bucket-scoped level paints under the All series pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiShowsMode {
    /// The flat newest-episodes list.
    Latest,
    /// The flat premieres list, with no Hero.
    Upcoming,
    /// The series list with the letter rail.
    All,
}

impl GuiShowsMode {
    /// The pill's label, as the design names the modes.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Latest => "Latest",
            Self::Upcoming => "Upcoming",
            Self::All => "All series",
        }
    }
}

impl From<GuiShowsMode> for TvContentMode {
    fn from(mode: GuiShowsMode) -> Self {
        match mode {
            GuiShowsMode::Latest => Self::Latest,
            GuiShowsMode::Upcoming => Self::Upcoming,
            GuiShowsMode::All => Self::All,
        }
    }
}

/// The pill vocabulary's mode for the active `TvContentMode`: the TUI's
/// letter-range buckets fetch bucket-scoped All series lists, and the GUI's
/// rail replaces the buckets, so a bucket-scoped level paints under the All
/// series pill.
#[must_use]
fn gui_shows_mode_of(mode: &TvContentMode) -> GuiShowsMode {
    match mode {
        TvContentMode::Latest => GuiShowsMode::Latest,
        TvContentMode::Upcoming => GuiShowsMode::Upcoming,
        TvContentMode::All | TvContentMode::Range(_) => GuiShowsMode::All,
    }
}

/// One view-mode pill of the Shows destination (design D12): single-select,
/// the selected one the core's active mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsPill {
    /// The mode the pill selects: the set-mode command carries it back.
    pub mode: GuiShowsMode,
    /// The pill's label.
    pub name: String,
    /// Whether the core's active mode is this one.
    pub selected: bool,
}

/// One row of the Shows list (design D12): a series row in the All series
/// mode, or a flat episode or premiere row in the Latest and Upcoming modes.
/// The identity is the stable Service id a select or play command sends,
/// never a row position; a premiere's identity is the stable key the TUI's
/// premiere rows carry, because a notice is not a media item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsRow {
    /// The row's stable identity.
    pub id: String,
    /// The row's primary label: the series' name in the All series mode, the
    /// show's name on a flat episode row, the series' name on a premiere
    /// row.
    pub title: String,
    /// The row's secondary label: the episode's own title on a flat row.
    pub subtitle: Option<String>,
    /// The gutter date: the provider date on a flat episode row, the
    /// premiere date on a premiere row. `None` on a series row.
    pub date: Option<String>,
    /// The row's rail letter (design D7): the first letter of the
    /// article-stripped sort name, or `#`. `None` on the date-sorted flat
    /// rows, whose list shows no rail.
    pub sort_letter: Option<char>,
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
    pub state: GuiHomeRowState,
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

/// The Hero's facts for the selected row (design D12): the shared Emby hero
/// producer's facts for the series or episode, the artwork at the artwork
/// policy's chain, and the series Hero's Workspace. The Upcoming mode
/// projects no Hero at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiShowsHero {
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order.
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does. The shared
    /// Emby hero producer fills none, so a Shows hero carries `None` exactly
    /// as the TUI's does.
    pub progress_row: Option<usize>,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the artwork policy's declared
    /// shape — landscape for a series and for an episode with a declared
    /// series thumb, portrait for an episode whose poster alone is declared.
    pub art_shape: GuiArtShape,
    /// The Hero's artwork, fetched through the image cache at the artwork
    /// policy's chain. `None` when the item declares no image source; the
    /// window content paints its own placeholder there.
    pub art: Option<GuiLibraryArtwork>,
    /// The series Hero's Workspace; `None` on the Latest mode's episode Hero
    /// and while the selected series' detail has not landed.
    pub workspace: Option<GuiShowsWorkspace>,
}

/// The Shows projection the GUI Library panel paints (design D12): the mode
/// pills, the active mode's rows, the core's selected row, and its Hero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiShowsProjection {
    /// The three view-mode pills in the design's order.
    pub pills: Vec<GuiShowsPill>,
    /// The active mode's name, as the selected collection's name the Library
    /// Status bar's location reads — the Music projection's one-derivation
    /// rule, applied to the mode the Shows list shows.
    pub collection_names: Vec<String>,
    /// Whether the active mode's list has not landed yet.
    pub loading: bool,
    /// Whether the selected series' detail or the selected season's episodes
    /// fetch has not landed yet (the All series mode only). The front end
    /// paints the Workspace's loading state with it.
    pub workspace_loading: bool,
    /// The active mode's rows in the list's order.
    pub rows: Vec<GuiShowsRow>,
    /// The core's selected row (design D2, D12): the row the select command
    /// last named while the list still carries it, else the first row — the
    /// Hero default, so a series Hero paints as soon as the series list
    /// lands and the Latest Hero as soon as the episode list does. `None`
    /// while the list holds no row.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` in the Upcoming mode and while
    /// no row is selected.
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

    /// The Shows projection the GUI Library panel paints (design D12): the
    /// mode pills, the active mode's rows, the selected row, and its Hero.
    /// A `lib_idx` that names no Shows library projects the empty
    /// projection, as the frame's gate excludes that destination anyway.
    #[must_use]
    pub fn gui_shows_projection(&mut self, lib_idx: usize) -> GuiShowsProjection {
        if !self.gui_tv_library(lib_idx) {
            return GuiShowsProjection::default();
        }
        let Some(tv_mode) = self.gui_tv_mode(lib_idx) else {
            return GuiShowsProjection::default();
        };
        let mode = gui_shows_mode_of(&tv_mode);
        let pills = Self::shows_pills(mode);
        let collection_names = vec![mode.label().to_string()];
        let (rows, loading) = self.gui_shows_rows(lib_idx, mode);
        let selected = self.gui_shows_selected(lib_idx, mode, &rows);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.gui_shows_hero(lib_idx, mode, selected));
        let workspace_loading = match (&mode, selected.as_ref()) {
            (GuiShowsMode::All, Some(series_id)) => {
                self.gui_shows_workspace_loading(lib_idx, series_id)
            }
            _ => false,
        };
        GuiShowsProjection {
            pills,
            collection_names,
            loading,
            workspace_loading,
            rows,
            selected,
            hero,
        }
    }

    /// The three view-mode pills (design D12), the selected one the active
    /// mode's.
    fn shows_pills(mode: GuiShowsMode) -> Vec<GuiShowsPill> {
        [
            GuiShowsMode::Latest,
            GuiShowsMode::Upcoming,
            GuiShowsMode::All,
        ]
        .into_iter()
        .map(|pill_mode| GuiShowsPill {
            mode: pill_mode,
            name: pill_mode.label().to_string(),
            selected: pill_mode == mode,
        })
        .collect()
    }

    /// The active mode's rows in the list's order, with the list's loading
    /// state (design D12). The All series mode's rows are the root level's
    /// series in the server's `SortName` order; the flat modes' rows are the
    /// root level's newest episodes and the snapshot's premieres.
    fn gui_shows_rows(&self, lib_idx: usize, mode: GuiShowsMode) -> (Vec<GuiShowsRow>, bool) {
        let level = self.libs.get(lib_idx).and_then(|lib| lib.nav_stack.last());
        match mode {
            GuiShowsMode::All => match level {
                Some(level) => (
                    level
                        .items
                        .iter()
                        .map(|item| GuiShowsRow {
                            id: item.id.clone(),
                            title: item.name.clone(),
                            subtitle: None,
                            date: None,
                            sort_letter: Some(sort_letter(effective_sort_str(item))),
                        })
                        .collect(),
                    level.loading,
                ),
                None => (Vec::new(), true),
            },
            GuiShowsMode::Latest => match level {
                Some(level) => (level.items.iter().map(latest_row).collect(), level.loading),
                None => (Vec::new(), true),
            },
            GuiShowsMode::Upcoming => {
                let library_id = self.libs[lib_idx].library.id.clone();
                let premieres = self
                    .tv_upcoming_snapshots
                    .get(&library_id)
                    .map_or(&[][..], Vec::as_slice);
                (
                    premieres.iter().map(upcoming_row).collect(),
                    level.is_none_or(|level| level.loading),
                )
            }
        }
    }

    /// The core's selected row for the active mode (design D2, D12): the
    /// stored selection while the list still carries it, else the first row.
    /// The All series mode's selection is the series id; the flat modes' is
    /// the flat row.
    fn gui_shows_selected(
        &self,
        lib_idx: usize,
        mode: GuiShowsMode,
        rows: &[GuiShowsRow],
    ) -> Option<String> {
        let stored = match mode {
            GuiShowsMode::All => self.gui_shows_default_series(lib_idx),
            _ => self.gui_shows_selection(lib_idx).flat_row.clone(),
        };
        stored
            .filter(|selected| rows.iter().any(|row| &row.id == selected))
            .or_else(|| rows.first().map(|row| row.id.clone()))
    }

    /// The series the All series mode's Hero shows (design D12): the stored
    /// selection while the root list still carries it as a Series, else the
    /// root list's first series row — the Hero default, so the Hero paints
    /// as soon as the series list lands and the detail load can start for it
    /// before any select does. `None` while the root list holds no series.
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

    /// Start the selected series' detail load when the cache does not hold
    /// it (design D12): the frame path calls this for the All series mode's
    /// destination, so the default first series' Hero paints its season
    /// pills and Workspace without a select, as the Music frame path ensures
    /// its selected group's albums. The fetch deduplicates what is in flight
    /// or cached.
    pub fn ensure_gui_shows_detail(&mut self, lib_idx: usize) {
        let Some(tv_mode) = self.gui_tv_mode(lib_idx) else {
            return;
        };
        if gui_shows_mode_of(&tv_mode) != GuiShowsMode::All {
            return;
        }
        if let Some(series_id) = self.gui_shows_default_series(lib_idx) {
            self.fetch_series_detail(&series_id);
        }
    }

    /// The Shows library's selection, as the TUI's empty selection reads.
    fn gui_shows_selection(&self, lib_idx: usize) -> GuiTvShowsSelection {
        self.gui_tv_shows.get(&lib_idx).cloned().unwrap_or_default()
    }

    /// The selected row's Hero (design D12): the All series mode's series
    /// Hero — the shared Emby hero producer's facts, the artwork policy's
    /// landscape chain, and the Workspace the series detail holds — or the
    /// Latest mode's episode Hero. The Upcoming mode projects no Hero at
    /// all.
    fn gui_shows_hero(
        &mut self,
        lib_idx: usize,
        mode: GuiShowsMode,
        selected: &str,
    ) -> Option<GuiShowsHero> {
        match mode {
            GuiShowsMode::Upcoming => None,
            GuiShowsMode::All => {
                let series = self.gui_shows_item(lib_idx, selected)?;
                let workspace = self.gui_shows_workspace(lib_idx, selected);
                Some(self.gui_shows_hero_facts(&series, workspace))
            }
            GuiShowsMode::Latest => {
                let episode = self.gui_shows_item(lib_idx, selected)?;
                Some(self.gui_shows_hero_facts(&episode, None))
            }
        }
    }

    /// The shared Emby hero producer's facts for one item, with its artwork
    /// fetched through the image cache at the artwork policy's declared
    /// chain — the same facts and fetch the TUI's hero paints.
    fn gui_shows_hero_facts(
        &mut self,
        item: &EmbyItem,
        workspace: Option<GuiShowsWorkspace>,
    ) -> GuiShowsHero {
        let content = hero_content_emby(item);
        let art = match &content.facts.artwork.source {
            Some(ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
            }) => Some(self.gui_home_hero_artwork(cache_key, item_id, series_id, image_types)),
            Some(_) | None => None,
        };
        GuiShowsHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(content.facts.artwork.shape),
            art,
            workspace,
        }
    }

    /// A series Hero's Workspace (design D12): the detail's season pills and
    /// the selected season's episodes. `None` while the series detail has
    /// not landed — the projection's `workspace_loading` names that load.
    fn gui_shows_workspace(&self, lib_idx: usize, series_id: &str) -> Option<GuiShowsWorkspace> {
        let detail = self.series_detail_cache.get(series_id)?;
        let selected_season = self.gui_shows_selected_season(lib_idx, series_id)?;
        let episodes = detail.episodes.get(&selected_season);
        let rows = episodes
            .map(|episodes| {
                episodes
                    .iter()
                    .enumerate()
                    .map(|(position, episode)| GuiShowsEpisodeRow {
                        episode_id: episode.id.clone(),
                        number: episode_number(episode, position),
                        name: episode.name.clone(),
                        duration: if episode.runtime_ticks > 0 {
                            fmt_duration_short(episode.runtime_ticks / TICKS_PER_SECOND)
                        } else {
                            String::new()
                        },
                        state: media_row_state(&MediaSemanticState::from_emby(episode)),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let seasons = detail
            .seasons
            .iter()
            .map(|season| GuiShowsSeasonPill {
                season_id: season.id.clone(),
                name: season.name.clone(),
                selected: selected_season == season.id,
            })
            .collect();
        Some(GuiShowsWorkspace {
            seasons,
            selected_season: Some(selected_season),
            episodes: rows,
        })
    }

    /// The season whose episodes the Workspace shows (design D12): the
    /// stored choice while the detail holds it, else the first season — the
    /// season pills start on the first season and are not kept across a
    /// series change. `None` while the detail holds no season.
    fn gui_shows_selected_season(&self, lib_idx: usize, series_id: &str) -> Option<String> {
        let detail = self.series_detail_cache.get(series_id)?;
        let stored = self.gui_shows_selection(lib_idx).season_id;
        stored
            .filter(|stored| detail.seasons.iter().any(|season| &season.id == stored))
            .or_else(|| detail.seasons.first().map(|season| season.id.clone()))
    }

    /// Whether the Workspace's loads have not landed yet (design D12): the
    /// selected series' detail fetch, or the selected season's episodes
    /// fetch — the in-flight marks the shared fetch helpers keep.
    fn gui_shows_workspace_loading(&self, lib_idx: usize, series_id: &str) -> bool {
        if self.series_detail_loading.contains(series_id) {
            return true;
        }
        self.gui_shows_selected_season(lib_idx, series_id)
            .is_some_and(|season_id| {
                self.series_season_loading
                    .contains(&(series_id.to_string(), season_id))
            })
    }

    /// The Emby item a Shows play command names (design D12): the root
    /// level's rows — the All series mode's series and the Latest mode's
    /// episodes — then the cached series details' episode lists, which the
    /// Workspace's rows name. `None` when no cached row carries the id, so
    /// a stale command plays nothing.
    fn gui_shows_item(&self, lib_idx: usize, item_id: &str) -> Option<EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if let Some(item) = lib
            .nav_stack
            .iter()
            .rev()
            .find_map(|level| level.items.iter().find(|item| item.id == item_id))
        {
            return Some(item.clone());
        }
        self.series_detail_cache
            .values()
            .flat_map(|detail| detail.episodes.values().flatten())
            .find(|episode| episode.id == item_id)
            .cloned()
    }

    /// Select a Shows row (design D12): the All series mode's select is the
    /// series select — the Hero's series, the season reset, and the detail
    /// load — and a flat mode's select stores the row the Hero shows. A
    /// stale frame's row — no row of the active mode's list — is an ignored
    /// command, so a stale select never replaces a good selection.
    pub(in crate::app::gui_library) fn select_gui_shows_row(&mut self, lib_idx: usize, row: &str) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "showsSelect",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        let Some(tv_mode) = self.gui_tv_mode(lib_idx) else {
            Self::log_ignored_library_command(
                "showsSelect",
                format_args!("the destination holds no view mode"),
            );
            return;
        };
        match gui_shows_mode_of(&tv_mode) {
            GuiShowsMode::All => self.gui_tv_select_series(lib_idx, row),
            GuiShowsMode::Latest | GuiShowsMode::Upcoming => {
                if self.gui_shows_flat_row_exists(lib_idx, row) {
                    self.gui_tv_shows.entry(lib_idx).or_default().flat_row = Some(row.to_string());
                } else {
                    Self::log_stale_library_item("showsSelect", row);
                }
            }
        }
    }

    /// Whether the flat mode's list carries `row`: the root level's episode
    /// ids in the Latest mode, the snapshot's premiere keys in the Upcoming
    /// mode.
    fn gui_shows_flat_row_exists(&self, lib_idx: usize, row: &str) -> bool {
        if let Some(level) = self.libs.get(lib_idx).and_then(|lib| lib.nav_stack.last())
            && level.items.iter().any(|item| item.id == row)
        {
            return true;
        }
        let library_id = &self.libs[lib_idx].library.id;
        self.tv_upcoming_snapshots
            .get(library_id)
            .is_some_and(|premieres| {
                premieres
                    .iter()
                    .any(|premiere| premiere.stable_key() == row)
            })
    }

    /// Play the Shows episode a command names (design D12): the shared
    /// library play path, whose admission and route checks are the Owner's
    /// queue's own. A stale id plays nothing.
    pub(in crate::app::gui_library) fn play_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.play_or_activate_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("showsPlay", item_id),
        }
    }

    /// Play the Shows episode next (design D12): the shared play-next
    /// admission, so the episode enters directly after the active slot.
    pub(in crate::app::gui_library) fn play_next_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.play_lib_item_next(lib_idx, item),
            None => Self::log_stale_library_item("showsPlayNext", item_id),
        }
    }

    /// Add the Shows episode to the queue (design D12): the shared add path.
    pub(in crate::app::gui_library) fn enqueue_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.enqueue_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("showsQueue", item_id),
        }
    }
}

/// One flat episode row of the Latest list (design D12): the TUI's flat
/// projection's split title — the show's name leads the episode row — and
/// the provider-date gutter (`crates/mbv-components/src/tv_content/`
/// `episode_rows.rs` `build_latest_episode_rows`).
fn latest_row(episode: &EmbyItem) -> GuiShowsRow {
    let item = QueueItem::Emby(Box::new(episode.clone()));
    let parts = item.playback_title_parts(None);
    let (title, subtitle) = match &parts.context {
        Some(context) => (context.text.clone(), Some(parts.title.text.clone())),
        None => (parts.title.text.clone(), None),
    };
    let date = mbv_ui_model::home_latest::provider_timestamp_secs(&item)
        .map(fmt_publish_date_short)
        .filter(|date| !date.is_empty());
    GuiShowsRow {
        id: episode.id.clone(),
        title,
        subtitle,
        date,
        sort_letter: None,
    }
}

/// One premiere row of the Upcoming list (design D12): the TUI's premiere
/// row's labels — the series' name, then the season/episode number with the
/// episode's title — with the premiere date on the row, as the design's flat
/// premieres list carries it.
fn upcoming_row(premiere: &mbv_emby_model::UpcomingPremiere) -> GuiShowsRow {
    GuiShowsRow {
        id: premiere.stable_key(),
        title: premiere.series_name.clone(),
        subtitle: Some(format!(
            "{} — {}",
            premiere_number_label(premiere),
            premiere.title
        )),
        date: (!premiere.premiere_date.is_empty())
            .then(|| format_release_date(&premiere.premiere_date)),
        sort_letter: None,
    }
}

/// The premiere's season/episode label: `S02:E07`, `S02`, `E07`, or
/// `no number` when the notice carries neither.
fn premiere_number_label(premiere: &mbv_emby_model::UpcomingPremiere) -> String {
    match (premiere.season, premiere.episode) {
        (Some(season), Some(episode)) => format!("S{season:02}:E{episode:02}"),
        (Some(season), None) => format!("S{season:02}"),
        (None, Some(episode)) => format!("E{episode:02}"),
        (None, None) => "no number".into(),
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
