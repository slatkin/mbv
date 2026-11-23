//! The GUI Library's Feeds projection (add-gui-library-panel design D12,
//! row 11.1): the interim collection pill row of All and one pill per
//! subscription, the selection's entry list in the list's sort — Title orders
//! the entry titles, and the date sorts use the shared
//! `compare_gui_date_order` rule, newest first with undated entries last while
//! Newest — the core's selected entry, and its Hero. All merges every
//! subscription's entries; a checked set merges the checked subscriptions'.
//! The list shows the letter rail exactly while its sort is a name sort, and
//! the TUI's Watched filter is dropped. The Hero is the shared
//! `hero_content_feed` producer's facts — the entry title with the duration —
//! behind the subscription name, with the placeholder artwork only: a feed
//! entry has no artwork source, so the frame carries the shape and no image.
//! The per-subscription lists are the App's own
//! (`FeedTabState::rebuild_all_entries`), so the two Clients cannot drift; the
//! projection reads `App` data only.
//!
//! The projection is a free function over the `App` the GUI core owns, with the
//! `GuiCore` entry as a one-line call (design D4), so the steps other GUI
//! modules read stay reachable through the `App` they hold.

use mbv_feed::FeedPlatform;
use mbv_queue::FeedEntry;
use mbv_ui_model::library_hero::producers::hero_content_feed;
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::ui_util::row_date;

use super::GuiListSort;
use super::GuiSortTarget;
use super::artwork::{feed_media_kind, feed_row_source, row_art};
use super::collection::{GuiCollectionTarget, GuiCollections};
use super::entry_list::{GuiEntryListProjection, GuiEntryRow, picked_or_first};
use super::home::{GuiArtShape, artwork_shape};
use super::list_sort::gui_title_sort_key;
use crate::app::App;
use crate::app::gui_client_state::GuiClientState;
use crate::gui::GuiCore;
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

/// One row of the Feeds list (row 11.1) — the shared collection-scoped
/// entry-list row. The identity is the entry's `(subscription url, guid)` pair,
/// encoded as one opaque string a select, play, or enqueue command sends, never
/// a row position.
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

/// The Feed platform every checked subscription shares (gui-library-header-
/// service-marks design D2): `None` for All, for no checked subscription, for
/// subscriptions of different platforms, and for a checked subscription whose
/// URL shows no known platform. The header's Feed mark follows this one answer,
/// so a Client paints it and no other site derives the rule.
#[must_use]
pub fn gui_feeds_platform(collections: &GuiCollections) -> Option<FeedPlatform> {
    if collections.all {
        return None;
    }
    let mut checked = collections.entries.iter().filter(|entry| entry.checked);
    let platform = FeedPlatform::of_url(&checked.next()?.key)?;
    checked
        .all(|entry| FeedPlatform::of_url(&entry.key) == Some(platform))
        .then_some(platform)
}

/// The Feeds projection the GUI Library panel paints (row 11.1) — the
/// shared collection-scoped entry-list projection over the collection selector
/// and Hero: All and the subscriptions with their checked state, the
/// selection's entry rows, the core's selected entry, and its Hero. No
/// Workspace and no state filter ride here.
pub type GuiFeedsProjection = GuiEntryListProjection<GuiFeedsHero>;

impl GuiCore {
    /// The Feeds projection the GUI Library panel paints (row 11.1): the
    /// collection selector, the selection's entry rows in the list's sort
    /// order, the selected entry, and its Hero.
    #[must_use]
    pub fn gui_feeds_projection(&self) -> GuiFeedsProjection {
        feeds_projection(&self.app, &self.gui)
    }
}

/// The Feeds projection the GUI Library panel paints (row 11.1): the
/// collection selector, the selection's entry rows in the list's sort order,
/// the selected entry, and its Hero. A Client with no configured subscription
/// projects the empty projection, as the frame's gate excludes that
/// destination anyway. The selection's entries are built once and sorted
/// once; the rows and the Hero lookup read the sorted list, so the frame never
/// rebuilds it.
#[must_use]
pub(in crate::app) fn feeds_projection(app: &App, gui: &GuiClientState) -> GuiFeedsProjection {
    if !super::feeds::feeds_present(app) {
        return GuiFeedsProjection::default();
    }
    let sort_frame = super::list_sort::list_sort_frame(app, gui, GuiSortTarget::Feeds);
    let sort = sort_frame.active;
    let collections = super::collection_command::collections(app, gui, GuiCollectionTarget::Feeds);
    let mut entries = super::feeds::feeds_entries(app, gui);
    if sort.is_name_sort() {
        // The entry title leads under Title, and the date newest follows it,
        // so two entries of one title keep the newest first (design D6).
        entries.sort_by_cached_key(|entry| {
            (
                gui_title_sort_key(&entry.title),
                entry.pub_date_secs.is_none(),
                std::cmp::Reverse(entry.pub_date_secs.unwrap_or_default()),
                entry.feed_id.clone().unwrap_or_default(),
                entry.guid.clone(),
            )
        });
    } else {
        entries.sort_by(|left, right| {
            sort.compare_date(left.pub_date_secs, right.pub_date_secs)
                .then_with(|| {
                    (
                        left.feed_id.as_deref().unwrap_or_default(),
                        left.guid.as_str(),
                    )
                        .cmp(&(
                            right.feed_id.as_deref().unwrap_or_default(),
                            right.guid.as_str(),
                        ))
                })
        });
    }
    let rows = feeds_rows(app, &entries, sort);
    let selected = feeds_selected(gui, &rows);
    let hero = selected
        .as_deref()
        .and_then(GuiFeedEntryId::decode)
        .and_then(|entry| feeds_hero(app, &entry, &entries));
    GuiFeedsProjection {
        collections,
        loading: app.feed_tab.loading,
        failed: false,
        rows,
        sort: Some(sort_frame),
        selected,
        hero,
    }
}

/// The Feeds collection selector's catalog (row 11.1): each configured
/// subscription by its name in the tab's order, keyed by its URL.
pub(in crate::app::gui_library) fn feeds_subscriptions(app: &App) -> Vec<(String, String)> {
    app.feed_tab
        .subscriptions
        .iter()
        .map(|subscription| (subscription.url.clone(), subscription.name.clone()))
        .collect()
}

/// The selection's entry rows (row 11.1; gui-collection-dropdown design D2,
/// D4): the projection has already ordered the entries by the list's sort,
/// and these rows carry that order as it is, with the rail letter a name sort
/// gives each row (design D6).
fn feeds_rows(app: &App, entries: &[&FeedEntry], sort: GuiListSort) -> Vec<GuiFeedsRow> {
    entries
        .iter()
        .map(|entry| feeds_row(app, entry, sort))
        .collect()
}

/// One row of the active list (row 11.1): the TUI feed row's facts — the
/// entry title, its subscription's name, and the publish date in the
/// right-aligned gutter — with the row's played state, the rail letter a name
/// sort gives it (design D6), the art its artwork policy declares
/// (gui-delineated-rows design D5: none, so the row is the no-source state),
/// and the media kind that state shows.
fn feeds_row(app: &App, entry: &FeedEntry, sort: GuiListSort) -> GuiFeedsRow {
    GuiFeedsRow {
        id: GuiFeedEntryId::new(
            entry.feed_id.clone().unwrap_or_default(),
            entry.guid.clone(),
        )
        .encode(),
        title: entry.title.clone(),
        subtitle: feeds_subscription_name(app, entry.feed_id.as_deref()).map(str::to_string),
        date: row_date(entry.pub_date_secs),
        state: MediaSemanticState::from_feed_entry(entry),
        sort_letter: sort.rail_letter(&entry.title),
        art: row_art(app, feed_row_source(entry).as_ref()),
        media: feed_media_kind(entry),
    }
}

/// The entry the Hero shows (row 11.1): the core's pick while the active
/// list still carries it, else that list's first row — the Hero default.
fn feeds_selected(gui: &GuiClientState, rows: &[GuiFeedsRow]) -> Option<String> {
    picked_or_first(
        super::feeds::feeds_picked_entry(gui).map(str::to_string),
        |picked| rows.iter().any(|row| row.id == picked),
        || rows.first().map(|row| row.id.clone()),
    )
}

/// The selected entry's Hero (row 11.1): the shared `hero_content_feed`
/// producer's title and duration behind the subscription name, and the feed
/// artwork policy's shape. The entry has no artwork source, so no image rides
/// here and the Hero paints its placeholder. `listed` is the projection's
/// sorted selection list — the list the Hero's row came from — so the lookup
/// re-reads it instead of rebuilding it.
fn feeds_hero(app: &App, entry: &GuiFeedEntryId, listed: &[&FeedEntry]) -> Option<GuiFeedsHero> {
    let entry = listed
        .iter()
        .find(|known| feed_entry_matches(known, entry))?;
    let content = hero_content_feed(entry);
    let parent = feeds_subscription_name(app, entry.feed_id.as_deref()).map(str::to_string);
    Some(GuiFeedsHero {
        title: content.facts.title.clone(),
        parent,
        meta_rows: content.facts.meta_rows.clone(),
        duration_row: content.facts.duration_row,
        progress_row: content.facts.progress_row,
        art_shape: artwork_shape(content.facts.artwork.shape),
    })
}

/// The name of the subscription `feed_id` names (row 11.1), or `None` when
/// the tab no longer holds it.
fn feeds_subscription_name<'a>(app: &'a App, feed_id: Option<&str>) -> Option<&'a str> {
    let feed_id = feed_id?;
    app.feed_tab
        .subscriptions
        .iter()
        .find(|subscription| subscription.url == feed_id)
        .map(|subscription| subscription.name.as_str())
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
