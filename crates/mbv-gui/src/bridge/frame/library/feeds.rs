//! The Feeds section of the Library frame (add-gui-library-panel design D12,
//! row 11.1): the subscription selector, the selected subscriptions' entry
//! rows in the date gutter, the core's selected entry, and the Hero with the
//! placeholder artwork's shape. A command names an entry by the opaque identity
//! the row carries — the `(subscription url, guid)` pair encoded in one place —
//! never by position, because two subscriptions can publish the same `guid`.
//! The row frame is the shared entry-list one (`entry_list.rs`); the Hero and
//! the frame struct are the destination's own.

use mbv::{GuiFeedsHero, GuiFeedsProjection};

use super::GuiArtShapeFrame;
use super::collections::GuiCollectionsFrame;
use super::entry_list::GuiEntryRowFrame;
use super::meta::{GuiMetaSegmentFrame, gui_meta_rows};

/// The Feeds destination's list part (add-gui-library-panel design D12, row
/// 11.1; `gui-change-driven-sections` design D8): the subscription selector and
/// the selected subscriptions' entry rows, with the load facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFeedsList {
    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): All, every configured subscription in the tab's order, the
    /// Subscriptions noun, and the list's date order.
    pub collections: Option<GuiCollectionsFrame>,
    /// Whether the tab's fetch results are still landing.
    pub loading: bool,
    /// Whether a fetch failed. The feed tab keeps no failure state — a failed
    /// subscription flashes the TUI's toast and keeps its previous entries —
    /// so this is always `false`.
    pub failed: bool,
    /// The selected subscriptions' entry rows (design D1, D2): All merges
    /// every subscription's entries, a checked set merges the checked
    /// subscriptions', both in the stored date order with undated entries
    /// last.
    pub rows: Vec<GuiEntryRowFrame>,
}

/// The Feeds destination's detail part (`gui-change-driven-sections` design
/// D8): the core's selected entry and its Hero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFeedsDetail {
    /// The core's selected entry (design D2, D12), as the frame painted it: a
    /// select, play, or enqueue command sends it back. The first row is the
    /// Hero default.
    pub selected: Option<String>,
    /// The selected entry's Hero facts; `None` while no entry is selected.
    pub hero: Option<GuiFeedsHeroFrame>,
}

/// The selected entry's Hero facts (row 11.1): the shared
/// `hero_content_feed` producer's title and duration behind the subscription
/// name, and the placeholder artwork's shape. A feed entry has no artwork
/// source and no overview, so neither rides here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFeedsHeroFrame {
    /// The hero's title: the entry title.
    pub title: String,
    /// The name of the largest unit the item belongs to; see the projection.
    pub parent: Option<String>,
    /// The meta rows in display order: the duration when the entry
    /// declares one (the subscription name is the parent).
    pub meta_rows: Vec<Vec<GuiMetaSegmentFrame>>,
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

impl GuiFeedsList {
    /// The Feeds list part for `projection` (row 11.1).
    pub(crate) fn from_projection(projection: &GuiFeedsProjection) -> Self {
        Self {
            collections: projection
                .collections
                .as_ref()
                .map(GuiCollectionsFrame::from),
            loading: projection.loading,
            failed: projection.failed,
            rows: projection.rows.iter().map(GuiEntryRowFrame::from).collect(),
        }
    }
}

impl GuiFeedsDetail {
    /// The Feeds detail part for `projection` (row 11.1).
    pub(crate) fn from_projection(projection: &GuiFeedsProjection) -> Self {
        Self {
            selected: projection.selected.clone(),
            hero: projection.hero.as_ref().map(GuiFeedsHeroFrame::from),
        }
    }
}

impl From<&GuiFeedsHero> for GuiFeedsHeroFrame {
    fn from(hero: &GuiFeedsHero) -> Self {
        Self {
            title: hero.title.clone(),
            parent: hero.parent.clone(),
            meta_rows: gui_meta_rows(&hero.meta_rows),
            duration_row: hero.duration_row,
            progress_row: hero.progress_row,
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
        }
    }
}
