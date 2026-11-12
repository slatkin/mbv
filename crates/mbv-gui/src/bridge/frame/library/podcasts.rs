//! The Podcasts section of the Library frame (add-gui-library-panel design
//! D12, row 10.1): the single-select pill row of Latest and one pill per show,
//! the active pill's episode rows in the date gutter, the core's selected
//! episode, and the Hero with the parent show's Square cover. A command names
//! an episode by the opaque identity the row carries — the provider's
//! `(library_item_id, episode_id)` pair encoded in one place — never by
//! position. The pill and row frames are the shared entry-list ones
//! (`entry_list.rs`); the Hero and the frame struct are the destination's own.

use mbv::{GuiPodcastsHero, GuiPodcastsProjection};
use serde::Serialize;

use super::GuiArtShapeFrame;
use super::entry_list::{GuiEntryRowFrame, GuiPillFrame};
use super::meta::{GuiMetaSegmentFrame, gui_meta_rows};
use crate::art::GuiArtRef;
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
    pub pills: Vec<GuiPillFrame<GuiPodcastPill>>,
    /// Whether the active view's episodes are still landing: the show
    /// catalog's pages or that view's own episode fetches.
    pub loading: bool,
    /// Whether the catalog fetch failed.
    pub failed: bool,
    /// The active pill's episode rows: Latest merges every listed show's
    /// episodes, a show pill holds that one show's, both newest first with
    /// undated episodes last.
    pub rows: Vec<GuiEntryRowFrame>,
    /// The core's selected episode (design D2, D12), as the frame painted it:
    /// a select, play, or enqueue command sends it back. The first row is the
    /// Hero default.
    pub selected: Option<String>,
    /// The selected episode's Hero facts; `None` while no episode is selected.
    pub hero: Option<GuiPodcastsHeroFrame>,
}

/// The selected episode's Hero facts (row 10.1): the shared
/// `hero_content_abs_episode` producer's facts, the cleaned description, and
/// the parent show's Square cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPodcastsHeroFrame {
    /// The hero's title: the episode title.
    pub title: String,
    /// The name of the largest unit the item belongs to; see the projection.
    pub parent: Option<String>,
    /// The meta rows in display order: the show title, the duration, and the
    /// publish date.
    pub meta_rows: Vec<Vec<GuiMetaSegmentFrame>>,
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
    pub art: Option<GuiArtRef>,
}

impl GuiPodcastsFrame {
    /// The Podcasts destination's content for `projection` (row 10.1), with
    /// the library index the Podcasts commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiPodcastsProjection) -> Self {
        Self {
            lib_idx,
            pills: projection.pills.iter().map(GuiPillFrame::from).collect(),
            loading: projection.loading,
            failed: projection.failed,
            rows: projection.rows.iter().map(GuiEntryRowFrame::from).collect(),
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiPodcastsHeroFrame::from),
        }
    }
}

impl From<&GuiPodcastsHero> for GuiPodcastsHeroFrame {
    fn from(hero: &GuiPodcastsHero) -> Self {
        Self {
            title: hero.title.clone(),
            parent: hero.parent.clone(),
            meta_rows: gui_meta_rows(&hero.meta_rows),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiArtRef::from),
        }
    }
}
