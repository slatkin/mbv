//! The GUI Library's Feeds projection (add-gui-library-panel design D12,
//! row 11.1): the single-select pill row of Latest and one pill per
//! subscription, the active pill's entry list in the TUI's date order —
//! newest first with undated entries last — the core's selected entry, and
//! its Hero. Latest merges every subscription's entries; a subscription pill
//! scopes the list to that one subscription. The list shows no letter rail,
//! because the sorted-by-date list has one, and the TUI's Watched filter is
//! dropped. The Hero is the shared `hero_content_feed` producer's facts — the
//! entry title with the duration — behind the subscription name, with the
//! placeholder artwork only: a feed entry has no artwork source, so the frame
//! carries the shape and no image. The order is the TUI's own
//! (`FeedTabState::rebuild_all_entries`), so the two Clients cannot drift;
//! the projection reads `App` data only.

use mbv_components::library_panel::hero::hero_content_feed;
use mbv_queue::FeedEntry;
use mbv_ui_model::home_latest::provider_timestamp_secs;
use mbv_ui_model::ui_util::fmt_publish_date_short;

use super::GuiFeedPill;
use super::entry_list::{
    GuiEntryListProjection, GuiEntryRow, GuiPillOption, picked_or_first, pill_options,
};
use super::home::{GuiArtShape, GuiHomeRowState, artwork_shape};
use crate::app::App;
use mbv_ui_model::meta::MetaSegment;

/// One entry's stable identity across every list view and every
/// subscription (row 11.1): the subscription's URL — the feed identity the
/// fetch stamps on each entry and the state store keys on — with the entry's
/// own `guid`, because two subscriptions can publish the same `guid`. The
/// frame carries it as one opaque string, in the form
/// [`GuiFeedEntryId::encode`] writes and [`GuiFeedEntryId::decode`] reads, so
/// a command built from the frame addresses the entry that frame painted or
/// nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiFeedEntryId {
    feed_id: String,
    guid: String,
}

impl GuiFeedEntryId {
    /// The identity of the entry `guid` of the subscription `feed_id`.
    #[must_use]
    pub fn new(feed_id: String, guid: String) -> Self {
        Self { feed_id, guid }
    }

    /// The subscription's URL, empty on an entry that carries no subscription
    /// identity (a legacy snapshot the tab never fetches).
    #[must_use]
    pub fn feed_id(&self) -> &str {
        &self.feed_id
    }

    /// The entry's provider-native `guid`.
    #[must_use]
    pub fn guid(&self) -> &str {
        &self.guid
    }

    /// The row id the frame carries: the byte length of the subscription URL,
    /// a `:`, then the URL and the entry's `guid` run together. The length
    /// prefix is what makes the pair one unambiguous string whatever the two
    /// ids hold — a separator would collide with an id that contains it — and
    /// the decode is its only reader. An empty subscription URL is legal,
    /// because an entry the fetch stamped no identity on is still one row.
    #[must_use]
    pub fn encode(&self) -> String {
        format!("{}:{}{}", self.feed_id.len(), self.feed_id, self.guid)
    }

    /// The identity the frame's row id names, or `None` when the string is no
    /// encoding of this form: a length that is not a number, a length past the
    /// end, or an empty `guid`. Every command arm treats `None` as a stale
    /// frame's id and changes nothing.
    #[must_use]
    pub fn decode(encoded: &str) -> Option<Self> {
        let (length, rest) = encoded.split_once(':')?;
        let length: usize = length.parse().ok()?;
        let feed_id = rest.get(..length)?;
        let guid = rest.get(length..)?;
        if guid.is_empty() {
            return None;
        }
        Some(Self {
            feed_id: feed_id.to_string(),
            guid: guid.to_string(),
        })
    }
}

/// One pill of the Feeds pill row (row 11.1): Latest first, then one pill per
/// configured subscription in the tab's order — the shared pill option over
/// the Feeds pill. The pill row is single-select, as the TUI owner's
/// one-valued group selection is; the state pills are dropped.
pub type GuiFeedsPillOption = GuiPillOption<GuiFeedPill>;

/// One row of the Feeds list (row 11.1) — the shared pill-scoped entry-list
/// row. The identity is the entry's `(subscription url, guid)` pair, encoded
/// as one opaque string a select, play, or enqueue command sends, never a row
/// position.
pub type GuiFeedsRow = GuiEntryRow;

/// The selected entry's Hero facts (row 11.1): the shared `hero_content_feed`
/// producer's title and duration, the subscription name ahead of them, and the
/// placeholder artwork's shape. A feed entry has no artwork source and no
/// overview, so neither rides here, and Feeds has no Workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiFeedsHero {
    /// The hero's title: the entry title.
    pub title: String,
    /// The name of the largest unit the item belongs to (the invariant 30 header
    /// title rule): the show, the feed, or the series. `None` when the item is
    /// its own largest unit. The meta rows never repeat it.
    pub parent: Option<String>,
    /// The meta rows in display order: the duration when the entry
    /// declares one (the subscription name is the parent).
    pub meta_rows: Vec<Vec<MetaSegment>>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role. The shared producer fills
    /// none for a feed entry, so this is `None` exactly as the TUI's is.
    pub progress_row: Option<usize>,
    /// The artwork box the Hero reserves: Square for a podcast feed
    /// (`FeedKind::Audio`), Landscape otherwise — the shared producer's
    /// feed policy, which has no source for either, so the Hero paints its
    /// placeholder.
    pub art_shape: GuiArtShape,
}

/// The Feeds projection the GUI Library panel paints (row 11.1) — the shared
/// pill-scoped entry-list projection over the Feeds pill and Hero: the
/// single-select pill row, the active pill's entry rows, the core's selected
/// entry, and its Hero. No letter rail rides here and no state filter.
pub type GuiFeedsProjection = GuiEntryListProjection<GuiFeedPill, GuiFeedsHero>;

impl App {
    /// The Feeds projection the GUI Library panel paints (row 11.1): the pill
    /// row, the active pill's entry rows, the selected entry, and its Hero. A
    /// Client with no configured subscription projects the empty projection,
    /// as the frame's gate excludes that destination anyway.
    #[must_use]
    pub fn gui_feeds_projection(&self) -> GuiFeedsProjection {
        if !self.gui_feeds_present() {
            return GuiFeedsProjection::default();
        }
        let pill = self.gui_feeds_pill();
        let pills = self.gui_feeds_pills(&pill);
        let rows = self.gui_feeds_rows();
        let selected = self.gui_feeds_selected(&rows);
        let hero = selected
            .as_deref()
            .and_then(GuiFeedEntryId::decode)
            .and_then(|entry| self.gui_feeds_hero(&entry));
        GuiFeedsProjection {
            pills,
            loading: self.feed_tab.loading,
            failed: false,
            rows,
            selected,
            hero,
        }
    }

    /// The pill row (row 11.1): Latest first, then each configured
    /// subscription by its name in the tab's order, with the effective pill
    /// selected.
    fn gui_feeds_pills(&self, pill: &GuiFeedPill) -> Vec<GuiFeedsPillOption> {
        pill_options(
            &GuiFeedPill::Latest,
            pill,
            self.feed_tab.subscriptions.iter().map(|subscription| {
                (
                    GuiFeedPill::Subscription(subscription.url.clone()),
                    subscription.name.clone(),
                )
            }),
        )
    }

    /// The active pill's entry rows (row 11.1): the order is the App's own,
    /// which `FeedTabState::rebuild_all_entries` sorted newest first with
    /// undated entries last.
    fn gui_feeds_rows(&self) -> Vec<GuiFeedsRow> {
        self.gui_feeds_entries()
            .iter()
            .map(|entry| self.gui_feeds_row(entry))
            .collect()
    }

    /// One row of the active list (row 11.1): the TUI feed row's facts — the
    /// entry title, its subscription's name, and the publish date in the
    /// right-aligned gutter — with the row's played state.
    fn gui_feeds_row(&self, entry: &FeedEntry) -> GuiFeedsRow {
        GuiFeedsRow {
            id: GuiFeedEntryId::new(
                entry.feed_id.clone().unwrap_or_default(),
                entry.guid.clone(),
            )
            .encode(),
            title: entry.title.clone(),
            subtitle: self
                .gui_feeds_subscription_name(entry.feed_id.as_deref())
                .map(str::to_string),
            date: provider_timestamp_secs(&mbv_queue::QueueItem::Feed(entry.clone()))
                .map(fmt_publish_date_short)
                .filter(|date| !date.is_empty()),
            state: feed_row_state(entry),
        }
    }

    /// The entry the Hero shows (row 11.1): the core's pick while the active
    /// list still carries it, else that list's first row — the Hero default.
    fn gui_feeds_selected(&self, rows: &[GuiFeedsRow]) -> Option<String> {
        picked_or_first(
            self.gui_feeds_picked_entry().map(str::to_string),
            |picked| rows.iter().any(|row| row.id == picked),
            || rows.first().map(|row| row.id.clone()),
        )
    }

    /// The selected entry's Hero (row 11.1): the shared `hero_content_feed`
    /// producer's title and duration behind the subscription name, and the
    /// feed artwork policy's shape. The entry has no artwork source, so no
    /// image rides here and the Hero paints its placeholder.
    fn gui_feeds_hero(&self, entry: &GuiFeedEntryId) -> Option<GuiFeedsHero> {
        let entry = self
            .gui_feeds_entries()
            .iter()
            .find(|known| feed_entry_matches(known, entry))?;
        let content = hero_content_feed(entry);
        let parent = self
            .gui_feeds_subscription_name(entry.feed_id.as_deref())
            .map(str::to_string);
        Some(GuiFeedsHero {
            title: content.facts.title.clone(),
            parent,
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            art_shape: artwork_shape(content.facts.artwork.shape),
        })
    }

    /// The name of the subscription `feed_id` names (row 11.1), or `None`
    /// when the tab no longer holds it.
    fn gui_feeds_subscription_name(&self, feed_id: Option<&str>) -> Option<&str> {
        let feed_id = feed_id?;
        self.feed_tab
            .subscriptions
            .iter()
            .find(|subscription| subscription.url == feed_id)
            .map(|subscription| subscription.name.as_str())
    }
}

/// Whether `entry` is the one `id` names (row 11.1): the subscription's URL
/// and the entry's `guid`, both — the pair two subscriptions' entries can
/// share half of. An entry the fetch stamped no identity on matches an empty
/// subscription URL.
pub(in crate::app::gui_library) fn feed_entry_matches(
    entry: &FeedEntry,
    id: &GuiFeedEntryId,
) -> bool {
    entry.feed_id.as_deref().unwrap_or_default() == id.feed_id() && entry.guid == id.guid()
}

/// One row's played state (row 11.1): the TUI feed row's rule through the
/// shared `from_progress` arm — a finished entry is `Played`, an unfinished
/// one with a resume position is `Active` with the percent of its duration
/// (no percent when the duration is unknown, but still `Active`), and every
/// other entry is `Ordinary`.
fn feed_row_state(entry: &FeedEntry) -> GuiHomeRowState {
    GuiHomeRowState::from_progress(
        entry.played,
        entry.position_ticks > 0,
        resume_percent(entry.position_ticks, entry.duration_ticks),
    )
}

/// The bounded resume percent of a partly played entry, the TUI row's
/// computation: the played fraction of the duration, clamped to `0..=100`, and
/// `None` when the duration is unknown or not positive.
fn resume_percent(position_ticks: i64, duration_ticks: Option<u64>) -> Option<u8> {
    let duration = duration_ticks.filter(|duration| *duration > 0)?;
    let position = u64::try_from(position_ticks.max(0)).unwrap_or(u64::MAX);
    let percent = (position.saturating_mul(100) / duration).min(100);
    u8::try_from(percent).ok()
}
