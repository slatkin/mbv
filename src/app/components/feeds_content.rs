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

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
};
use super::library_panel::hero::hero_content_feed;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaListTrailing, MediaListTransition, MediaSemanticState, RowIntent,
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier.cursor()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn watched_filter(&self) -> WatchedFilter {
        self.watched_filter
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn selected_group(&self) -> usize {
        self.selected_group
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn latest_selected(&self) -> bool {
        self.latest_selected
    }

    pub(in crate::app) fn set_latest_marker(&mut self, marker: bool) {
        self.latest_marker = marker;
    }

    pub(in crate::app) fn group_count(&self) -> usize {
        1 + self.subscriptions.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn visible_titles(&self) -> Vec<&str> {
        self.visible_entries
            .iter()
            .map(|entry| entry.title.as_str())
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn subscription_names(&self) -> Vec<&str> {
        self.subscriptions
            .iter()
            .map(|subscription| subscription.name.as_str())
            .collect()
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
        self.selected_group =
            (self.selected_group as i64 + delta).rem_euclid(count as i64) as usize;
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
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return None;
        }
        match key.code {
            Key::Char('r') => Some(Msg::Shell(ShellRequest::RefreshFeeds)),
            Key::Char('w') => {
                self.cycle_watched_filter();
                None
            }
            Key::Up | Key::Char('k') | Key::Left | Key::Char('h') => {
                self.delegate_row_local_input(MediaListSurfaceInput::Move(-1), None);
                None
            }
            Key::Down | Key::Char('j') | Key::Right | Key::Char('l') => {
                self.delegate_row_local_input(MediaListSurfaceInput::Move(1), None);
                None
            }
            Key::PageUp => {
                self.delegate_row_local_input(MediaListSurfaceInput::Page(-1), None);
                None
            }
            Key::PageDown => {
                self.delegate_row_local_input(MediaListSurfaceInput::Page(1), None);
                None
            }
            Key::Home => {
                self.delegate_row_local_input(MediaListSurfaceInput::First, None);
                None
            }
            Key::End => {
                self.delegate_row_local_input(MediaListSurfaceInput::Last, None);
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
            Key::Enter => match self
                .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
                .external_intent
            {
                Some(RowIntent::Activate(target)) => Some(Msg::Shell(ShellRequest::FeedsPlay(
                    self.entry_for_target(&target)
                        .cloned()
                        .into_iter()
                        .collect(),
                ))),
                _ => Some(Msg::Shell(ShellRequest::FeedsPlay(Vec::new()))),
            },
            Key::Char('.') => {
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
                Some(Msg::Shell(ShellRequest::RowContextMenu(
                    crate::app::state::types::context_menu::ContextMenuTargets::Feeds(entries),
                    None,
                )))
            }
            Key::Char('e') => match self
                .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
                .external_intent
            {
                Some(RowIntent::Activate(target)) => Some(Msg::Shell(ShellRequest::FeedsEnqueue(
                    self.entry_for_target(&target)
                        .cloned()
                        .into_iter()
                        .collect(),
                ))),
                _ => Some(Msg::Shell(ShellRequest::FeedsEnqueue(Vec::new()))),
            },
            _ => None,
        }
    }

    fn rebuild_visible_entries(&mut self) {
        let source = if self.latest_selected || self.selected_group == 0 {
            &self.all_entries
        } else {
            self.entries
                .get(self.selected_group - 1)
                .map(Vec::as_slice)
                .unwrap_or(&[])
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
            .map(|row| match row {
                FeedDisplayRow::Spacer => MediaListRow::Spacer,
                FeedDisplayRow::Heading(group) => MediaListRow::Heading {
                    text: group.label().to_string(),
                },
                FeedDisplayRow::Entry(index) => {
                    let entry = &self.visible_entries[index];
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
                            crate::app::home_latest::provider_timestamp_secs(&queue_item)
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
                            let progress = entry
                                .duration_ticks
                                .filter(|duration| *duration > 0)
                                .map(|duration| {
                                    ((entry.position_ticks.max(0) as u64 * 100) / duration).min(100)
                                        as u16
                                });
                            MediaSemanticState::active(progress)
                        } else {
                            MediaSemanticState::Ordinary
                        },
                    }
                }
            })
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

    /// Park the shared owner at the first entry after a discrete group/filter
    /// change (design D5: re-project then explicitly select the required
    /// stable target).
    fn reset_selection(&mut self) {
        self.delegate_row_local_input(MediaListSurfaceInput::First, None);
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

impl LibraryContentOwner for FeedsContent {
    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.loading && self.visible_entries.is_empty() && !self.subscriptions.is_empty() {
            return false;
        }
        if self.subscriptions.is_empty() {
            self.latest_selected = false;
            self.rebuild_visible_entries();
        } else {
            match state.selector.as_ref() {
                Some(SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Latest,
                }) => {
                    self.latest_selected = true;
                }
                Some(SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Filter(filter),
                }) => {
                    self.latest_selected = false;
                    self.selected_group = 0;
                    self.watched_filter = match filter {
                        FeedsFilter::All => WatchedFilter::All,
                        FeedsFilter::Played => WatchedFilter::Watched,
                        FeedsFilter::Unplayed => WatchedFilter::Unwatched,
                    };
                }
                Some(SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Group(FeedGroupKey::Feed(url)),
                }) => {
                    self.latest_selected = false;
                    self.selected_group = self
                        .subscriptions
                        .iter()
                        .position(|subscription| &subscription.url == url)
                        .map(|index| index + 1)
                        .unwrap_or(0);
                    self.watched_filter = WatchedFilter::All;
                }
                _ => {
                    self.latest_selected = false;
                    self.selected_group = 0;
                    self.watched_filter = WatchedFilter::All;
                }
            }
            self.rebuild_visible_entries();
        }
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Feeds { id }) => self.carrier.select_target(id),
            _ => false,
        };
        if !selected {
            self.carrier.select_first();
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = if self.subscriptions.is_empty() {
            None
        } else if self.latest_selected {
            Some(SelectorIdentity::Feeds {
                key: FeedsSelectorKey::Latest,
            })
        } else if self.selected_group == 0 && self.watched_filter != WatchedFilter::All {
            // The combined row's active pill is the filter block or the
            // selected Feed group. A non-default filter is the only stable
            // identity for the filter state in the current group.
            Some(SelectorIdentity::Feeds {
                key: FeedsSelectorKey::Filter(match self.watched_filter {
                    WatchedFilter::All => FeedsFilter::All,
                    WatchedFilter::Watched => FeedsFilter::Played,
                    WatchedFilter::Unwatched => FeedsFilter::Unplayed,
                }),
            })
        } else if self.selected_group == 0 {
            Some(SelectorIdentity::Feeds {
                key: FeedsSelectorKey::Group(FeedGroupKey::All),
            })
        } else {
            self.subscriptions
                .get(self.selected_group - 1)
                .map(|subscription| SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Group(FeedGroupKey::Feed(subscription.url.clone())),
                })
        };
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Feeds { id });
        (selector, item)
    }

    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        self.carrier.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        Some(self.carrier.selection_summary())
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        // Hero first: it only reads the projected snapshot, while the list
        // slot borrows the shared carrier mutably for the rest of the frame.
        let hero = self.selected_entry().map(|entry| {
            let data = hero_content_feed(entry);
            let mut facts = data.facts;
            facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts,
                overview: data.overview,
                credits: data.credits,
                workspace: None,
            }
        });
        let has_subs = !self.subscriptions.is_empty();
        // One Selector bar carries Latest, watched filters, then the existing
        // feed-group pills. Group/filter selection remains owner-local.
        let selector = has_subs.then(|| SelectorRow {
            pills: std::iter::once("Latest".to_string())
                .chain(
                    [
                        WatchedFilter::All,
                        WatchedFilter::Watched,
                        WatchedFilter::Unwatched,
                    ]
                    .iter()
                    .map(|filter| filter.label().to_string()),
                )
                .chain(std::iter::once("All".to_string()))
                .chain(
                    self.subscriptions
                        .iter()
                        .map(|subscription| trunc_str(&subscription.name, MAX_GROUP_LABEL)),
                )
                .collect(),
            markers: super::selector_markers(
                1 + WatchedFilter::COUNT + 1 + self.subscriptions.len(),
                self.latest_marker,
            ),
            // `[`/`]` move the feed-group selection, so the active pill and
            // overflow window follow that group within the combined row.
            active: Some(if self.latest_selected {
                0
            } else {
                1 + WatchedFilter::COUNT + self.selected_group
            }),
        });
        let list = if !has_subs {
            ListSlot::Empty {
                loading: false,
                text: " No feed subscriptions configured".into(),
            }
        } else if self.visible_entries.is_empty() {
            ListSlot::Empty {
                loading: self.loading,
                text: " Press r to load feeds".into(),
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }

    /// Translate one resolved slot event into Feeds' existing typed `Msg`s
    /// (design D2): the panel resolves pointer geometry against its own
    /// painted slots, and the owner resolves the row's stable target through
    /// its own carrier.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                if index == 0 {
                    self.latest_selected = true;
                    self.rebuild_visible_entries();
                    self.reset_selection();
                    Some(Msg::Shell(ShellRequest::FeedsLatestSelected))
                } else if index <= WatchedFilter::COUNT {
                    self.latest_selected = false;
                    self.select_watched_filter(index - 1);
                    None
                } else {
                    self.latest_selected = false;
                    self.select_group(index - 1 - WatchedFilter::COUNT);
                    None
                }
            }
            LibrarySlotEvent::List(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    // The claim gate mirrors the mounted component: a wheel
                    // outside the painted active list is unclaimed.
                    if !self.claims_current_point(at) {
                        return None;
                    }
                    self.delegate_row_local_input(MediaListSurfaceInput::Wheel { at, delta }, None);
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at) => {
                    let target = self.resolve_row_id(at)?;
                    self.delegate_row_local_input(input, Some(target));
                    let _ = ();
                    Some(Msg::Shell(ShellRequest::FeedsRowClick))
                }
                MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.resolve_row_id(at)?;
                    let outcome = self.delegate_row_local_input(input, Some(target.clone()));
                    let entries = match outcome.external_intent {
                        Some(RowIntent::ContextSelection(targets)) => targets
                            .into_iter()
                            .filter_map(|target| self.entry_for_target(&target).cloned())
                            .collect(),
                        _ => vec![self.entry_for_target(&target)?.clone()],
                    };
                    Some(Msg::Shell(ShellRequest::RowContextMenu(
                        crate::app::state::types::context_menu::ContextMenuTargets::Feeds(entries),
                        Some((at.x, at.y)),
                    )))
                }
                MediaListSurfaceInput::DoubleClick(at) => {
                    // Resolve once, then delegate the target-bearing activation.
                    let target = self.resolve_row_id(at)?;
                    self.carrier
                        .delegate_operation(MediaListOperation::Activate(target.clone()));
                    let entry = self.entry_for_target(&target)?.clone();
                    Some(Msg::Shell(ShellRequest::FeedsPlay(vec![entry])))
                }
                _ => None,
            },
            // Feeds has no Workspace and no hero-pane input of its own.
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => self
                .selected_entry()
                .cloned()
                .map(|entry| Msg::Shell(ShellRequest::FeedsPlay(vec![entry]))),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if matches!(
                key.code,
                Key::Up
                    | Key::Down
                    | Key::PageUp
                    | Key::PageDown
                    | Key::Home
                    | Key::End
                    | Key::Left
                    | Key::Right
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    // Feed entries are leaf Heroes: the first Enter opens the Library Hero
    // overlay and the overlay's subsequent activation uses the same typed
    // playback request as the browser row.
    fn hero_overlay_available(&mut self) -> bool {
        self.selected_entry().is_some()
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.selected_entry().map(hero_content_feed)
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::app::components::library_panel::content::ArtworkShape;
    use mbv_core::config::FeedKind;

    fn subscription(name: &str) -> FeedSubscription {
        FeedSubscription {
            name: name.into(),
            url: format!("https://example.test/{name}"),
            kind: FeedKind::Audio,
        }
    }

    fn entry(guid: &str, kind: FeedKind, played: bool) -> FeedEntry {
        FeedEntry {
            guid: guid.into(),
            title: guid.into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: Some(kind),
            feed_id: None,
            position_ticks: 0,
            played,
        }
    }

    fn owner(subscriptions: &[FeedSubscription], entries: Vec<FeedEntry>) -> FeedsContent {
        let mut owner = FeedsContent::new();
        let grouped = vec![entries.clone()];
        owner.set_content(FeedsOwnerPush {
            subscriptions: subscriptions.to_vec(),
            entries: grouped,
            all_entries: entries,
            loading: false,
        });
        owner
    }

    /// One Selector row carries watched-filter pills followed by feed groups.
    #[test]
    fn content_has_one_selector_row_for_filter_and_groups() {
        let mut owner = owner(
            &[subscription("A"), subscription("B")],
            vec![entry("one", FeedKind::Audio, false)],
        );
        let content = owner.content();
        let selector = content.selector.as_ref().expect("feed-group pills");
        assert_eq!(
            selector.pills,
            ["Latest", "All", "Played", "Unplayed", "All", "A", "B"]
        );
        assert_eq!(selector.active, Some(4));
        // The panel's no-secondary-row contract is owned by the shared
        // `narrow_skeleton_keeps_fixed_rows_and_panel_slots` test.
        drop(content);

        owner.cycle_group(1);
        assert_eq!(owner.content().selector.unwrap().active, Some(5));
    }

    /// Without subscriptions the legacy chrome painted no pill bar at all:
    /// the Selector row is absent; the shared panel owns the row structure.
    #[test]
    fn content_omits_selector_row_without_subscriptions() {
        let mut owner = FeedsContent::new();
        owner.set_content(FeedsOwnerPush {
            subscriptions: Vec::new(),
            entries: Vec::new(),
            all_entries: Vec::new(),
            loading: false,
        });
        let content = owner.content();
        assert!(content.selector.is_none());
        // Secondary-row absence is covered by the shared panel skeleton test;
        // this owner test only covers Feeds' empty-state slot projection.
        match content.list {
            ListSlot::Empty { loading, text } => {
                assert!(!loading);
                assert_eq!(text, " No feed subscriptions configured");
            }
            _ => panic!("expected the unconfigured placeholder"),
        }
    }

    /// The watched pill state remains owner-local, and an empty filtered list
    /// still renders the selector plus the reload placeholder.
    #[test]
    fn content_selector_follows_the_active_watched_filter() {
        let mut owner = owner(
            &[subscription("A")],
            vec![entry("unplayed", FeedKind::Audio, false)],
        );
        owner.cycle_watched_filter();
        assert_eq!(owner.watched_filter(), WatchedFilter::Watched);
        let content = owner.content();
        assert_eq!(content.selector.unwrap().active, Some(4));
        match content.list {
            ListSlot::Empty { loading, text } => {
                assert!(!loading);
                assert_eq!(text, " Press r to load feeds");
            }
            _ => panic!("expected the empty-filter placeholder"),
        }
    }

    /// A selected podcast feed entry is Square with the placeholder (design
    /// D5: Feeds entries declare no artwork source).
    #[test]
    fn selected_podcast_entry_hero_is_square_with_the_placeholder() {
        let mut owner = owner(
            &[subscription("A")],
            vec![entry("episode", FeedKind::Audio, false)],
        );
        let hero = owner.content().hero.expect("selected entry hero");
        assert_eq!(hero.facts.artwork.shape, ArtworkShape::Square);
        assert!(hero.facts.artwork.source.is_none());
        assert_eq!(hero.facts.title, "episode");
    }

    /// A selected video feed entry is the Landscape placeholder (design D5).
    #[test]
    fn selected_video_entry_hero_is_landscape_with_the_placeholder() {
        let mut owner = owner(
            &[subscription("A")],
            vec![entry("video", FeedKind::Video, false)],
        );
        let hero = owner.content().hero.expect("selected entry hero");
        assert_eq!(hero.facts.artwork.shape, ArtworkShape::Landscape);
        assert!(hero.facts.artwork.source.is_none());
    }

    /// The list slot hands the shared carrier over as the active canonical
    /// list when the projection has selectable entries.
    #[test]
    fn content_list_slot_is_the_canonical_media_list_with_entries() {
        let mut owner = owner(
            &[subscription("A")],
            vec![entry("one", FeedKind::Audio, false)],
        );
        assert!(matches!(owner.content().list, ListSlot::Media(_)));
    }
}
