//! The Audiobooks section of the Library frame (add-gui-library-panel design
//! D12, row 9.1): the flat book list with its rail letters, the core's
//! selected book, and the Hero with its chapters Workspace. A command names a
//! book by its stable `library_item_id` and a chapter by the Service
//! discriminator the Workspace row carries, never by position.

use mbv::{
    GuiAudiobooksChapterRow, GuiAudiobooksHero, GuiAudiobooksProjection, GuiAudiobooksRow,
    GuiAudiobooksWorkspace,
};
use serde::Serialize;

use super::GuiArtShapeFrame;
use super::meta::{GuiMetaSegmentFrame, gui_meta_rows};
use crate::art::GuiArtRef;

/// The Audiobooks destination's content (add-gui-library-panel design D12,
/// row 9.1): the book list, the core's selected book, and its Hero. No pill
/// rides here — the letter rail replaces the TUI's surname buckets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAudiobooksFrame {
    /// The Audiobooks library index the Audiobooks commands name (design
    /// D2, D12): the library whose content this is.
    pub lib_idx: usize,
    /// Whether the book catalog has not landed yet.
    pub loading: bool,
    /// Whether the catalog fetch failed.
    pub failed: bool,
    /// Whether the selected book's detail fetch has not landed yet: the
    /// Workspace's loading state.
    pub workspace_loading: bool,
    /// The catalog's books in the TUI's sort order.
    pub rows: Vec<GuiAudiobooksRowFrame>,
    /// The core's selected book (design D2, D12), as the frame painted it:
    /// a select command sends it back. The first book is the Hero default.
    pub selected: Option<String>,
    /// The Audiobooks book playing at the playhead (`gui-change-driven-sections`
    /// design D5): its stable `library_item_id`, the fact the window compares
    /// with the Hero's book before it marks a chapter row from the position.
    /// `None` while no book plays or the active slot is pending.
    pub playing_book_id: Option<String>,
    /// The selected book's Hero facts; `None` while no book is selected.
    pub hero: Option<GuiAudiobooksHeroFrame>,
}

/// One row of the book list (row 9.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAudiobooksRowFrame {
    /// The book's stable `library_item_id` (design D2, D12): a command names
    /// it, the frame's `selected` carries it, never a row position.
    pub id: String,
    /// The book's display title.
    pub title: String,
    /// The author display credit.
    pub subtitle: Option<String>,
    /// The book's total duration, the TUI row's duration gutter; empty when
    /// unknown.
    pub duration: String,
    /// The row's rail letter, or `#` for a digit- or symbol-led author sort
    /// key.
    pub sort_letter: char,
}

/// The book Hero's facts (row 9.1), with the chapters Workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAudiobooksHeroFrame {
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order: the author, the duration, and the
    /// `Finished` or `N%` progress row.
    pub meta_rows: Vec<Vec<GuiMetaSegmentFrame>>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does.
    pub progress_row: Option<usize>,
    /// The cleaned description, when the catalog record carries one: the
    /// overview the TUI's Hero paints.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the book artwork policy's
    /// portrait.
    pub art_shape: GuiArtShapeFrame,
    /// The Hero's cover artwork, present exactly while the book obtains art
    /// from the image cache; the window paints its own placeholder while
    /// `None`.
    pub art: Option<GuiArtRef>,
    /// The book's chapter rows; `None` while the book's detail has not
    /// landed, which the frame's `workspaceLoading` names.
    pub workspace: Option<GuiAudiobooksWorkspaceFrame>,
}

/// The book Hero's Workspace (row 9.1): the book's chapter rows — or its
/// `Part N` rows when the book holds no chapters — in the TUI's
/// `visible_rows` order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAudiobooksWorkspaceFrame {
    /// The chapter rows in the TUI's `visible_rows` order.
    pub rows: Vec<GuiAudiobooksChapterRowFrame>,
}

/// One chapter row of the Hero's Workspace (row 9.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAudiobooksChapterRowFrame {
    /// The row's stable Service discriminator: the chapter number on a
    /// chapter row, the audio-part index on a `Part N` row. A chapter
    /// command carries it, never a display position.
    pub chapter: usize,
    /// The row's label: the chapter's title, or `Part N`.
    pub title: String,
    /// The row's duration, the TUI's duration gutter; empty when unknown.
    pub duration: String,
    /// The chapter's start on the book's merged timeline, in ticks (design
    /// D5). An audio-part row carries no span — the window marks the row
    /// whose span holds the playhead position, and an empty span never
    /// holds one.
    pub start_ticks: i64,
    /// The chapter's end on the book's merged timeline, in ticks (design D5).
    pub end_ticks: i64,
}

impl GuiAudiobooksFrame {
    /// The Audiobooks destination's content for `projection` (row 9.1), with
    /// the library index the Audiobooks commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiAudiobooksProjection) -> Self {
        Self {
            lib_idx,
            loading: projection.loading,
            failed: projection.failed,
            workspace_loading: projection.workspace_loading,
            rows: projection
                .rows
                .iter()
                .map(GuiAudiobooksRowFrame::from)
                .collect(),
            selected: projection.selected.clone(),
            playing_book_id: projection.playing_book_id.clone(),
            hero: projection.hero.as_ref().map(GuiAudiobooksHeroFrame::from),
        }
    }
}

impl From<&GuiAudiobooksRow> for GuiAudiobooksRowFrame {
    fn from(row: &GuiAudiobooksRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            duration: row.duration.clone(),
            sort_letter: row.sort_letter,
        }
    }
}

impl From<&GuiAudiobooksHero> for GuiAudiobooksHeroFrame {
    fn from(hero: &GuiAudiobooksHero) -> Self {
        Self {
            title: hero.title.clone(),
            meta_rows: gui_meta_rows(&hero.meta_rows),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiArtRef::from),
            workspace: hero
                .workspace
                .as_ref()
                .map(GuiAudiobooksWorkspaceFrame::from),
        }
    }
}

impl From<&GuiAudiobooksWorkspace> for GuiAudiobooksWorkspaceFrame {
    fn from(workspace: &GuiAudiobooksWorkspace) -> Self {
        Self {
            rows: workspace
                .rows
                .iter()
                .map(GuiAudiobooksChapterRowFrame::from)
                .collect(),
        }
    }
}

impl From<&GuiAudiobooksChapterRow> for GuiAudiobooksChapterRowFrame {
    fn from(row: &GuiAudiobooksChapterRow) -> Self {
        Self {
            chapter: row.chapter,
            title: row.title.clone(),
            duration: row.duration.clone(),
            start_ticks: row.start_ticks,
            end_ticks: row.end_ticks,
        }
    }
}
