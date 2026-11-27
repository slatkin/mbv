//! The Shows section of the Library frame (add-gui-library-panel design D14;
//! gui-collection-dropdown design D8): the series list's tree, the core's
//! selected series, and the Hero with its series Workspace. The GUI Shows
//! destination paints the series list alone, and a command names a row, a
//! season, or an episode by the stable Service id the frame carries, never by
//! position.

use crate::gui_core::{
    GuiLibraryView, GuiListSortFrame, GuiShowsEpisodeRow, GuiShowsHero, GuiShowsProjection,
    GuiShowsRow, GuiShowsSeasonNode, GuiShowsSeasonPill, GuiShowsSeries, GuiShowsWorkspace,
};

use super::favourites::GuiFavouritesList;
use super::hero_facts::GuiHeroFactsFrame;
use super::{GuiArtShapeFrame, GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::art::GuiArtRef;

/// The Shows destination's list part (add-gui-library-panel design D12, the
/// USER-APPROVED GUI Shows design; `gui-change-driven-sections` design D8;
/// gui-collection-dropdown design D8): the series list's tree and the load
/// facts. The GUI Shows destination paints the series list alone, so it carries
/// no collection selector (design D6) and no view-mode pills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiShowsList {
    /// The Shows library index the Shows commands name (design D2, D12):
    /// the library whose content this is.
    pub lib_idx: usize,
    /// The one active view (design D8; emby-favourites design D5): the
    /// destination choice. The header
    /// paints the matching single-choice dropdown.
    pub view: GuiLibraryView,
    /// Whether the active list has not landed yet.
    pub loading: bool,
    /// The series list's tree (design D14): the series nodes with their
    /// seasons and loaded episodes.
    pub series: Vec<GuiShowsSeriesFrame>,
    /// Whether the Upcoming mode's eager fetch failed after all its retries.
    /// The GUI Shows destination paints no Upcoming mode, so the core clears it
    /// until the collection-dropdown units remove the wire field.
    pub upcoming_failed: bool,
    /// The flat Latest and Upcoming rows, which the core leaves empty while the
    /// GUI Shows destination paints the series list alone (design D8).
    pub rows: Vec<GuiShowsRowFrame>,
    /// The New list's sort control (gui-flat-list-sort design D7): the target a
    /// press names, its active sort, and the choices the library offers.
    /// `Some` only while the view is New, whose list is the one the sort
    /// orders.
    pub sort: Option<GuiListSortFrame>,
    /// The library's Favourites list (emby-favourites design D6): `Some` while
    /// the view is Favourites, and `None` otherwise, whose All tree paints.
    pub favourites: Option<GuiFavouritesList>,
}

/// The Shows destination's detail part (`gui-change-driven-sections` design
/// D8): the core's selected row, its Hero, and the Workspace's load fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiShowsDetail {
    /// Whether the selected series' detail or the selected season's episodes
    /// fetch has not landed yet.
    pub workspace_loading: bool,
    /// The core's selected row (design D2, D12), as the frame painted it:
    /// a select command sends it back. The first row is the Hero default.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` in the Upcoming mode and while
    /// no row is selected.
    pub hero: Option<GuiShowsHeroFrame>,
    /// The Emby episode playing at the playhead (design D6), by its stable
    /// Service id; the Workspace row with this id is the playing row.
    pub playing_episode_id: Option<String>,
}

/// One series node of the All series tree (design D14), the Music artist
/// node's counterpart: the series row, its rail letter, and its seasons.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKindFrame,
}

/// One season node under a series (design D14), the Music album node's
/// counterpart: the season row and its episodes, or `null` while that
/// season's episode load has not landed — an open season then asks the core to
/// load them.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// flat rows, which paint under no heading and no rail
    pub sort_letter: Option<char>,
    /// Whether the row takes no select, play, or enqueue (design D9): an
    /// Upcoming premiere is display-only.
    pub display_only: bool,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKindFrame,
}

/// The Hero's facts for the selected row (design D12), with the series
/// Hero's Workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiShowsHeroFrame {
    /// The hero's title.
    pub title: String,
    /// The name of the largest unit the item belongs to; see the projection.
    pub parent: Option<String>,
    /// The item's facts (gui-hero-design-polish design D1): the header
    /// band's pills and the detail block's labelled rows, each in paint
    /// order.
    pub facts: GuiHeroFactsFrame,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves (the artwork policy's declared
    /// arm: landscape for a series and a thumb-declared episode, portrait
    /// for a poster-declared one).
    pub art_shape: GuiArtShapeFrame,
    /// The Hero's artwork, present exactly while the item obtains art from
    /// the image cache; the window paints its own placeholder while `None`.
    pub art: Option<GuiArtRef>,
    /// The series Hero's Workspace; `None` on the Latest mode's episode Hero
    /// and while the selected series' detail has not landed.
    pub workspace: Option<GuiShowsWorkspaceFrame>,
}

/// A series Hero's Workspace (design D12): the season pills and the selected
/// season's episodes.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiShowsSeasonPillFrame {
    /// The season's stable Service id: the select-season command carries it.
    pub season_id: String,
    /// The pill's label.
    pub name: String,
    /// Whether this is the Workspace's selected season.
    pub selected: bool,
}

/// One episode row of the Hero's Workspace (design D12).
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl GuiShowsList {
    /// The Shows list part for `projection`, with the library index the Shows
    /// commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiShowsProjection) -> Self {
        Self {
            lib_idx,
            view: projection.view,
            loading: projection.loading,
            rows: projection.rows.iter().map(GuiShowsRowFrame::from).collect(),
            sort: projection.sort.clone(),
            series: projection
                .series
                .iter()
                .map(GuiShowsSeriesFrame::from)
                .collect(),
            upcoming_failed: false,
            favourites: projection
                .favourites
                .as_ref()
                .map(GuiFavouritesList::from_projection),
        }
    }
}

impl GuiShowsDetail {
    /// The Shows detail part for `projection`: the selection and the Hero with
    /// its series Workspace.
    pub(crate) fn from_projection(projection: &GuiShowsProjection) -> Self {
        Self {
            workspace_loading: projection.workspace_loading,
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiShowsHeroFrame::from),
            playing_episode_id: projection.playing_episode_id.clone(),
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
            display_only: row.display_only,
            art: GuiRowArtFrame::from(&row.art),
            media: GuiRowMediaKindFrame::from(row.media),
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
            art: GuiRowArtFrame::from(&series.art),
            media: GuiRowMediaKindFrame::from(series.media),
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

impl From<&GuiShowsHero> for GuiShowsHeroFrame {
    fn from(hero: &GuiShowsHero) -> Self {
        Self {
            title: hero.title.clone(),
            parent: hero.parent.clone(),
            facts: GuiHeroFactsFrame::from(&hero.facts),
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiArtRef::from),
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
