//! The Feeds section of the Library frame (add-gui-library-panel design D12,
//! row 11.1): the subscription selector, the selected subscriptions' entry
//! rows in the date gutter, the core's selected entry, and the Hero with the
//! placeholder artwork's shape. A command names an entry by the opaque identity
//! the row carries — the `(subscription url, guid)` pair encoded in one place —
//! never by position, because two subscriptions can publish the same `guid`.
//! The row frame is the shared entry-list one (`entry_list.rs`); the Hero and
//! the frame struct are the destination's own.

use crate::gui_core::{GuiCollections, GuiFeedsHero, GuiFeedsProjection, GuiListSortFrame};
use mbv_feed::FeedPlatform;

use super::GuiArtShapeFrame;
use super::entry_list::GuiEntryRowFrame;
use super::hero_facts::GuiHeroFactsFrame;

/// The Feeds destination's list part (add-gui-library-panel design D12, row
/// 11.1; `gui-change-driven-sections` design D8): the subscription selector and
/// the selected subscriptions' entry rows, with the load facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFeedsList {
    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): All, every configured subscription in the tab's order, and the
    /// Subscriptions noun.
    pub collections: GuiCollections,
    /// Whether the tab's fetch results are still landing.
    pub loading: bool,
    /// Whether a fetch failed. The feed tab keeps no failure state — a failed
    /// subscription flashes the TUI's toast and keeps its previous entries —
    /// so this is always `false`.
    pub failed: bool,
    /// The Feed platform every checked subscription shares
    /// (gui-library-header-service-marks design D3): the header's Feed mark,
    /// and `None` for All, for a mixed or generic check set, and for an empty
    /// one, where the header keeps the RSS mark.
    pub platform: Option<FeedPlatform>,
    /// The selected subscriptions' entry rows (design D1, D2): All merges
    /// every subscription's entries, a checked set merges the checked
    /// subscriptions', both in the stored date order with undated entries
    /// last.
    pub rows: Vec<GuiEntryRowFrame>,
    /// The list's sort control (gui-flat-list-sort design D7): the target a
    /// press names, its active sort, and the choices the tab offers.
    pub sort: Option<GuiListSortFrame>,
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
    /// The feed entry playing at the playhead, in the row id form; the row
    /// whose id it equals is the playing row.
    pub playing_entry_id: Option<String>,
}

/// The selected entry's Hero facts (row 11.1; gui-hero-design-polish design
/// D1): the D1 feed builder's facts — the duration pill over the Published
/// row (when the entry has a date) — and the placeholder artwork's shape. A
/// feed entry has no artwork source and no overview, so neither rides here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiFeedsHeroFrame {
    /// The hero's title: the entry title.
    pub title: String,
    /// The name of the largest unit the item belongs to; see the projection.
    pub parent: Option<String>,
    /// The item's facts (gui-hero-design-polish design D1): the header
    /// band's pills and the detail block's labelled rows, each in paint
    /// order.
    pub facts: GuiHeroFactsFrame,
    /// The artwork box the Hero reserves: Square for a podcast feed, Landscape
    /// otherwise. The Hero paints its placeholder, because a feed entry has no
    /// artwork source.
    pub art_shape: GuiArtShapeFrame,
}

impl GuiFeedsList {
    /// The Feeds list part for `projection` (row 11.1).
    pub(crate) fn from_projection(projection: &GuiFeedsProjection) -> Self {
        Self {
            collections: projection.list.collections.clone(),
            loading: projection.list.loading,
            failed: projection.list.failed,
            platform: projection.platform,
            rows: projection
                .list
                .rows
                .iter()
                .map(GuiEntryRowFrame::from)
                .collect(),
            sort: projection.list.sort.clone(),
        }
    }
}

impl GuiFeedsDetail {
    /// The Feeds detail part for `projection` (row 11.1).
    pub(crate) fn from_projection(projection: &GuiFeedsProjection) -> Self {
        Self {
            selected: projection.list.selected.clone(),
            hero: projection.list.hero.as_ref().map(GuiFeedsHeroFrame::from),
            playing_entry_id: projection.playing_entry_id.clone(),
        }
    }
}

impl From<&GuiFeedsHero> for GuiFeedsHeroFrame {
    fn from(hero: &GuiFeedsHero) -> Self {
        Self {
            title: hero.title.clone(),
            parent: hero.parent.clone(),
            facts: GuiHeroFactsFrame::from(&hero.facts),
            art_shape: GuiArtShapeFrame::from(hero.art_shape),
        }
    }
}
