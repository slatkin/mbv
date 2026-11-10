//! The Shows section of the Library frame (add-gui-library-panel design D14,
//! which supersedes D12's first Shows design): the view-mode pills, the
//! active mode's rows — the All series mode's tree, or the flat Latest and
//! Upcoming lists — the core's selected row, and the Hero with its series
//! Workspace. A command names a row, a season, or an episode by the stable
//! Service id the frame carries, never by position.

use mbv::{
    GuiShowsEpisodeRow, GuiShowsHero, GuiShowsMode, GuiShowsPill, GuiShowsProjection, GuiShowsRow,
    GuiShowsSeasonNode, GuiShowsSeasonPill, GuiShowsSeries, GuiShowsWorkspace,
};
use serde::Serialize;

use super::{GuiArtShapeFrame, GuiHomeRowStateFrame, GuiLibraryArt};

/// The Shows destination's content (add-gui-library-panel design D12, the
/// USER-APPROVED GUI Shows design): the view-mode pills, the active mode's
/// rows, the core's selected row, and its Hero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsFrame {
    /// The Shows library index the Shows commands name (design D2, D12):
    /// the library whose content this is.
    pub lib_idx: usize,
    /// The three view-mode pills in the design's order (design D12).
    pub pills: Vec<GuiShowsPillFrame>,
    /// The active mode's name, as the selected collection's name the Library
    /// Status bar's location reads (design D10) — carried once for the whole
    /// panel, as the Music section carries its names.
    pub collection_names: Vec<String>,
    /// Whether the active mode's list has not landed yet.
    pub loading: bool,
    /// Whether the selected series' detail or the selected season's episodes
    /// fetch has not landed yet (the All series mode only).
    pub workspace_loading: bool,
    /// The All series mode's tree (design D14): the series nodes with their
    /// seasons and loaded episodes. Empty in the flat modes, whose list the
    /// `rows` carry.
    pub series: Vec<GuiShowsSeriesFrame>,
    /// Whether the Upcoming mode's eager fetch failed after all its retries:
    /// the list paints the TUI's failure line and the pill is the retry.
    pub upcoming_failed: bool,
    /// The active mode's rows in the list's order (the flat Latest and
    /// Upcoming lists).
    pub rows: Vec<GuiShowsRowFrame>,
    /// The core's selected row (design D2, D12), as the frame painted it:
    /// a select command sends it back. The first row is the Hero default.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` in the Upcoming mode and while
    /// no row is selected.
    pub hero: Option<GuiShowsHeroFrame>,
}

/// One view mode of the Shows destination's pill row (design D12). The
/// TUI's letter-range buckets are not pills here — the All series list's
/// letter rail replaces them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiShowsModeFrame {
    Latest,
    Upcoming,
    All,
}

impl From<GuiShowsMode> for GuiShowsModeFrame {
    fn from(mode: GuiShowsMode) -> Self {
        match mode {
            GuiShowsMode::Latest => Self::Latest,
            GuiShowsMode::Upcoming => Self::Upcoming,
            GuiShowsMode::All => Self::All,
        }
    }
}

/// One view-mode pill of the Shows destination (design D12): single-select,
/// the selected one the core's active mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsPillFrame {
    /// The mode the pill selects: the set-mode command carries it back.
    pub mode: GuiShowsModeFrame,
    /// The pill's label.
    pub name: String,
    /// Whether the core's active mode is this one.
    pub selected: bool,
    /// Whether the pill is a collection pill (multi-select) rather than a
    /// single-select one. Every Shows pill is single-select (design D12).
    pub collection: bool,
}

/// One series node of the All series tree (design D14), the Music artist
/// node's counterpart: the series row, its rail letter, and its seasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsSeriesFrame {
    /// The series' stable Service id: the select command names it, and the
    /// row's each-block key reads it.
    pub id: String,
    /// The series' display name, the row's label.
    pub title: String,
    /// The row's rail letter, or `#`.
    pub sort_letter: char,
    /// Whether the node opens. A series whose detail has not landed counts as
    /// expandable; its open asks the core to load the detail.
    pub expandable: bool,
    /// The series' seasons in the detail's order; empty while the detail has
    /// not landed.
    pub seasons: Vec<GuiShowsSeasonFrame>,
}

/// One season node under a series (design D14), the Music album node's
/// counterpart: the season row and its episodes, or `null` while that
/// season's episode load has not landed — an open season then asks the core to
/// load them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsSeasonFrame {
    /// The season's stable Service id: the select-season command names it.
    pub id: String,
    /// The season's display name.
    pub name: String,
    /// The season's episodes in their fetch order; `null` until the load
    /// lands.
    pub episodes: Option<Vec<GuiShowsEpisodeRowFrame>>,
}

/// One row of the Shows list (design D12): a series row in the All series
/// mode, or a flat episode or premiere row in the Latest and Upcoming modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsRowFrame {
    /// The row's stable identity (design D2, D12): a command names it, the
    /// frame's `selected` carries it, never a row position.
    pub id: String,
    /// The row's primary label.
    pub title: String,
    /// The row's secondary label: the episode's own title on a flat row.
    pub subtitle: Option<String>,
    /// The gutter date on a flat row; `None` on a series row.
    pub date: Option<String>,
    /// The row's rail letter (design D7), or `#`; `None` on the date-sorted
    /// flat rows, whose list shows no rail.
    pub sort_letter: Option<char>,
}

/// The Hero's facts for the selected row (design D12), with the series
/// Hero's Workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsHeroFrame {
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order.
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does.
    pub progress_row: Option<usize>,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves (the artwork policy's declared
    /// arm: landscape for a series and a thumb-declared episode, portrait
    /// for a poster-declared one).
    pub art_shape: GuiArtShapeFrame,
    /// The Hero's artwork, present exactly while the item obtains art from
    /// the image cache; the window paints its own placeholder while `None`.
    pub art: Option<GuiLibraryArt>,
    /// The series Hero's Workspace; `None` on the Latest mode's episode Hero
    /// and while the selected series' detail has not landed.
    pub workspace: Option<GuiShowsWorkspaceFrame>,
}

/// A series Hero's Workspace (design D12): the season pills and the selected
/// season's episodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsWorkspaceFrame {
    /// The season pills in the detail's order.
    pub seasons: Vec<GuiShowsSeasonPillFrame>,
    /// The selected season's id; `None` while the detail holds no season.
    pub selected_season: Option<String>,
    /// The selected season's episodes in their fetch order; empty while
    /// that fetch has not landed, which the frame's `workspaceLoading`
    /// names.
    pub episodes: Vec<GuiShowsEpisodeRowFrame>,
}

/// One season pill of a series' Hero (design D12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsSeasonPillFrame {
    /// The season's stable Service id: the select-season command carries it.
    pub season_id: String,
    /// The pill's label.
    pub name: String,
    /// Whether this is the Workspace's selected season.
    pub selected: bool,
}

/// One episode row of the Hero's Workspace (design D12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiShowsEpisodeRowFrame {
    /// The episode's stable Service id.
    pub episode_id: String,
    /// The number the row paints.
    pub number: i64,
    /// The episode's display name.
    pub name: String,
    /// The runtime, `M:SS` or `H:MM:SS`; empty when unknown.
    pub duration: String,
    /// The played/progress state, the shared media-list row state the Home
    /// rows carry (`GuiHomeRowStateFrame`).
    pub state: GuiHomeRowStateFrame,
}

impl GuiShowsFrame {
    /// The Shows destination's content for `projection` (design D12), with
    /// the library index the Shows commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiShowsProjection) -> Self {
        Self {
            lib_idx,
            pills: projection
                .pills
                .iter()
                .map(GuiShowsPillFrame::from)
                .collect(),
            collection_names: projection.collection_names.clone(),
            loading: projection.loading,
            workspace_loading: projection.workspace_loading,
            series: projection
                .series
                .iter()
                .map(GuiShowsSeriesFrame::from)
                .collect(),
            upcoming_failed: projection.upcoming_failed,
            rows: projection.rows.iter().map(GuiShowsRowFrame::from).collect(),
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiShowsHeroFrame::from),
        }
    }
}

impl From<&GuiShowsSeries> for GuiShowsSeriesFrame {
    fn from(series: &GuiShowsSeries) -> Self {
        Self {
            id: series.id.clone(),
            title: series.title.clone(),
            sort_letter: series.sort_letter,
            expandable: series.expandable,
            seasons: series
                .seasons
                .iter()
                .map(GuiShowsSeasonFrame::from)
                .collect(),
        }
    }
}

impl From<&GuiShowsSeasonNode> for GuiShowsSeasonFrame {
    fn from(season: &GuiShowsSeasonNode) -> Self {
        Self {
            id: season.id.clone(),
            name: season.name.clone(),
            episodes: season
                .episodes
                .as_ref()
                .map(|episodes| episodes.iter().map(GuiShowsEpisodeRowFrame::from).collect()),
        }
    }
}

impl From<&GuiShowsPill> for GuiShowsPillFrame {
    fn from(pill: &GuiShowsPill) -> Self {
        Self {
            mode: GuiShowsModeFrame::from(pill.mode),
            name: pill.name.clone(),
            selected: pill.selected,
            collection: false,
        }
    }
}

impl From<&GuiShowsRow> for GuiShowsRowFrame {
    fn from(row: &GuiShowsRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            date: row.date.clone(),
            sort_letter: row.sort_letter,
        }
    }
}

impl From<&GuiShowsHero> for GuiShowsHeroFrame {
    fn from(hero: &GuiShowsHero) -> Self {
        Self {
            title: hero.title.clone(),
            meta_rows: hero.meta_rows.clone(),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiLibraryArt::from),
            workspace: hero.workspace.as_ref().map(GuiShowsWorkspaceFrame::from),
        }
    }
}

impl From<&GuiShowsWorkspace> for GuiShowsWorkspaceFrame {
    fn from(workspace: &GuiShowsWorkspace) -> Self {
        Self {
            seasons: workspace
                .seasons
                .iter()
                .map(GuiShowsSeasonPillFrame::from)
                .collect(),
            selected_season: workspace.selected_season.clone(),
            episodes: workspace
                .episodes
                .iter()
                .map(GuiShowsEpisodeRowFrame::from)
                .collect(),
        }
    }
}

impl From<&GuiShowsSeasonPill> for GuiShowsSeasonPillFrame {
    fn from(pill: &GuiShowsSeasonPill) -> Self {
        Self {
            season_id: pill.season_id.clone(),
            name: pill.name.clone(),
            selected: pill.selected,
        }
    }
}

impl From<&GuiShowsEpisodeRow> for GuiShowsEpisodeRowFrame {
    fn from(row: &GuiShowsEpisodeRow) -> Self {
        Self {
            episode_id: row.episode_id.clone(),
            number: row.number,
            name: row.name.clone(),
            duration: row.duration.clone(),
            state: GuiHomeRowStateFrame::from(&row.state),
        }
    }
}
