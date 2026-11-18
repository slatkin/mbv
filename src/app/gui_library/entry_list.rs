//! The GUI Library's one collection-scoped entry-list projection (row 7C.4,
//! from the /simplify review of PR 993): the entry row and projection frame
//! the Feeds (row 11.1) and Podcasts (row 10.1) destinations share, the
//! row-state arm both of their row builders apply, and the select-by-id arm the
//! Audiobooks destination (row 9.1) shares with them. Each destination's own
//! module keeps what is truly its own — the collection catalogs, the row
//! builder, the Hero, and the loads — so the share carries only the vocabulary
//! every list paints and the fallback rules every list applies. Audiobooks has
//! no collection selector (its letter rail replaces the TUI's surname buckets)
//! and a book row of its own fields, so it takes the selection arm only.

use mbv_ui_model::media_list::MediaSemanticState;

use super::collection::{GuiCollectionNoun, GuiCollectionSet, GuiCollectionTarget, GuiCollections};

/// One row of a collection-scoped destination's entry list (rows 10.1,
/// 11.1). The identity is the entry's stable Service identity, encoded as one
/// opaque string a select, play, or enqueue command sends, never a row
/// position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEntryRow {
    /// The entry identity, as the destination's id encoding writes it.
    pub id: String,
    /// The entry's display title, the row's primary label.
    pub title: String,
    /// The row's secondary label — the fact that names the row's scope (the
    /// subscription, the show) in the merged Latest view.
    pub subtitle: Option<String>,
    /// The gutter date: the TUI row's fixed right-aligned `day month` text
    /// (`fmt_publish_date_short` over the publish instant). `None` on an
    /// undated entry, and `Some("")` never rides here — the TUI's rows drop
    /// an empty gutter too.
    pub date: Option<String>,
    /// The row's played state, the shared media-list semantic state its
    /// provider's constructor derives: `Played` for an item Play would not
    /// resume, `Active` with the resume percent exactly when Play would
    /// resume a saved position, otherwise `Ordinary`.
    pub state: MediaSemanticState,
}

/// The collection-scoped entry-list projection a destination paints (rows 10.1,
/// 11.1): the destination's collection selector, the selection's entry rows,
/// the core's selected entry, and its Hero. No letter rail rides here — the
/// lists are sorted by date — and no Workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEntryListProjection<H> {
    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): All, every scope in the destination's order with its checked
    /// flag, the destination's noun, and its date order.
    pub collections: GuiCollections,
    /// Whether the active view's content is still landing: the feed tab's
    /// fetch for Feeds, the show catalog's pages and the active view's own
    /// episode fetches for Podcasts.
    pub loading: bool,
    /// Whether the active view's fetch failed: always `false` for Feeds — the
    /// tab keeps no failure state, a failed subscription flashes the TUI's
    /// toast and keeps its previous entries — and the browse state's error
    /// for Podcasts, whose published episodes keep serving when they can.
    pub failed: bool,
    /// The selection's entry rows: All merges every scope's entries, and a
    /// checked set holds the checked scopes' — in the destination's date
    /// order.
    pub rows: Vec<GuiEntryRow>,
    /// The core's selected entry (rows 10.1, 11.1), as the frame painted it:
    /// the pick the select command last named while the active list still
    /// carries it, else that list's first row — the Hero default, so the Hero
    /// paints as soon as a list lands. `None` while the list holds no row.
    pub selected: Option<String>,
    /// The selected entry's Hero facts; `None` while no entry is selected.
    pub hero: Option<H>,
}

impl<H> Default for GuiEntryListProjection<H> {
    /// The empty projection: All with no collections. The Feeds target is the
    /// one value the shared shape can carry — only a destination whose frame
    /// gate excludes it (a non-podcast library, a Feeds tab with no
    /// subscriptions) paints it, and no painter then reads the selector.
    fn default() -> Self {
        Self {
            collections: GuiCollections::new(
                GuiCollectionTarget::Feeds,
                &GuiCollectionSet::All,
                GuiCollectionNoun::Subscriptions,
                std::iter::empty::<(String, String)>(),
                None,
            ),
            loading: false,
            failed: false,
            rows: Vec::new(),
            selected: None,
            hero: None,
        }
    }
}

/// The row the Hero shows (rows 9.1, 10.1, 11.1): the core's pick while the
/// list still carries it, else the list's first row — the Hero default, so
/// the Hero paints as soon as a list lands. `None` while the list holds no
/// row. `listed` is the list's membership test over a row id, and `first`
/// yields the list's first row's id.
pub(in crate::app::gui_library) fn picked_or_first(
    picked: Option<String>,
    listed: impl Fn(&str) -> bool,
    first: impl FnOnce() -> Option<String>,
) -> Option<String> {
    picked.filter(|picked| listed(picked)).or_else(first)
}
