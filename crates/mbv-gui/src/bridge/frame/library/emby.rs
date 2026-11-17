//! The plain Emby-library section of the Library frame (add-gui-library-panel
//! design D12, row 8.1): the feed-group folder selector, the active list's
//! rows, the core's selected row, and the selected row's Hero with no
//! Workspace. A command names a row or a folder by the stable Service id the
//! frame carries, never by position.

use mbv::{GuiEmbyHero, GuiEmbyHeroCredit, GuiEmbyProjection, GuiEmbyRow, GuiEmbyRowKind};

use super::GuiArtShapeFrame;
use super::collections::GuiCollectionsFrame;
use super::meta::{GuiMetaSegmentFrame, gui_meta_rows};
use crate::art::GuiArtRef;

/// The plain Emby-library destination's list part (row 8.1; design D8): the
/// feed-group folder selector, the active list's rows, and the load facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEmbyList {
    /// The plain Emby-library index the Emby commands name (design D2, row
    /// 8.1): the library whose content this is.
    pub lib_idx: usize,
    /// The library's display name: the list header's title, so the column
    /// names the library it holds.
    pub library_name: String,
    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): `Some` in the feed-group mode, with All, every root folder in
    /// listing order, the Folders noun, and the list's date order; `None` in
    /// plain mode, whose letter rail replaces it.
    pub collections: Option<GuiCollectionsFrame>,
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
}

/// The plain Emby-library destination's detail part (`gui-change-driven-sections`
/// design D8): the core's selected row and its Hero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEmbyDetail {
    /// The core's selected row (design D2, row 8.1), as the frame painted
    /// it: a select command sends it back. The first row is the Hero
    /// default.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` while no row is selected.
    pub hero: Option<GuiEmbyHeroFrame>,
}

/// One row's kind (row 8.1), mirroring `mbv::GuiEmbyRowKind`: `collection`
/// when the row is a folder or a boxset the folder-play rule expands,
/// `media` on a playable leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiEmbyRowKindFrame {
    Collection,
    Media,
}

/// One row of the plain Emby-library list (row 8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEmbyHeroFrame {
    /// The item's kind word for the Hero's kicker (design D17): `movie`,
    /// `video`, as the projection carries it.
    pub kind: String,
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order.
    pub meta_rows: Vec<Vec<GuiMetaSegmentFrame>>,
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
    pub art: Option<GuiArtRef>,
    /// The movie's credits table (design D17, Q7): the director first, then
    /// the cast, as the TUI's credits table paints them.
    pub credits: Vec<GuiEmbyHeroCreditFrame>,
    /// The `IMDb` link's target (design D17, Q7), when the item carries one;
    /// the `IMDb` meta row links to it.
    pub imdb_url: Option<String>,
}

/// One credit of the Hero's credits table (design D17, Q7), mirroring
/// `mbv::GuiEmbyHeroCredit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEmbyHeroCreditFrame {
    /// The person's name.
    pub name: String,
    /// The role painted beside the name: the person's role, else their
    /// kind (a director's `Director`).
    pub role: String,
}

impl GuiEmbyList {
    /// The plain Emby-library list part for `projection` (row 8.1), with the
    /// library index the Emby commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiEmbyProjection) -> Self {
        Self {
            lib_idx,
            library_name: projection.library_name.clone(),
            collections: projection
                .collections
                .as_ref()
                .map(GuiCollectionsFrame::from),
            collection_names: projection.collection_names.clone(),
            feed_group: projection.feed_group,
            loading: projection.loading,
            failed: projection.failed,
            rows: projection.rows.iter().map(GuiEmbyRowFrame::from).collect(),
        }
    }
}

impl GuiEmbyDetail {
    /// The plain Emby-library detail part for `projection` (row 8.1).
    pub(crate) fn from_projection(projection: &GuiEmbyProjection) -> Self {
        Self {
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiEmbyHeroFrame::from),
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
            kind: hero.kind.clone(),
            title: hero.title.clone(),
            meta_rows: gui_meta_rows(&hero.meta_rows),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiArtRef::from),
            credits: hero
                .credits
                .iter()
                .map(GuiEmbyHeroCreditFrame::from)
                .collect(),
            imdb_url: hero.imdb_url.clone(),
        }
    }
}

impl From<&GuiEmbyHeroCredit> for GuiEmbyHeroCreditFrame {
    fn from(credit: &GuiEmbyHeroCredit) -> Self {
        Self {
            name: credit.name.clone(),
            role: credit.role.clone(),
        }
    }
}
