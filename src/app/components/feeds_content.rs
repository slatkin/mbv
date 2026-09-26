//! The Feeds destination's embedded content owner (task 7.1, design D2/D3).
//! A plain type — never mounted, focused, subscribed, or given a
//! `ComponentId` — that keeps the shell-projected feed snapshot, the
//! feed-group/Watched filter selection, and the one shared canonical
//! `MediaList` owner of the active group's grouped-entry projection. It
//! produces the panel's [`LibraryPanelContent`] per frame and translates the
//! panel's slot events into the same typed `Msg`s the deleted mounted Feeds
//! component emitted.
//!
//! Task 7.3 registers the owner in the mounted `LibraryPanel`'s map under
//! `LibraryKey::Feeds`: the panel is the library area's one event boundary,
//! so hit resolution and the Wide split gesture happen there and this owner
//! only translates the resolved slot events and forwarded chords.
//!
//! The selected entry's hero comes from the shared `hero_content_feed`
//! producer (design D5) — Square for a podcast feed, the Landscape
//! placeholder otherwise — and this owner never fetches an image itself:
//! the shell's hero projection (task 5.10) supplies the image state through
//! [`LibraryContentOwner::set_hero_image`].

use ratatui::layout::Position;

use mbv_core::config::{
    FeedGroupKey, FeedSubscription, FeedsFilter, FeedsSelectorKey, LibraryItemIdentity,
    SelectorIdentity,
};
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::library_panel::HeroImageState;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaListSurfaceInput, MediaListTrailing,
    MediaListTransition, MediaSemanticState, RowIntent,
};
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::{current_time_secs, feed_display_rows, FeedDisplayRow};
use crate::app::state::types::feed_tab::WatchedFilter;
use crate::app::ui_util::trunc_str;

/// Max feed-group pill label length. The Feeds owner and the podcast tab's
/// show pills are the producers of the Selector row's labels (design D8: no
/// destination-side pill vocabulary).
pub(in crate::app) const MAX_GROUP_LABEL: usize = 18;

/// The owner's shell-projected snapshot (the legacy Feeds component's
/// `set_content` contract). The Watched filter and the selected feed group
/// are owner-local selection, not shell content, so they never cross this
/// seam.
pub(in crate::app) struct FeedsOwnerPush {
    pub subscriptions: Vec<FeedSubscription>,
    pub entries: Vec<Vec<FeedEntry>>,
    pub all_entries: Vec<FeedEntry>,
    pub loading: bool,
}

/// The Feeds embedded content owner (design D2, task 7.1). Plain type,
/// hosted by the mounted `LibraryPanel` under `LibraryKey::Feeds` (task 7.3).
pub(in crate::app) struct FeedsContent {
    subscriptions: Vec<FeedSubscription>,
    entries: Vec<Vec<FeedEntry>>,
    all_entries: Vec<FeedEntry>,
    visible_entries: Vec<FeedEntry>,
    /// The one shared canonical owner of the active group's grouped-entry
    /// projection. It owns cursor, scroll, and selected target; presentation
    /// is driven from the destination's breakpoint (the panel's, once Feeds
    /// registers).
    carrier: MediaListCarrier<String>,
    watched_filter: WatchedFilter,
    selected_group: usize,
    latest_selected: bool,
    latest_marker: bool,
    loading: bool,
    last_subscription_urls: Vec<String>,
    /// The rows last handed to the carrier (the 6.1 `last_projected_rows`
    /// pattern): an identical re-projection skips `set_content`, which would
    /// otherwise invalidate the painted frame and make a sync-without-draw
    /// frame unclaimable by pointer input.
    last_projected_rows: Option<Vec<MediaListRow<String>>>,
    /// The projection's image state for the current hero (task 5.10): set by
    /// the shell, read by the painters through the panel content.
    hero_image: HeroImageState,
}

impl FeedsContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
            entries: Vec::new(),
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            carrier: MediaListCarrier::new(),
            watched_filter: WatchedFilter::default(),
            selected_group: 0,
            latest_selected: false,
            latest_marker: false,
            loading: false,
            last_subscription_urls: Vec::new(),
            last_projected_rows: None,
            hero_image: HeroImageState::None,
        }
    }

    /// Replace the shell-owned snapshot while preserving this owner's
    /// render/input state shape (the legacy Feeds component's `set_content`
    /// contract, unchanged).
    pub(in crate::app) fn set_content(&mut self, push: FeedsOwnerPush) {
        let subscription_urls: Vec<String> = push
            .subscriptions
            .iter()
            .map(|subscription| subscription.url.clone())
            .collect();
        let subscriptions_changed = self.last_subscription_urls != subscription_urls;
        self.last_subscription_urls = subscription_urls;
        self.subscriptions = push.subscriptions;
        self.entries = push.entries;
        self.all_entries = push.all_entries;
        if self.subscriptions.is_empty() {
            self.latest_selected = false;
        }
        self.selected_group = self
            .selected_group
            .min(self.group_count().saturating_sub(1));
        self.loading = push.loading;
        self.rebuild_visible_entries();
        // An ordinary refresh keeps the active control authoritative (the
        // selected target is preserved by `MediaList::set_content`); only a
        // subscription-set change resets the selection.
        if subscriptions_changed {
            self.reset_selection();
        }
    }

    pub(in crate::app) fn latest_selected(&self) -> bool {
        self.latest_selected
    }

    pub(in crate::app) fn set_latest_marker(&mut self, marker: bool) {
        self.latest_marker = marker;
    }

    pub(in crate::app) fn group_count(&self) -> usize {
        1 + self.subscriptions.len()
    }

    /// The stable row id under `point`, resolved by the shared owner that
    /// painted the active list (design D6).
    pub(in crate::app) fn resolve_row_id(&self, at: Position) -> Option<String> {
        self.carrier.resolve_current_point(at).cloned()
    }

    /// Whether the active presentation's retained frame claims `point`
    /// (design D6 frame invalidation).
    pub(in crate::app) fn claims_current_point(&self, at: Position) -> bool {
        self.carrier.claims_current_point(at)
    }

    /// The entry whose stable `guid` the shared owner selected. Effect
    /// requests are built from this owner-resolved target, never by indexing
    /// `visible_entries` with the cursor.
    pub(in crate::app) fn entry_for_target(&self, target: &str) -> Option<&FeedEntry> {
        self.visible_entries
            .iter()
            .find(|entry| entry.guid == target)
    }

    pub(in crate::app) fn delegate_row_local_input(
        &mut self,
        input: MediaListSurfaceInput,
        pointer_target: Option<String>,
    ) -> MediaListTransition<String> {
        input
            .into_operation(pointer_target)
            .map_or_else(MediaListTransition::unhandled, |operation| {
                self.carrier.delegate_operation(operation)
            })
    }

    /// Cycle the Watched filter and re-project (the legacy `w` key).
    pub(in crate::app) fn cycle_watched_filter(&mut self) {
        self.latest_selected = false;
        self.watched_filter = self.watched_filter.cycle();
        self.rebuild_visible_entries();
        self.reset_selection();
    }

    /// Cycle the feed group and re-project (the legacy `[`/`]` keys).
    pub(in crate::app) fn cycle_group(&mut self, delta: i64) {
        self.latest_selected = false;
        let count = self.group_count();
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let selected = i64::try_from(self.selected_group).unwrap_or(i64::MAX);
        self.selected_group =
            usize::try_from((selected + delta).rem_euclid(count)).unwrap_or(usize::MAX);
        self.rebuild_visible_entries();
        self.reset_selection();
    }

    /// Adopt `target` as the selected group and rebuild.
    fn select_group(&mut self, target: usize) {
        self.selected_group = target;
        self.rebuild_visible_entries();
        self.reset_selection();
    }

    /// Set the Watched filter from a Selector-row pill index.
    fn select_watched_filter(&mut self, position: usize) {
        if let Some(filter) = WatchedFilter::from_position(position) {
            self.watched_filter = filter;
            self.rebuild_visible_entries();
            self.reset_selection();
        }
    }

    /// The entry whose stable `guid` the shared owner selected, or `None`.
    fn selected_entry(&self) -> Option<&FeedEntry> {
        self.carrier
            .selected_target()
            .and_then(|target| self.entry_for_target(target))
    }

    /// This owner's local key interpretation, forwarded by the focused panel
    /// (the legacy Feeds component's `handle_key` contract, unchanged: the
    /// router owns every global chord and keeps precedence). Page movement
    /// uses the shared owner's canonical page stride.
    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            ))));
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return None;
        }
        if self.handle_navigation_key(key.code) {
            return None;
        }
        match key.code {
            Key::Char('r') => Some(Msg::Shell(Box::new(ShellRequest::RefreshFeeds))),
            Key::Char('w') => {
                self.cycle_watched_filter();
                None
            }
            Key::Char('[') => {
                self.cycle_group(-1);
                None
            }
            Key::Char(']') => {
                self.cycle_group(1);
                None
            }
            Key::Enter => Some(self.play_selected_feed()),
            Key::Char('e') => Some(self.enqueue_selected_feed()),
            Key::Char('.') => self.open_selected_feed_context(),
            _ => None,
        }
    }

    fn handle_navigation_key(&mut self, key: Key) -> bool {
        let input = match key {
            Key::Up | Key::Char('k' | 'h') | Key::Left => MediaListSurfaceInput::Move(-1),
            Key::Down | Key::Char('j' | 'l') | Key::Right => MediaListSurfaceInput::Move(1),
            Key::PageUp => MediaListSurfaceInput::Page(-1),
            Key::PageDown => MediaListSurfaceInput::Page(1),
            Key::Home => MediaListSurfaceInput::First,
            Key::End => MediaListSurfaceInput::Last,
            _ => return false,
        };
        self.delegate_row_local_input(input, None);
        true
    }

    fn play_selected_feed(&mut self) -> Msg {
        match self
            .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
            .external_intent
        {
            Some(RowIntent::Activate(target)) => Msg::Shell(Box::new(ShellRequest::FeedsPlay(
                self.entry_for_target(&target)
                    .cloned()
                    .into_iter()
                    .collect(),
            ))),
            _ => Msg::Shell(Box::new(ShellRequest::FeedsPlay(Vec::new()))),
        }
    }

    fn enqueue_selected_feed(&mut self) -> Msg {
        match self
            .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
            .external_intent
        {
            Some(RowIntent::Activate(target)) => Msg::Shell(Box::new(ShellRequest::FeedsEnqueue(
                self.entry_for_target(&target)
                    .cloned()
                    .into_iter()
                    .collect(),
            ))),
            _ => Msg::Shell(Box::new(ShellRequest::FeedsEnqueue(Vec::new()))),
        }
    }

    fn open_selected_feed_context(&mut self) -> Option<Msg> {
        let target = self.carrier.selected_target()?.clone();
        let entries = match self
            .delegate_row_local_input(MediaListSurfaceInput::Context, None)
            .external_intent
        {
            Some(RowIntent::ContextSelection(targets)) => targets
                .into_iter()
                .filter_map(|target| self.entry_for_target(&target).cloned())
                .collect(),
            _ => vec![self.entry_for_target(&target)?.clone()],
        };
        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Feeds(entries),
            None,
        ))))
    }

    fn rebuild_visible_entries(&mut self) {
        let source = if self.latest_selected || self.selected_group == 0 {
            self.all_entries.as_slice()
        } else {
            self.entries
                .get(self.selected_group - 1)
                .map_or(&[][..], Vec::as_slice)
        };
        self.visible_entries = source
            .iter()
            .filter(|entry| self.latest_selected || self.watched_filter.matches(entry.played))
            .cloned()
            .collect();

        // Project grouped `FeedEntries` into the canonical row vocabulary
        // (the legacy Feeds `rebuild_visible_entries` body).
        let now = current_time_secs();
        let rows: Vec<MediaListRow<String>> = feed_display_rows(&self.visible_entries, now)
            .into_iter()
            .map(|row| self.media_list_row(&row))
            .collect();
        // Ordinary refresh: an unchanged projection preserves the shared
        // owner's painted frame instead of re-issuing it (the 6.1
        // `last_projected_rows` skip; the shell pushes the same snapshot every
        // sync, and re-setting it would invalidate the painted frame).
        if self.last_projected_rows.as_ref() != Some(&rows) {
            self.carrier.set_content(rows.clone());
            self.last_projected_rows = Some(rows);
        }
    }

    fn media_list_row(&self, row: &FeedDisplayRow) -> MediaListRow<String> {
        match row {
            FeedDisplayRow::Spacer => MediaListRow::Spacer,
            FeedDisplayRow::Heading(group) => MediaListRow::Heading {
                text: group.label().to_string(),
            },
            FeedDisplayRow::Entry(index) => {
                let entry = &self.visible_entries[*index];
                let (primary, secondary, trailing) = if self.latest_selected {
                    let feed_name = entry.feed_id.as_deref().and_then(|feed_id| {
                        self.subscriptions
                            .iter()
                            .find(|subscription| subscription.url == feed_id)
                            .map(|subscription| subscription.name.as_str())
                    });
                    let queue_item = QueueItem::Feed(entry.clone());
                    let parts = queue_item.playback_title_parts(feed_name);
                    let (primary, secondary) = match parts.context {
                        Some(context) => (context.text, Some(parts.title.text)),
                        None => (parts.title.text, None),
                    };
                    let trailing =
                        crate::app::state::home_latest::provider_timestamp_secs(&queue_item)
                            .map(crate::app::ui_util::fmt_publish_date_short)
                            .filter(|date| !date.is_empty())
                            .map(MediaListTrailing::Gutter);
                    (primary, secondary, trailing)
                } else {
                    (entry.title.clone(), None, None)
                };
                MediaListRow::Item {
                    target: entry.guid.clone(),
                    primary,
                    secondary,
                    trailing,
                    // Library lists carry no time column (only the Queue
                    // list and the sessions modal show one).
                    duration: None,
                    kind: MediaKind::Media,
                    semantic_state: if entry.played {
                        MediaSemanticState::Played
                    } else if entry.position_ticks > 0 {
                        let progress =
                            entry
                                .duration_ticks
                                .filter(|duration| *duration > 0)
                                .map(|duration| {
                                    (u64::try_from(entry.position_ticks.max(0))
                                        .unwrap_or(u64::MAX)
                                        .saturating_mul(100)
                                        / duration)
                                        .min(100) as u16
                                });
                        MediaSemanticState::active(progress)
                    } else {
                        MediaSemanticState::Ordinary
                    },
                }
            }
        }
    }

    /// Park the shared owner at the first entry after a discrete group/filter
    /// change (design D5: re-project then explicitly select the required
    /// stable target).
    fn reset_selection(&mut self) {
        self.delegate_row_local_input(MediaListSurfaceInput::First, None);
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_cursor(&self) -> usize {
        self.carrier.cursor()
    }

    /// The selected feed group, and the Watched filter applied within it:
    /// owner-local selection state with no other production-observable
    /// signal (the Selector row's `active` pill only ever encodes Latest or
    /// the group, never the filter — see `content()`).
    #[cfg(test)]
    pub(in crate::app) fn selected_group(&self) -> usize {
        self.selected_group
    }

    #[cfg(test)]
    pub(in crate::app) fn watched_filter(&self) -> WatchedFilter {
        self.watched_filter
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_rows(&self) -> &[MediaListRow<String>] {
        self.carrier.rows()
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_selectable_len(&self) -> usize {
        self.carrier
            .rows()
            .iter()
            .filter(|row| row.selectable_target().is_some())
            .count()
    }

    #[cfg(test)]
    pub(in crate::app) fn canonical_selected_target(&self) -> Option<&String> {
        self.carrier.selected_target()
    }
}

impl Default for FeedsContent {
    fn default() -> Self {
        Self::new()
    }
}

mod panel;

#[cfg(test)]
mod tests;
