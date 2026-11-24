//! The Home destination's Library content (add-gui-library-panel design D12,
//! the USER-APPROVED GUI Home design): the flat list's rows, the core's
//! selected row, and its Hero. `gui-change-driven-sections` design D8 splits
//! the destination into a list part ([`GuiHomeList`]) and a detail part
//! ([`GuiHomeDetail`]). The `libraryList` and `libraryDetail` sections carry them.

use crate::gui_core::{
    GuiHomeHero, GuiHomeItemRow, GuiHomeProjection, GuiHomeRow, GuiHomeRowId, GuiHomeRowKind,
};
use mbv_queue::QueueItemContentId;
use mbv_ui_model::media_list::{ActiveProgress, MediaSemanticState};

use super::meta::{GuiMetaSegmentFrame, gui_meta_rows};
use super::{GuiArtShapeFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::art::GuiArtRef;

/// The Home destination's list part (design D8): whether the first fetch has
/// landed, and the flat list's rows in paint order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHomeList {
    /// Whether the Home content's first fetch has not landed yet: the list
    /// paints its loading line.
    pub loading: bool,
    /// The flat list's rows in paint order: the two sections' heading rows
    /// and item rows, with the sections and the spacer the projection
    /// builds (a heading only above its rows, empty sections omitted, the
    /// spacer only between two non-empty sections).
    pub rows: Vec<GuiHomeRowFrame>,
}

/// The Home destination's detail part (design D8): the core's selected row and
/// its Hero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHomeDetail {
    /// The core's selected row (design D2, D12), as the window painted it:
    /// a select, play, or enqueue command sends it back.
    pub selected: Option<GuiHomeRowIdFrame>,
    /// The selected row's Hero facts; `None` while no row is selected — an
    /// all-empty list, or the loading list's slot.
    pub hero: Option<GuiHomeHeroFrame>,
}

impl GuiHomeList {
    /// The Home list part for `projection` (design D12).
    pub(crate) fn from_projection(projection: &GuiHomeProjection) -> Self {
        Self {
            loading: projection.loading,
            rows: projection.rows.iter().map(GuiHomeRowFrame::from).collect(),
        }
    }
}

impl GuiHomeDetail {
    /// The Home detail part for `projection` (design D12): the selection and
    /// the Hero's facts beside their artwork.
    pub(crate) fn from_projection(projection: &GuiHomeProjection) -> Self {
        Self {
            selected: projection.selected.as_ref().map(GuiHomeRowIdFrame::from),
            hero: projection.hero.as_ref().map(GuiHomeHeroFrame::from),
        }
    }
}

/// One row of the flat Home list, as the window paints it (design D12):
/// the row tag names the three row kinds the projection builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiHomeRowFrame {
    /// A section heading row (`Continue watching`, `Recently added`).
    Heading {
        /// The section's name, painted as-is.
        text: String,
    },
    /// The gap between the two sections.
    Spacer,
    /// One item row.
    Item(GuiHomeItemRowFrame),
}

/// One item row of the flat Home list (design D12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHomeItemRowFrame {
    /// The row's stable identity (design D2, D12): a command names it, the
    /// section's `selected` carries it, never a row position.
    pub id: GuiHomeRowIdFrame,
    /// The row's primary label (a `Recently added` episode's show, or the
    /// item's own title on an unsplit row).
    pub title: String,
    /// The row's secondary label: the item's own title on a split row;
    /// `None` on an unsplit row.
    pub subtitle: Option<String>,
    /// The row's trailing text (gui-delineated-rows design D2): the time left
    /// on a `Continue watching` row, and the projection's `day month` gutter
    /// date on a `Recently added` one. `None` when the row has neither.
    pub trailing: Option<String>,
    /// The row's kind (`collection` when the row targets a container the
    /// play path refuses, `media` on a playable leaf).
    pub kind: GuiHomeRowKindFrame,
    /// The row's state, with the Continue Watching resume progress.
    pub state: GuiHomeRowStateFrame,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKindFrame,
}

/// One Home row's stable identity, mirroring `crate::gui_core::GuiHomeRowId` (design
/// D12): the identity `App::home_stable_target` resolves. `Recent` carries
/// the queue item's content id, whose wire form is itself (`provider` and
/// `value`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiHomeRowIdFrame {
    /// A `Continue watching` row's Emby item id.
    Continue(String),
    /// A `Recently added` row's queue-item content id.
    Recent(QueueItemContentId),
}

/// The state one item row paints (design D12), from the shared
/// `MediaSemanticState`: a resume state is `active` with the bounded
/// percentage (`playing`'s resume position maps the same way), `played` is
/// `played`, otherwise `ordinary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiHomeRowStateFrame {
    Ordinary,
    Played,
    Active {
        /// The resume progress's bounded percentage, `None` when the
        /// item's runtime is unknown.
        progress: Option<u8>,
    },
}

/// The kind one item row paints (design D12), mirroring
/// `crate::gui_core::GuiHomeRowKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiHomeRowKindFrame {
    Collection,
    Media,
}

/// The Hero's facts for the selected row (design D12): the meta rows with
/// their duration and progress roles, the overview, and the artwork.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiHomeHeroFrame {
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
    /// The artwork box the Hero reserves (the artwork policy's arm).
    pub art_shape: GuiArtShapeFrame,
    /// The Hero's artwork, present exactly while the row's item obtains
    /// art from the image cache; the window paints its own placeholder
    /// while `None`.
    pub art: Option<GuiArtRef>,
}

impl From<&GuiHomeRow> for GuiHomeRowFrame {
    fn from(row: &GuiHomeRow) -> Self {
        match row {
            GuiHomeRow::Heading { text } => Self::Heading { text: text.clone() },
            GuiHomeRow::Spacer => Self::Spacer,
            GuiHomeRow::Item(item) => Self::Item(GuiHomeItemRowFrame::from(item)),
        }
    }
}

impl From<&GuiHomeItemRow> for GuiHomeItemRowFrame {
    fn from(item: &GuiHomeItemRow) -> Self {
        Self {
            id: GuiHomeRowIdFrame::from(&item.id),
            title: item.title.clone(),
            subtitle: item.subtitle.clone(),
            trailing: item.trailing.clone(),
            kind: GuiHomeRowKindFrame::from(item.kind),
            state: GuiHomeRowStateFrame::from(&item.state),
            art: GuiRowArtFrame::from(&item.art),
            media: GuiRowMediaKindFrame::from(item.media),
        }
    }
}

impl From<&GuiHomeRowId> for GuiHomeRowIdFrame {
    fn from(id: &GuiHomeRowId) -> Self {
        match id {
            GuiHomeRowId::Continue(item_id) => Self::Continue(item_id.clone()),
            GuiHomeRowId::Recent(content_id) => Self::Recent(content_id.clone()),
        }
    }
}

impl GuiHomeRowIdFrame {
    /// The id this row takes in a `RowsInView` report (gui-delineated-rows
    /// design D7): the core's own encoding (`GuiHomeRowId::rows_in_view_id`),
    /// which matches the row by an opaque string and never by a position. A
    /// `Continue` watching row carries its Emby item id, and a `Recently added`
    /// row its queue item's content id, provider first.
    #[must_use]
    pub fn rows_in_view_id(&self) -> String {
        crate::gui_core::GuiHomeRowId::from(&crate::bridge::library::GuiHomeRowId::from(self))
            .rows_in_view_id()
    }
}

/// The wire row id a carried row's identity sends (design D12): the same
/// shape on the way out, so a command built from a carried row addresses
/// exactly that row. The owned form is the ref form's delegate, so the
/// variant names are hand-written once.
impl From<&GuiHomeRowIdFrame> for crate::bridge::library::GuiHomeRowId {
    fn from(id: &GuiHomeRowIdFrame) -> Self {
        match id {
            GuiHomeRowIdFrame::Continue(item_id) => Self::Continue(item_id.clone()),
            GuiHomeRowIdFrame::Recent(content_id) => Self::Recent(content_id.clone()),
        }
    }
}

impl From<GuiHomeRowIdFrame> for crate::bridge::library::GuiHomeRowId {
    fn from(id: GuiHomeRowIdFrame) -> Self {
        Self::from(&id)
    }
}

impl From<&MediaSemanticState> for GuiHomeRowStateFrame {
    fn from(state: &MediaSemanticState) -> Self {
        match state {
            MediaSemanticState::Ordinary => Self::Ordinary,
            MediaSemanticState::Played => Self::Played,
            MediaSemanticState::Active { progress }
            | MediaSemanticState::NowPlaying { progress } => Self::Active {
                progress: progress.map(ActiveProgress::percent),
            },
        }
    }
}

impl From<GuiHomeRowKind> for GuiHomeRowKindFrame {
    fn from(kind: GuiHomeRowKind) -> Self {
        match kind {
            GuiHomeRowKind::Collection => Self::Collection,
            GuiHomeRowKind::Media => Self::Media,
        }
    }
}

impl From<&GuiHomeHero> for GuiHomeHeroFrame {
    fn from(hero: &GuiHomeHero) -> Self {
        Self {
            title: hero.title.clone(),
            meta_rows: gui_meta_rows(&hero.meta_rows),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            overview: hero.overview.clone(),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
            art: hero.art.as_ref().map(GuiArtRef::from),
        }
    }
}
