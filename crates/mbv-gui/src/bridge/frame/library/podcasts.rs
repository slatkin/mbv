//! The Podcasts section of the Library frame (add-gui-library-panel design
//! D12, row 10.1): the single-select pill row of Latest and one pill per show,
//! the active pill's episode rows in the date gutter, the core's selected
//! episode, and the Hero with the parent show's Square cover. A command names
//! an episode by the opaque identity the row carries — the provider's
//! `(library_item_id, episode_id)` pair encoded in one place — never by
//! position.

use mbv::{GuiPodcastsHero, GuiPodcastsPillOption, GuiPodcastsProjection, GuiPodcastsRow};
use serde::Serialize;

use super::{GuiArtShapeFrame, GuiHomeRowStateFrame, GuiLibraryArt};
use crate::bridge::library::GuiPodcastPill;

/// The Podcasts destination's content (add-gui-library-panel design D12, row
/// 10.1): the pill row, the active pill's episode rows, the core's selected
/// episode, and its Hero. No letter rail rides here — the list is sorted by
/// date — and no Workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPodcastsFrame {
    /// The Podcasts library index the Podcasts commands name (design D2, D12):
    /// the library whose content this is.
    pub lib_idx: usize,
    /// The pill row in paint order: Latest, then each show by title.
    pub pills: Vec<GuiPodcastsPillFrame>,
    /// Whether the active view's episodes are still landing: the show
    /// catalog's pages or that view's own episode fetches.
    pub loading: bool,
    /// Whether the catalog fetch failed.
    pub failed: bool,
    /// The active pill's episode rows: Latest merges every listed show's
    /// episodes, a show pill holds that one show's, both newest first with
    /// undated episodes last.
    pub rows: Vec<GuiPodcastsRowFrame>,
    /// The core's selected episode (design D2, D12), as the frame painted it:
    /// a select, play, or enqueue command sends it back. The first row is the
    /// Hero default.
    pub selected: Option<String>,
    /// The selected episode's Hero facts; `None` while no episode is selected.
    pub hero: Option<GuiPodcastsHeroFrame>,
}

/// One pill of the Podcasts pill row (row 10.1): single-select, as the TUI's
/// one-valued `PillSelection` is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPodcastsPillFrame {
    /// The pill's selection: the set-pill command carries it back.
    pub pill: GuiPodcastPill,
    /// The pill's label: `Latest`, or the show's title.
    pub name: String,
    /// Whether the core's stored pill is this one.
    pub selected: bool,
    /// Whether the pill is a collection pill (multi-select) rather than a
    /// single-select one. Every Podcasts pill is single-select.
    pub collection: bool,
}

/// One row of the Podcasts list (row 10.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPodcastsRowFrame {
    /// The episode's stable identity (design D2, D12): the provider's
    /// `(library_item_id, episode_id)` pair as one opaque string
    /// (`GuiPodcastEpisodeId::encode`), which the play, queue, and select
    /// commands send back. Never a row position.
    pub id: String,
    /// The episode's display title.
    pub title: String,
    /// The parent show's title.
    pub subtitle: Option<String>,
    /// The gutter date, `day month`; `None` on an undated episode.
    pub date: Option<String>,
    /// The row's played state, with the resume percent on a partly played
    /// episode.
    pub state: GuiHomeRowStateFrame,
}

/// The selected episode's Hero facts (row 10.1): the shared
/// `hero_content_abs_episode` producer's facts, the cleaned description, and
/// the parent show's Square cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPodcastsHeroFrame {
    /// The hero's title: the episode title.
    pub title: String,
    /// The meta rows in display order: the show title, the duration, and the
    /// publish date.
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role; `None` on every episode, as
    /// the shared producer fills none.
    pub progress_row: Option<usize>,
    /// The cleaned description text, when the episode carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the episode artwork policy's
    /// Square.
    pub art_shape: GuiArtShapeFrame,
    /// The parent show's cover, present exactly while the episode obtains art
    /// from the image cache; the window paints its own placeholder while
    /// `None`.
    pub art: Option<GuiLibraryArt>,
}

impl GuiPodcastsFrame {
    /// The Podcasts destination's content for `projection` (row 10.1), with
    /// the library index the Podcasts commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiPodcastsProjection) -> Self {
        Self {
            lib_idx,
            pills: projection
                .pills
                .iter()
                .map(GuiPodcastsPillFrame::from)
                .collect(),
            loading: projection.loading,
            failed: projection.failed,
            rows: projection
                .rows
                .iter()
                .map(GuiPodcastsRowFrame::from)
                .collect(),
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiPodcastsHeroFrame::from),
        }
    }
}

impl From<&GuiPodcastsPillOption> for GuiPodcastsPillFrame {
    fn from(pill: &GuiPodcastsPillOption) -> Self {
        Self {
            pill: GuiPodcastPill::from(&pill.pill),
            name: pill.name.clone(),
            selected: pill.selected,
            collection: false,
        }
    }
}

impl From<&GuiPodcastsRow> for GuiPodcastsRowFrame {
    fn from(row: &GuiPodcastsRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            date: row.date.clone(),
            state: GuiHomeRowStateFrame::from(&row.state),
        }
    }
}

impl From<&GuiPodcastsHero> for GuiPodcastsHeroFrame {
    fn from(hero: &GuiPodcastsHero) -> Self {
        Self {
            title: hero.title.clone(),
            meta_rows: hero.meta_rows.clone(),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiLibraryArt::from),
        }
    }
}
