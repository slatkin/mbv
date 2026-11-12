//! The Home destination's Library content (add-gui-library-panel design D12,
//! the USER-APPROVED GUI Home design): the flat list's rows, the core's
//! selected row, and its Hero. `gui-change-driven-sections` design D8 splits
//! the destination into a list part ([`GuiHomeList`]) and a detail part
//! ([`GuiHomeDetail`]). [`GuiHomeFrame`] composes both for the current
//! `library` section until the Library sections replace it (task 6.3).

use mbv::{
    GuiHomeHero, GuiHomeItemRow, GuiHomeProjection, GuiHomeRow, GuiHomeRowId, GuiHomeRowKind,
    GuiHomeRowState,
};
use mbv_queue::QueueItemContentId;
use serde::Serialize;

use super::GuiArtShapeFrame;
use super::meta::{GuiMetaSegmentFrame, gui_meta_rows};
use crate::art::GuiArtRef;

/// The Home destination's list part (design D8): whether the first fetch has
/// landed, and the flat list's rows in paint order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiHomeDetail {
    /// The core's selected row (design D2, D12), as the window painted it:
    /// a select, play, or enqueue command sends it back.
    pub selected: Option<GuiHomeRowIdFrame>,
    /// The selected row's Hero facts; `None` while no row is selected — an
    /// all-empty list, or the loading list's slot.
    pub hero: Option<GuiHomeHeroFrame>,
}

/// The Home destination's content as the current `library` section carries it
/// (design D8): the list part and the detail part, flattened so the wire keys
/// stay the ones the window reads today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiHomeFrame {
    /// The list part (design D8).
    #[serde(flatten)]
    pub list: GuiHomeList,
    /// The detail part (design D8).
    #[serde(flatten)]
    pub detail: GuiHomeDetail,
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

impl GuiHomeFrame {
    /// The Home content for `projection` (design D12): the rows, the
    /// selection, and the Hero's facts beside their artwork.
    pub(crate) fn from_projection(projection: &GuiHomeProjection) -> Self {
        Self {
            list: GuiHomeList::from_projection(projection),
            detail: GuiHomeDetail::from_projection(projection),
        }
    }
}

/// One row of the flat Home list, as the window paints it (design D12):
/// the row tag names the three row kinds the projection builds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "row", rename_all = "camelCase")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// The gutter date, the shared projection's `day month` text; `None` on
    /// a `Continue watching` row and on an item with no parseable date.
    pub date: Option<String>,
    /// The row's kind (`collection` when the row targets a container the
    /// play path refuses, `media` on a playable leaf).
    pub kind: GuiHomeRowKindFrame,
    /// The row's state, with the Continue Watching resume progress.
    pub state: GuiHomeRowStateFrame,
    /// The row's artwork, present exactly while the row's item obtains art
    /// from the image cache.
    pub art: Option<GuiArtRef>,
}

/// One Home row's stable identity, mirroring `mbv::GuiHomeRowId` (design
/// D12): the identity `App::home_stable_target` resolves. `Recent` carries
/// the queue item's content id, whose serde form is itself (`provider` and
/// `value`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiHomeRowIdFrame {
    /// A `Continue watching` row's Emby item id.
    Continue(String),
    /// A `Recently added` row's queue-item content id.
    Recent(QueueItemContentId),
}

/// The state one item row paints (design D12), mirroring
/// `mbv::GuiHomeRowState`: `playing`'s resume position is `active` with the
/// bounded percentage, `played` without one is `played`, otherwise
/// `ordinary`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
/// `mbv::GuiHomeRowKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiHomeRowKindFrame {
    Collection,
    Media,
}

/// The Hero's facts for the selected row (design D12): the meta rows with
/// their duration and progress roles, the overview, and the artwork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
            date: item.date.clone(),
            kind: GuiHomeRowKindFrame::from(item.kind),
            state: GuiHomeRowStateFrame::from(&item.state),
            art: item.art.as_ref().map(GuiArtRef::from),
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

impl From<&GuiHomeRowState> for GuiHomeRowStateFrame {
    fn from(state: &GuiHomeRowState) -> Self {
        match state {
            GuiHomeRowState::Ordinary => Self::Ordinary,
            GuiHomeRowState::Played => Self::Played,
            GuiHomeRowState::Active { progress } => Self::Active {
                progress: *progress,
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
