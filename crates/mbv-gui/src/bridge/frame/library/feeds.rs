//! The Feeds section of the Library frame (add-gui-library-panel design D12,
//! row 11.1): the single-select pill row of Latest and one pill per
//! subscription, the active pill's entry rows in the date gutter, the core's
//! selected entry, and the Hero with the placeholder artwork's shape. A
//! command names an entry by the opaque identity the row carries — the
//! `(subscription url, guid)` pair encoded in one place — never by position,
//! because two subscriptions can publish the same `guid`.

use mbv::{GuiFeedsHero, GuiFeedsPillOption, GuiFeedsProjection, GuiFeedsRow};
use serde::Serialize;

use super::{GuiArtShapeFrame, GuiHomeRowStateFrame};
use crate::bridge::library::GuiFeedPill;

/// The Feeds destination's content (add-gui-library-panel design D12, row
/// 11.1): the pill row, the active pill's entry rows, the core's selected
/// entry, and its Hero. No letter rail rides here — the list is sorted by
/// date — no state filter, and no Workspace. A Client holds one feed tab, so
/// no library index names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFeedsFrame {
    /// The pill row in paint order: Latest, then each configured subscription
    /// by its name.
    pub pills: Vec<GuiFeedsPillFrame>,
    /// Whether the tab's fetch results are still landing.
    pub loading: bool,
    /// Whether a fetch failed. The feed tab keeps no failure state — a failed
    /// subscription flashes the TUI's toast and keeps its previous entries —
    /// so this is always `false`.
    pub failed: bool,
    /// The active pill's entry rows: Latest merges every subscription's
    /// entries, a subscription pill holds that one subscription's, both newest
    /// first with undated entries last.
    pub rows: Vec<GuiFeedsRowFrame>,
    /// The core's selected entry (design D2, D12), as the frame painted it: a
    /// select, play, or enqueue command sends it back. The first row is the
    /// Hero default.
    pub selected: Option<String>,
    /// The selected entry's Hero facts; `None` while no entry is selected.
    pub hero: Option<GuiFeedsHeroFrame>,
}

/// One pill of the Feeds pill row (row 11.1): single-select, as the TUI
/// owner's one-valued group selection is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFeedsPillFrame {
    /// The pill's selection: the set-pill command carries it back.
    pub pill: GuiFeedPill,
    /// The pill's label: `Latest`, or the subscription's name.
    pub name: String,
    /// Whether the core's stored pill is this one.
    pub selected: bool,
    /// Whether the pill is a collection pill (multi-select) rather than a
    /// single-select one. Every Feeds pill is single-select.
    pub collection: bool,
}

/// One row of the Feeds list (row 11.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFeedsRowFrame {
    /// The entry's stable identity (design D2, D12): the entry's
    /// `(subscription url, guid)` pair as one opaque string
    /// (`GuiFeedEntryId::encode`), which the play, queue, and select commands
    /// send back. Never a row position.
    pub id: String,
    /// The entry's display title.
    pub title: String,
    /// The subscription's name; `None` on an entry whose subscription the
    /// tab no longer holds.
    pub subtitle: Option<String>,
    /// The gutter date, `day month`; `None` on an undated entry.
    pub date: Option<String>,
    /// The row's played state, with the resume percent on a partly played
    /// entry.
    pub state: GuiHomeRowStateFrame,
}

/// The selected entry's Hero facts (row 11.1): the shared
/// `hero_content_feed` producer's title and duration behind the subscription
/// name, and the placeholder artwork's shape. A feed entry has no artwork
/// source and no overview, so neither rides here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFeedsHeroFrame {
    /// The hero's title: the entry title.
    pub title: String,
    /// The name of the largest unit the item belongs to; see the projection.
    pub parent: Option<String>,
    /// The meta rows in display order: the duration when the entry
    /// declares one (the subscription name is the parent).
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role; `None` on every entry, as the
    /// shared producer fills none.
    pub progress_row: Option<usize>,
    /// The artwork box the Hero reserves: Square for a podcast feed, Landscape
    /// otherwise. The Hero paints its placeholder, because a feed entry has no
    /// artwork source.
    pub art_shape: GuiArtShapeFrame,
}

impl GuiFeedsFrame {
    /// The Feeds destination's content for `projection` (row 11.1).
    pub(crate) fn from_projection(projection: &GuiFeedsProjection) -> Self {
        Self {
            pills: projection
                .pills
                .iter()
                .map(GuiFeedsPillFrame::from)
                .collect(),
            loading: projection.loading,
            failed: projection.failed,
            rows: projection.rows.iter().map(GuiFeedsRowFrame::from).collect(),
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiFeedsHeroFrame::from),
        }
    }
}

impl From<&GuiFeedsPillOption> for GuiFeedsPillFrame {
    fn from(pill: &GuiFeedsPillOption) -> Self {
        Self {
            pill: GuiFeedPill::from(&pill.pill),
            name: pill.name.clone(),
            selected: pill.selected,
            collection: false,
        }
    }
}

impl From<&GuiFeedsRow> for GuiFeedsRowFrame {
    fn from(row: &GuiFeedsRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            date: row.date.clone(),
            state: GuiHomeRowStateFrame::from(&row.state),
        }
    }
}

impl From<&GuiFeedsHero> for GuiFeedsHeroFrame {
    fn from(hero: &GuiFeedsHero) -> Self {
        Self {
            title: hero.title.clone(),
            parent: hero.parent.clone(),
            meta_rows: hero.meta_rows.clone(),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
        }
    }
}
