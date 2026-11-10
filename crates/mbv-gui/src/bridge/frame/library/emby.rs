//! The plain Emby-library section of the Library frame (add-gui-library-panel
//! design D12, row 8.1): the feed-group pills, the active list's rows, the
//! core's selected row, and the selected row's Hero with no Workspace. A
//! command names a row or a folder by the stable Service id the frame
//! carries, never by position.

use mbv::{
    GuiEmbyFeedGroup, GuiEmbyHero, GuiEmbyPill, GuiEmbyProjection, GuiEmbyRow, GuiEmbyRowKind,
};
use serde::Serialize;

use super::{GuiArtShapeFrame, GuiLibraryArt};

/// The plain Emby-library destination's content (row 8.1), present while a
/// plain Emby library is the active destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiEmbyFrame {
    /// The plain Emby-library index the Emby commands name (design D2, row
    /// 8.1): the library whose content this is.
    pub lib_idx: usize,
    /// The library's display name: the list header's title, so the column
    /// names the library it holds.
    pub library_name: String,
    /// The feed-group mode's pills in pill order; empty in plain mode, whose
    /// letter rail replaces the TUI's pill row.
    pub pills: Vec<GuiEmbyPillFrame>,
    /// The selected collection's name the Library Status bar's location
    /// reads (design D10); empty in plain mode, whose list is the whole
    /// library.
    pub collection_names: Vec<String>,
    /// Whether the library presents the feed-group mode (`feed_view_libraries`).
    pub feed_group: bool,
    /// Whether the active list has not landed yet.
    pub loading: bool,
    /// Whether the plain mode's root-list fetch failed.
    pub failed: bool,
    /// The active list's rows in the list's order.
    pub rows: Vec<GuiEmbyRowFrame>,
    /// The core's selected row (design D2, row 8.1), as the frame painted
    /// it: a select command sends it back. The first row is the Hero
    /// default.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` while no row is selected.
    pub hero: Option<GuiEmbyHeroFrame>,
}

/// The feed-group pill a press selects (row 8.1): `New` (the destination
/// Latest view), `All`, or one root folder by its stable Service id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiEmbyFeedGroupFrame {
    /// The New pill: the library's destination-Latest snapshot, newest first.
    Latest,
    /// The All pill: the library root's own listing, newest first.
    All,
    /// One root folder that holds items, by its stable Service id.
    Folder(String),
}

impl From<GuiEmbyFeedGroup> for GuiEmbyFeedGroupFrame {
    fn from(group: GuiEmbyFeedGroup) -> Self {
        match group {
            GuiEmbyFeedGroup::Latest => Self::Latest,
            GuiEmbyFeedGroup::All => Self::All,
            GuiEmbyFeedGroup::Folder(folder_id) => Self::Folder(folder_id),
        }
    }
}

/// One pill of a feed-view library's pill row (row 8.1): single-select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiEmbyPillFrame {
    /// The group the pill selects: the set-feed-group command carries it.
    pub group: GuiEmbyFeedGroupFrame,
    /// The pill's label: `All`, or the folder's name.
    pub name: String,
    /// Whether the feed state's selection is this pill's group.
    pub selected: bool,
    /// Whether the pill is a collection pill (multi-select) rather than a
    /// single-select one. Every Emby-library pill is single-select (design
    /// D6).
    pub collection: bool,
}

/// One row's kind (row 8.1), mirroring `mbv::GuiEmbyRowKind`: `collection`
/// when the row is a folder or a boxset the folder-play rule expands,
/// `media` on a playable leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiEmbyRowKindFrame {
    Collection,
    Media,
}

/// One row of the plain Emby-library list (row 8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiEmbyRowFrame {
    /// The row's stable identity (design D2, row 8.1): a command names it,
    /// the frame's `selected` carries it, never a row position.
    pub id: String,
    /// The row's primary label, with a folder's item-count decoration.
    pub title: String,
    /// The row's secondary label (plain mode): the year, else the runtime.
    pub subtitle: Option<String>,
    /// The gutter date (feed-group mode): the production year.
    pub date: Option<String>,
    /// The row's rail letter (design D7), or `#`; set only in plain mode.
    pub sort_letter: Option<char>,
    /// The row's kind.
    pub kind: GuiEmbyRowKindFrame,
}

/// The Hero's facts for the selected row (row 8.1), with no Workspace: a
/// plain Emby-library Hero carries none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiEmbyHeroFrame {
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
    /// arm).
    pub art_shape: GuiArtShapeFrame,
    /// The Hero's artwork, present exactly while the item obtains art from
    /// the image cache; the window paints its own placeholder while `None`.
    pub art: Option<GuiLibraryArt>,
}

impl GuiEmbyFrame {
    /// The plain Emby-library destination's content for `projection` (row
    /// 8.1), with the library index the Emby commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiEmbyProjection) -> Self {
        Self {
            lib_idx,
            library_name: projection.library_name.clone(),
            pills: projection
                .pills
                .iter()
                .map(GuiEmbyPillFrame::from)
                .collect(),
            collection_names: projection.collection_names.clone(),
            feed_group: projection.feed_group,
            loading: projection.loading,
            failed: projection.failed,
            rows: projection.rows.iter().map(GuiEmbyRowFrame::from).collect(),
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiEmbyHeroFrame::from),
        }
    }
}

impl From<&GuiEmbyPill> for GuiEmbyPillFrame {
    fn from(pill: &GuiEmbyPill) -> Self {
        Self {
            group: GuiEmbyFeedGroupFrame::from(pill.group.clone()),
            name: pill.name.clone(),
            selected: pill.selected,
            collection: false,
        }
    }
}

impl From<&GuiEmbyRow> for GuiEmbyRowFrame {
    fn from(row: &GuiEmbyRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            date: row.date.clone(),
            sort_letter: row.sort_letter,
            kind: GuiEmbyRowKindFrame::from(row.kind),
        }
    }
}

impl From<GuiEmbyRowKind> for GuiEmbyRowKindFrame {
    fn from(kind: GuiEmbyRowKind) -> Self {
        match kind {
            GuiEmbyRowKind::Collection => Self::Collection,
            GuiEmbyRowKind::Media => Self::Media,
        }
    }
}

impl From<&GuiEmbyHero> for GuiEmbyHeroFrame {
    fn from(hero: &GuiEmbyHero) -> Self {
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
