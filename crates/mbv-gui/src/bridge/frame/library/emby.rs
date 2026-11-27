//! The plain Emby-library section of the Library frame (add-gui-library-panel
//! design D12, row 8.1): the feed-group folder selector, the active list's
//! rows, the core's selected row, and the selected row's Hero with no
//! Workspace. A command names a row or a folder by the stable Service id the
//! frame carries, never by position.

use crate::gui_core::{
    GuiCollections, GuiEmbyHero, GuiEmbyProjection, GuiEmbyRow, GuiEmbyRowKind, GuiLibraryView,
    GuiListSortFrame,
};

use super::favourites::GuiFavouritesList;
use super::hero_facts::{GuiHeroCastMemberFrame, GuiHeroFactsFrame};
use super::{GuiArtShapeFrame, GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
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
    /// listing order, and the Folders noun; `None` in plain mode, whose letter
    /// rail replaces it.
    pub collections: Option<GuiCollections>,
    /// The selected collection's name the Library Status bar's location
    /// reads (design D10); empty in plain mode, whose list is the whole
    /// library.
    pub collection_names: Vec<String>,
    /// Whether the library presents the feed-group mode (`feed_view_libraries`).
    pub feed_group: bool,
    /// The destination's one active view (emby-favourites design D5): the
    /// header's view dropdown marks it, and the Library Status bar reads it.
    pub view: GuiLibraryView,
    /// Whether the active list has not landed yet.
    pub loading: bool,
    /// Whether the plain mode's root-list fetch failed.
    pub failed: bool,
    /// The active list's rows in the list's order.
    pub rows: Vec<GuiEmbyRowFrame>,
    /// The active list's sort control (gui-flat-list-sort design D7): the
    /// target a press names, its active sort, and the choices the library
    /// offers. The plain mode and the feed-group mode share it.
    pub sort: Option<GuiListSortFrame>,
    /// The library's Favourites list (emby-favourites design D6): `Some` while
    /// the destination's view is Favourites, and `None` otherwise, whose All
    /// list paints.
    pub favourites: Option<GuiFavouritesList>,
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

/// One row's kind (row 8.1), mirroring `crate::gui_core::GuiEmbyRowKind`: `collection`
/// when the row is a folder or a boxset the folder-play rule expands,
/// `media` on a playable leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiEmbyRowKindFrame {
    Collection,
    Media,
}

/// One row of the plain Emby-library list (row 8.1). The row carries the
/// shared media-list row state (`GuiHomeRowStateFrame`), through the one
/// conversion the Home rows already use (gui-emby-row-played-marks design
/// D2): no second frame state type exists.
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
    /// The row's played/progress state, the shared media-list row state the
    /// Home rows carry (`GuiHomeRowStateFrame`).
    pub state: GuiHomeRowStateFrame,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKindFrame,
}

/// The Hero's facts for the selected row (row 8.1), with no Workspace: a
/// plain Emby-library Hero carries none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEmbyHeroFrame {
    /// The hero's title.
    pub title: String,
    /// The item's facts (gui-hero-design-polish design D1): the header
    /// band's pills and the detail block's labelled rows, each in paint
    /// order.
    pub facts: GuiHeroFactsFrame,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves (the artwork policy's declared
    /// arm).
    pub art_shape: GuiArtShapeFrame,
    /// The Hero's artwork, present exactly while the item obtains art from
    /// the image cache; the window paints its own placeholder while `None`.
    pub art: Option<GuiArtRef>,
    /// The movie's cast (gui-hero-design-polish design D3): the people that
    /// are not directors, in the order Emby gives. Empty when the item is
    /// not a movie or carries no people.
    pub cast: Vec<GuiHeroCastMemberFrame>,
}

impl GuiEmbyList {
    /// The plain Emby-library list part for `projection` (row 8.1), with the
    /// library index the Emby commands name.
    pub(crate) fn from_projection(lib_idx: usize, projection: &GuiEmbyProjection) -> Self {
        Self {
            lib_idx,
            library_name: projection.library_name.clone(),
            collections: projection.collections.clone(),
            collection_names: projection.collection_names.clone(),
            feed_group: projection.feed_group,
            view: projection.view,
            loading: projection.loading,
            failed: projection.failed,
            rows: projection.rows.iter().map(GuiEmbyRowFrame::from).collect(),
            sort: projection.sort.clone(),
            favourites: projection
                .favourites
                .as_ref()
                .map(GuiFavouritesList::from_projection),
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
            state: GuiHomeRowStateFrame::from(&row.state),
            art: GuiRowArtFrame::from(&row.art),
            media: GuiRowMediaKindFrame::from(row.media),
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
            facts: GuiHeroFactsFrame::from(&hero.facts),
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiArtRef::from),
            cast: hero.cast.iter().map(GuiHeroCastMemberFrame::from).collect(),
        }
    }
}
