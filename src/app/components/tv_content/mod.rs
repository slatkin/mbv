//! The TV embedded content owner (tasks 8.1–8.4,
//! unify-screens-under-panel-components; design.md D12).
//!
//! One plain type owns TV at every breakpoint: the series list (one fixed-row
//! Wide presentation over one shared `MediaListCarrier`), the episode list,
//! the season cursor, and the Inline Search session. It is never mounted,
//! focused, subscribed, or given a `ComponentId`: the mounted `LibraryPanel`
//! hosts it under `LibraryKey::Service(TvShows)` and is the library area's one
//! event boundary. Wide and Narrow both paint through the Library panel
//! skeleton; geometry changes clamp the same owner's viewport in place.
//!
//! Pointer resolution moved into the panel with the registration: the panel
//! resolves the letter pills, season pills, series rows and hero-pane input
//! it painted and hands over typed slot events; this owner resolves the
//! row-local target through its own carriers and emits the same
//! `ShellRequest::TvHit*` messages the deleted component emitted.
use super::inline_search::{InlineSearch, InlineSearchHost};
use super::library_panel::{
    hero_content_emby, HeroContent, HeroContentData, HeroImageState, LibraryContentOwner,
    LibraryPanelContent, LibrarySlotEvent, ListSlot, SelectorRow, Workspace,
};
use super::list::tree_browser::{TreeBrowser, TreeEntry, TreeMarkPolicy, TreeNode, TreeOperation};
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaListTrailing, MediaSemanticState, RowIntent, ViewportAnchor,
};
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent, TvHit};
use super::tv_tree_target::TvTreeTarget;
use crate::app::render::{
    effective_sort_str, letter_bucket, LetterFilter, LetterFilterKind, TvWideRenderCtx,
};
use crate::app::ui_util::{fmt_duration_gutter, fmt_publish_date_short, natural_sort_key};
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use mbv_core::config::{EmbyLetterBucket, EmbySelectorKey, LibraryItemIdentity, SelectorIdentity};
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::Position;
use time::Date;
#[cfg(test)]
use tuirealm::event::Key;
use tuirealm::event::KeyEvent;
mod keyboard;
mod navigation;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Pane {
    Series,
    Episodes,
}
pub(in crate::app) struct TvContent {
    context: TvWideRenderCtx,
    /// The one shared series-row owner, kept in the fixed-row Wide
    /// presentation at every breakpoint; geometry changes clamp its viewport
    /// in place without transferring state to another presentation.
    carrier: MediaListCarrier<String>,
    /// Retained TV hierarchy projection for show modes. Flat episode modes
    /// and Inline Search continue to use `carrier`/their own result list.
    browser: TreeBrowser<TvTreeTarget>,
    season_cursor: usize,
    /// Embedded canonical control for the recessed episode media-list box
    /// (task 4.2d): owns cursor/scroll/hit-resolution for the current
    /// season's episode rows. Content always mirrors the current season
    /// (like `carrier` mirrors the series rail) so the box previews episodes
    /// even while the Series pane holds focus; `pane` controls whether its
    /// selected row paints as focused. Wide-only: Narrow has no Episodes
    /// pane. It is a carrier (task 8.2) so the Library panel's shared
    /// `Workspace` slot can hand it over as a `PanelList`; it never changes
    /// presentation.
    episodes: MediaListCarrier<String>,
    pane: Pane,
    initialized: bool,
    last_series_id: Option<String>,
    viewport_height: usize,
    /// The rows last handed to the series carrier (the 6.1
    /// `last_projected_rows` pattern): an identical re-projection skips
    /// `set_content`, which would otherwise invalidate the painted frame and
    /// make a sync-without-draw frame unclaimable by pointer input (D6).
    last_series_rows: Option<Vec<MediaListRow<String>>>,
    /// The episode twin of [`TvContent::last_series_rows`].
    last_episode_rows: Option<Vec<MediaListRow<String>>>,
    /// The embedded Inline Search control (design.md D1). See
    /// `EmbyLibraryContent::inline_search` for the migration-phase notes.
    inline_search: InlineSearch,
    /// The breakpoint the shell pushed for this frame (`App::
    /// wide_tv_library_area`): `true` paints the pane-based Wide workspace,
    /// `false` paints the flat Narrow series list. Defaults to `true` so an
    /// owner built and viewed without an explicit push (existing unit
    /// tests) keeps painting the Wide workspace.
    is_wide: bool,
    /// Whether the Library Hero overlay is open over this owner (pushed by
    /// the panel, design D2). In Narrow geometry only the overlay focuses
    /// the Episodes pane, so this gates the overlay Workspace's key routing.
    hero_overlay_open: bool,
    latest_has_new_content: bool,
    latest_acknowledged: bool,
}

/// Build the embedded episode `WideMediaList`'s rows from a season's
/// episodes (task 4.2d): the canonical control's row content. Episode
/// runtimes use the shared green gutter; the row state comes from the one
/// canonical derivation.
fn build_episode_rows(episodes: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    episodes
        .iter()
        .enumerate()
        .map(|(index, episode)| {
            let number = if episode.index_number > 0 {
                episode.index_number
            } else {
                index as i64 + 1
            };
            let trailing = (episode.runtime_ticks > 0)
                .then(|| fmt_duration_gutter(episode.runtime_ticks / TICKS_PER_SECOND))
                .map(MediaListTrailing::Gutter);
            MediaListRow::Item {
                target: episode.id.clone(),
                primary: format!("{number}. {}", episode.name),
                secondary: None,
                trailing,
                duration: None,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::from_emby(episode),
            }
        })
        .collect()
}

/// TV Latest shares Home's split episode title and provider-date gutter.
pub(super) fn build_latest_episode_rows(episodes: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    episodes
        .iter()
        .map(|episode| {
            let item = QueueItem::Emby(Box::new(episode.clone()));
            let parts = item.playback_title_parts(None);
            let (primary, secondary) = match parts.context {
                Some(context) => (context.text, Some(parts.title.text)),
                None => (parts.title.text, None),
            };
            let trailing = crate::app::home_latest::provider_timestamp_secs(&item)
                .map(fmt_publish_date_short)
                .filter(|date| !date.is_empty())
                .map(MediaListTrailing::Gutter);
            MediaListRow::Item {
                target: episode.id.clone(),
                primary,
                secondary,
                trailing,
                duration: None,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::from_queue_item(&item),
            }
        })
        .collect()
}

fn parse_premiere_date(value: &str) -> Option<Date> {
    let date = value.split('T').next()?;
    Date::parse(
        date,
        &time::format_description::well_known::Iso8601::DEFAULT,
    )
    .ok()
}

fn upcoming_date_heading(date: Date, today: Date) -> String {
    match (today - date).whole_days() {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => format!("{}, {} {}", date.weekday(), date.month(), date.day()),
    }
}

pub(in crate::app) fn upcoming_episode_target(episode: &EmbyItem) -> String {
    if !episode.id.is_empty() {
        return episode.id.clone();
    }

    let series = if episode.series_id.is_empty() {
        &episode.series_name
    } else {
        &episode.series_id
    };
    format!(
        "upcoming:{}:{}:{}:{}:{}:{}",
        series.len(),
        series,
        episode.parent_index_number,
        episode.index_number,
        episode.name.len(),
        episode.name
    )
}

fn upcoming_episode_rows(episodes: &[EmbyItem], today: Date) -> Vec<MediaListRow<String>> {
    let mut groups: Vec<(Option<Date>, Vec<&EmbyItem>)> = Vec::new();
    for episode in episodes {
        let date = parse_premiere_date(&episode.premiere_date);
        if let Some((_, rows)) = groups
            .iter_mut()
            .find(|(group_date, _)| *group_date == date)
        {
            rows.push(episode);
        } else {
            groups.push((date, vec![episode]));
        }
    }

    groups
        .into_iter()
        .flat_map(|(date, episodes)| {
            let heading = date.map(|date| MediaListRow::Heading {
                text: upcoming_date_heading(date, today),
            });
            heading
                .into_iter()
                .chain(episodes.into_iter().map(|episode| {
                    let trailing = (episode.runtime_ticks > 0)
                        .then(|| fmt_duration_gutter(episode.runtime_ticks / TICKS_PER_SECOND))
                        .map(MediaListTrailing::Gutter);
                    MediaListRow::Item {
                        target: upcoming_episode_target(episode),
                        primary: episode.series_name.clone(),
                        secondary: Some(format!(
                            "S{:02}:E{:02} — {}",
                            episode.parent_index_number, episode.index_number, episode.name
                        )),
                        trailing,
                        duration: None,
                        kind: MediaKind::Media,
                        semantic_state: MediaSemanticState::from_emby(episode),
                    }
                }))
        })
        .collect()
}
impl TvContent {
    pub fn new() -> Self {
        let mut context = TvWideRenderCtx::new(
            crate::app::render::LibraryListRenderCtx::from_items(Vec::new(), 0),
            None,
            None,
            0,
            None,
            false,
        );
        // A bare owner (unit tests) starts focused; the shell pushes the real
        // library-pane focus bit each sync pass (task 8.4).
        context.focused = true;
        Self {
            context,
            carrier: MediaListCarrier::new(),
            browser: TreeBrowser::new(),
            season_cursor: 0,
            episodes: MediaListCarrier::new(),
            pane: Pane::Series,
            initialized: false,
            last_series_id: None,
            viewport_height: 1,
            last_series_rows: None,
            last_episode_rows: None,
            inline_search: InlineSearch::new(),
            is_wide: true,
            hero_overlay_open: false,
            latest_has_new_content: false,
            latest_acknowledged: false,
        }
    }
    /// Records the session-only Wide hero list-pane width override for the
    /// next frame's content producer. Pushed each sync pass by the shell
    /// beside the other per-frame facts; it is a layout fact, not content.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.context.list.list_pane_width = list_pane_width;
    }
    /// Records this frame's breakpoint (design.md D12): `true` selects the
    /// Whether the current geometry uses the Wide pane-based workspace.
    /// Must be pushed before `set_content` so viewport sizing uses the current
    /// frame's geometry.
    pub(in crate::app) fn set_is_wide(&mut self, is_wide: bool) {
        self.is_wide = is_wide;
    }
    pub(in crate::app) fn set_hero_overlay_open(&mut self, open: bool) {
        self.hero_overlay_open = open;
    }

    pub(in crate::app) fn set_latest_marker(&mut self, has_new_content: bool, acknowledged: bool) {
        self.latest_has_new_content = has_new_content;
        self.latest_acknowledged = acknowledged;
    }
    /// Keep the shared owner in its fixed-row presentation and clamp its
    /// viewport for the current geometry. No content or cursor state is copied
    /// between adapters.
    fn ensure_carrier(&mut self) {
        let viewport_height = self.painted_viewport_height();
        self.carrier.clamp_viewport(viewport_height);
    }
    #[cfg(test)]
    pub(in crate::app) fn test_set_letter_filter(&mut self, index: usize) {
        self.context.list.letter_filter =
            LetterFilter::for_index_for_kind(index, LetterFilterKind::Tv);
    }

    /// Project the settled show-mode catalog into the shared tree vocabulary.
    /// Only the selected show's detail snapshot is available at this boundary;
    /// task 2.2 wires in additional shell-owned loaded details.
    fn stable_show_target(show: &EmbyItem, duplicate_id: bool, occurrence: usize) -> String {
        let base = if show.id.is_empty() {
            format!("tv-name:{}:{}", show.name.len(), show.name)
        } else if duplicate_id {
            format!(
                "tv-id:{}:{}:{}:{}",
                show.id.len(),
                show.id,
                show.name.len(),
                show.name
            )
        } else {
            format!("tv-id:{}:{}", show.id.len(), show.id)
        };
        if occurrence == 0 {
            base
        } else {
            format!("{base}:{occurrence}")
        }
    }

    fn tree_projection(context: &TvWideRenderCtx) -> Vec<TreeEntry<TvTreeTarget>> {
        let grouped = context.show_letter_pills
            || context.list.has_letter_filter()
            || context.list.true_total() >= 50;
        let bucket_total = if context.list.has_letter_filter() {
            usize::MAX
        } else {
            context.list.true_total()
        };
        let mut shows: Vec<&EmbyItem> = context.list.items.iter().collect();
        shows.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));

        let mut id_counts = std::collections::HashMap::new();
        for show in &shows {
            *id_counts.entry(show.id.as_str()).or_insert(0usize) += 1;
        }
        let mut target_occurrences = std::collections::HashMap::new();
        let mut entries = Vec::with_capacity(shows.len() + 8);
        let mut previous_bucket: Option<String> = None;
        let mut detail_projected = false;
        for show in shows {
            let duplicate_id = id_counts.get(show.id.as_str()).copied().unwrap_or_default() > 1;
            let base_target = Self::stable_show_target(show, duplicate_id, 0);
            let occurrence = target_occurrences.entry(base_target).or_insert(0usize);
            let show_id = Self::stable_show_target(show, duplicate_id, *occurrence);
            *occurrence += 1;
            let target = TvTreeTarget::Show(show_id.clone());
            if grouped {
                let bucket = letter_bucket(show, bucket_total);
                if previous_bucket.as_deref() != Some(bucket.as_str()) {
                    if previous_bucket.is_some() {
                        entries.push(TreeEntry::Spacer);
                    }
                    entries.push(TreeEntry::Heading(bucket.clone()));
                    previous_bucket = Some(bucket);
                }
            }
            let root_index = entries.len();
            entries.push(TreeEntry::Node(
                TreeNode::new(
                    target.clone(),
                    None,
                    show.display_name(),
                    effective_sort_str(show),
                    MediaSemanticState::from_emby(show),
                    TreeMarkPolicy::Direct,
                )
                .with_expandable(true),
            ));

            let Some(detail) = context
                .selected_series
                .as_ref()
                .filter(|selected| {
                    !detail_projected && selected.id == show.id && selected.name == show.name
                })
                .and(context.series_detail.as_ref())
            else {
                continue;
            };
            detail_projected = true;
            // An empty completed detail has no pending children to load.
            if let TreeEntry::Node(root) = &mut entries[root_index] {
                root.expandable = !detail.seasons.is_empty();
            }
            let mut season_occurrences = std::collections::HashMap::new();
            for season in &detail.seasons {
                let occurrence = season_occurrences
                    .entry(season.id.clone())
                    .or_insert(0usize);
                let season_occurrence = *occurrence;
                let season_target = TvTreeTarget::Season {
                    show: show_id.clone(),
                    season: season.id.clone(),
                    occurrence: season_occurrence,
                };
                *occurrence += 1;
                let episodes = detail.episodes.get(&season.id);
                entries.push(TreeEntry::Node(
                    TreeNode::new(
                        season_target.clone(),
                        Some(target.clone()),
                        season.display_name(),
                        season.name.clone(),
                        MediaSemanticState::from_emby(season),
                        TreeMarkPolicy::Direct,
                    )
                    .with_expandable(episodes.is_none_or(|episodes| !episodes.is_empty())),
                ));
                if let Some(episodes) = episodes {
                    let mut episode_occurrences = std::collections::HashMap::new();
                    for (index, episode) in episodes.iter().enumerate() {
                        let episode_id = if episode.id.is_empty() {
                            upcoming_episode_target(episode)
                        } else {
                            episode.id.clone()
                        };
                        let episode_occurrence = episode_occurrences
                            .entry(episode_id.clone())
                            .or_insert(0usize);
                        let number = if episode.index_number > 0 {
                            episode.index_number
                        } else {
                            index as i64 + 1
                        };
                        let episode_target = TvTreeTarget::Episode {
                            show: show_id.clone(),
                            season: season.id.clone(),
                            season_occurrence,
                            episode: episode_id,
                            occurrence: *episode_occurrence,
                        };
                        *episode_occurrence += 1;
                        entries.push(TreeEntry::Node(TreeNode::new(
                            episode_target,
                            Some(season_target.clone()),
                            format!("{number}. {}", episode.name),
                            episode.name.clone(),
                            MediaSemanticState::from_emby(episode),
                            TreeMarkPolicy::Direct,
                        )));
                    }
                }
            }
        }
        entries
    }

    pub(crate) fn tree_expansion_source(
        &self,
        target: &TvTreeTarget,
    ) -> Option<(String, Option<String>)> {
        let (show_target, season_id) = match target {
            TvTreeTarget::Show(show) => (show, None),
            TvTreeTarget::Season { show, season, .. } => (show, Some(season.clone())),
            TvTreeTarget::Episode { .. } => return None,
        };
        self.browser.node(target)?;
        let mut counts = std::collections::HashMap::new();
        for item in &self.context.list.items {
            *counts.entry(item.id.as_str()).or_insert(0usize) += 1;
        }
        let mut occurrences = std::collections::HashMap::new();
        let mut shows: Vec<_> = self.context.list.items.iter().collect();
        shows.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));
        let show = shows.into_iter().find(|item| {
            let duplicate_id = counts.get(item.id.as_str()).copied().unwrap_or_default() > 1;
            let base = Self::stable_show_target(item, duplicate_id, 0);
            let occurrence = occurrences.entry(base.clone()).or_insert(0usize);
            let matches = Self::stable_show_target(item, duplicate_id, *occurrence) == *show_target;
            *occurrence += 1;
            matches
        })?;
        (!show.id.is_empty()).then(|| (show.id.clone(), season_id))
    }

    // Kept as a typed boundary seam until task 3.1 connects mounted tree input.
    #[allow(dead_code)]
    pub(crate) fn toggle_tree_expansion(&mut self, target: TvTreeTarget) -> Option<Msg> {
        let was_expanded = self.browser.is_expanded(&target);
        let transition = self
            .browser
            .apply(TreeOperation::ToggleExpansionTarget(target.clone()));
        if was_expanded
            || transition.disposition == super::list::tree_browser::TreeConsumed::Unhandled
        {
            return None;
        }
        Some(Msg::Shell(ShellRequest::TvTreeExpand { target }))
    }

    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
        self.ensure_carrier();
        let episode_mode = matches!(
            context.tv_content_mode,
            Some(
                mbv_core::config::TvContentMode::Latest | mbv_core::config::TvContentMode::Upcoming
            )
        );
        let rows = if episode_mode {
            if context.tv_content_mode == Some(mbv_core::config::TvContentMode::Upcoming) {
                let today = time::OffsetDateTime::now_utc().date();
                upcoming_episode_rows(&context.list.items, today)
            } else {
                build_latest_episode_rows(&context.list.items)
            }
        } else {
            let grouped = !self.inline_search.is_active()
                && (context.show_letter_pills
                    || context.list.has_letter_filter()
                    || context.list.true_total() >= 50);
            let bucket_total = if context.list.has_letter_filter() {
                usize::MAX
            } else {
                context.list.true_total()
            };
            let mut sorted_items: Vec<&EmbyItem> = context.list.items.iter().collect();
            sorted_items.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));
            sorted_items
                .iter()
                .enumerate()
                .flat_map(|(index, item)| {
                    let heading = grouped
                        .then(|| {
                            let current = letter_bucket(item, bucket_total);
                            let previous = index
                                .checked_sub(1)
                                .map(|i| letter_bucket(sorted_items[i], bucket_total));
                            (previous.as_deref() != Some(current.as_str())).then(|| {
                                let heading = MediaListRow::Heading { text: current };
                                if previous.is_some() {
                                    vec![MediaListRow::Spacer, heading]
                                } else {
                                    vec![heading]
                                }
                            })
                        })
                        .flatten();
                    heading
                        .into_iter()
                        .flatten()
                        .chain(std::iter::once(MediaListRow::Item {
                            target: item.id.clone(),
                            primary: item.display_name(),
                            secondary: None,
                            trailing: (item.production_year > 0).then(|| {
                                MediaListTrailing::Gutter(item.production_year.to_string())
                            }),
                            duration: None,
                            kind: MediaKind::Collection,
                            semantic_state: MediaSemanticState::from_emby(item),
                        }))
                })
                .collect::<Vec<_>>()
        };
        // Keep the show hierarchy settled beside (not instead of) the flat
        // Library Panel slot. Flat episode modes and Inline Search must not
        // disturb the retained tree's selection or expansion.
        if !episode_mode && !self.inline_search.is_active() {
            let projection = Self::tree_projection(&context);
            // Reconciliation is atomic; if malformed service data still
            // produces a collision, keep the last valid tree instead of
            // taking down the TUI during refresh.
            let _ = self.browser.reconcile(projection);
        }
        // The canonical cursor is in the rendered (natural-sort) order. Seed
        // the local list from that stable target on first mount; thereafter
        // preserve the stable target already owned by the component.
        let restore_target = self.carrier.selected_target().cloned();
        // The 6.1 `last_projected_rows` pattern: an identical re-projection
        // skips `set_content` so the painted frame stays claimable (D6).
        if self.last_series_rows.as_ref() != Some(&rows) {
            self.carrier.set_content(rows.clone());
            self.last_series_rows = Some(rows);
        }
        if !self.initialized {
            // First mount seeds from the shell's stable target, not its
            // numeric display cursor (design.md D4/D5).
            if let Some(item) = context.list.items.get(context.list.cursor()) {
                self.carrier.select_target(&item.id);
            }
        } else if let Some(target) = restore_target {
            self.carrier.select_target(&target);
        }
        let series_changed =
            context.selected_series.as_ref().map(|item| &item.id) != self.last_series_id.as_ref();
        if series_changed {
            self.season_cursor = 0;
            self.episodes.set_content(Vec::new());
            self.pane = Pane::Series;
            self.last_series_id = context.selected_series.as_ref().map(|item| item.id.clone());
        }
        if !self.initialized {
            if !series_changed {
                self.season_cursor = context.season_cursor;
                self.pane = if context.episode_cursor.is_some() {
                    Pane::Episodes
                } else {
                    Pane::Series
                };
            }
            self.initialized = true;
        }
        // Content projection never carries framework focus; preserve the
        // component-owned value across the shell snapshot swap.
        let focused = self.context.focused;
        self.context = context;
        self.context.focused = focused;
        let season_count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        self.season_cursor = self.season_cursor.min(season_count.saturating_sub(1));
        // Missing detail or episode data means the season's refresh is still
        // loading; do not discard the component-local episode selection by
        // clearing the canonical control's content in that interval. Once
        // the season's episode key is present (even an empty `Vec`), refresh
        // unconditionally -- the box previews episodes regardless of pane.
        if self.current_season_episodes_key_present() {
            self.refresh_episode_rows();
        }
    }
    /// Records whether the library pane holds Panel focus this frame. Pushed
    /// by the shell each sync pass, like `set_is_wide`: framework focus lives
    /// on the mounted `LibraryPanel` (task 8.4), while this bit drives the
    /// owner's focus-dependent painting (which pane's rows paint as focused)
    /// and its local key claim — the same value `Component::attr(Focus)`
    /// carried before the owner became embedded.
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.context.focused = focused;
    }
    /// The current season's episode `Vec`, `&[]` when the season has no
    /// episodes loaded yet or `series_detail`/`season_cursor` cannot resolve
    /// one.
    fn current_season_episodes(&self) -> &[EmbyItem] {
        self.context
            .series_detail
            .as_ref()
            .and_then(|detail| {
                let season = detail.seasons.get(self.season_cursor)?;
                detail.episodes.get(&season.id)
            })
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    /// Whether the current season's episode key is present in
    /// `series_detail.episodes` at all (even mapped to an empty `Vec`) --
    /// the loaded/loading distinction the refresh guard in `set_content`
    /// needs, unlike [`Self::current_season_episodes`] which treats both as
    /// empty.
    fn current_season_episodes_key_present(&self) -> bool {
        self.context
            .series_detail
            .as_ref()
            .and_then(|detail| {
                let season = detail.seasons.get(self.season_cursor)?;
                detail.episodes.get(&season.id)
            })
            .is_some()
    }
    /// Rebuild the embedded episode `WideMediaList`'s content from the
    /// current season's episodes (design.md D3): preserves the selected
    /// target where still present, otherwise clamps locally -- the same
    /// canonical machinery the series rail uses.
    fn refresh_episode_rows(&mut self) {
        let rows = build_episode_rows(self.current_season_episodes());
        // The 6.1 `last_projected_rows` pattern: an identical re-projection
        // skips `set_content`, which would otherwise invalidate the painted
        // frame and make a sync-without-draw frame unclaimable by pointer
        // input (design D6 frame invalidation).
        if self.last_episode_rows.as_ref() != Some(&rows) {
            self.episodes.set_content(rows.clone());
            self.last_episode_rows = Some(rows);
        }
    }
    /// This frame's typed Library panel content (design D3, task 8.2): the
    /// letter pills are the one Selector row, the hero comes from the shared
    /// `EmbyItem` producer with the shell-projected image state, and the
    /// Workspace is the season pills plus the episode list. The Inline Search
    /// session takes the list slot while active (the panel places its box in
    /// the Selector row and its results in the list box).
    fn panel_content(&mut self) -> LibraryPanelContent<'_> {
        let searching = self.inline_search.is_active();
        // Hero facts first: reading the projected snapshot and image state
        // ends before the Workspace borrows the episode carrier mutably.
        let flat_episode_mode = self.flat_episode_mode();
        let hero_item = if flat_episode_mode && !self.is_wide && self.hero_overlay_open {
            self.selected_episode_item()
        } else {
            None
        };
        let hero_data = if flat_episode_mode {
            hero_item.map(|episode| hero_content_emby(&episode))
        } else {
            self.context
                .selected_series
                .clone()
                .map(|series| hero_content_emby(&series))
        };
        let hero_data = hero_data.map(|mut data| {
            data.facts.artwork.image = self.context.hero_image.clone();
            data
        });
        // Season pills are the Workspace selector; without a resolvable
        // season there is no selector row and the episode box takes the
        // whole workspace.
        let workspace_selector = self
            .context
            .series_detail
            .as_ref()
            .filter(|detail| !detail.seasons.is_empty())
            .map(|detail| SelectorRow {
                pills: detail
                    .seasons
                    .iter()
                    .map(|season| season.display_name())
                    .collect(),
                markers: vec![],
                active: Some(self.season_cursor.min(detail.seasons.len() - 1)),
            });
        let workspace_focused = self.context.focused && self.pane == Pane::Episodes;
        let selector = if !searching && self.context.show_letter_pills {
            let large = self
                .context
                .list
                .library_total
                .is_some_and(|total| total > crate::app::render::LIBRARY_PILL_THRESHOLD);
            let mut pills = vec!["Latest".to_string(), "Upcoming".to_string()];
            let latest_marker = self.latest_has_new_content && !self.latest_acknowledged;
            if large {
                pills.extend(LetterFilter::labels_for_kind(LetterFilterKind::Tv));
            } else {
                pills.push("All".to_string());
            }
            let active = match self.context.tv_content_mode.as_ref() {
                Some(mbv_core::config::TvContentMode::Latest) => 0,
                Some(mbv_core::config::TvContentMode::Upcoming) => 1,
                Some(mbv_core::config::TvContentMode::All) => 2,
                Some(mbv_core::config::TvContentMode::Range(index)) => index + 2,
                None => {
                    if large {
                        0
                    } else {
                        2
                    }
                }
            };
            Some(SelectorRow {
                markers: std::iter::once(latest_marker)
                    .chain(std::iter::repeat_n(false, pills.len().saturating_sub(1)))
                    .collect(),
                pills,
                active: Some(active),
            })
        } else {
            None
        };
        let hero = hero_data.map(|data| HeroContent {
            facts: data.facts,
            overview: data.overview,
            credits: data.credits,
            workspace: (!flat_episode_mode).then_some(Workspace {
                header: None,
                selector: workspace_selector,
                list: &mut self.episodes,
                focused: workspace_focused,
            }),
        });
        let list = if searching {
            ListSlot::Search(&mut self.inline_search)
        } else if flat_episode_mode {
            ListSlot::Media(&mut self.carrier)
        } else {
            ListSlot::Media(&mut self.browser)
        };
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn cursor(&self) -> usize {
        self.carrier.cursor()
    }
    /// The shared owner's selection as a position in `context.list.items`
    /// (raw, shell-projected order) rather than the active presentation's
    /// displayed (natural-sorted, grouped) row order. Used for the Narrow
    /// shell effects that persist a resting `BrowseLevel` cursor
    /// (`App::narrow_browse_extras`), mirroring the prior TV browse cursor
    /// before the merge.
    pub(in crate::app) fn browse_cursor(&self) -> usize {
        self.carrier
            .selected_target()
            .and_then(|target| {
                self.context
                    .list
                    .items
                    .iter()
                    .position(|item| &item.id == target)
            })
            .unwrap_or(0)
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        self.carrier.viewport_anchor(viewport_height)
    }
    /// The painted item-row height the owner's pagination strides by: the
    /// active presentation's retained content rect from the last frame the
    /// panel viewed it (ADR 0024: the owner reads only geometry it helped
    /// paint), falling back to the height the shell last pushed content for.
    pub(in crate::app) fn painted_viewport_height(&self) -> usize {
        let painted = self
            .carrier
            .current_content_rect()
            .map_or(0, |rect| rect.height) as usize;
        if painted == 0 {
            self.viewport_height
        } else {
            painted
        }
    }
    /// The scroll offset the component tracks for its series list.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }
    #[cfg(test)]
    pub(crate) fn selected_tree_target(&self) -> Option<&TvTreeTarget> {
        self.browser.selected_target()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn selected_item_id(&self) -> Option<String> {
        let target = self.carrier.selected_target()?;
        self.context
            .list
            .items
            .iter()
            .find(|item| &item.id == target)
            .map(|item| item.id.clone())
    }
    /// Shell-driven selection re-anchor (a navigation the shell performed,
    /// e.g. an Inline Search activation): point the series selection at a
    /// stable target; the next content push preserves it.
    pub(in crate::app) fn select_series_target(&mut self, target: &str) {
        self.carrier.select_target(&target.to_string());

        let mut shows: Vec<_> = self.context.list.items.iter().collect();
        shows.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));
        let mut id_counts = std::collections::HashMap::new();
        for show in &shows {
            *id_counts.entry(show.id.as_str()).or_insert(0usize) += 1;
        }
        let mut occurrences = std::collections::HashMap::new();
        let mut target_by_id = std::collections::HashMap::new();
        for show in shows {
            let duplicate_id = id_counts.get(show.id.as_str()).copied().unwrap_or_default() > 1;
            let base = Self::stable_show_target(show, duplicate_id, 0);
            let occurrence = occurrences.entry(base.clone()).or_insert(0usize);
            let show_target = Self::stable_show_target(show, duplicate_id, *occurrence);
            *occurrence += 1;
            target_by_id.entry(show.id.as_str()).or_insert(show_target);
        }
        if let Some(tree_target) = self.browser.roots().into_iter().find_map(|tree_target| {
            let TvTreeTarget::Show(show_target) = tree_target else {
                return None;
            };
            (target_by_id
                .get(target)
                .is_some_and(|candidate| candidate.as_str() == show_target.as_str()))
            .then_some(TvTreeTarget::Show(show_target.clone()))
        }) {
            self.browser.apply(TreeOperation::Select(tree_target));
        }
    }
    /// Enter episode selection (the wide second-Enter move; the same
    /// "workspace is active" state the Hero overlay's focused Workspace
    /// holds in Narrow).
    pub(in crate::app) fn enter_episode_selection(&mut self) {
        self.episodes.select_first();
        self.pane = Pane::Episodes;
    }
    /// Deep selection (task 6.1, design D6 of change
    /// `per-destination-item-navigation`): point the workspace at the
    /// navigated episode's season and select the episode with episode focus.
    /// Returns false when the season is out of range or the episode is not
    /// among the (refreshed) season rows; the caller treats a miss as
    /// absence (the landing stands, default selection, no error).
    pub(in crate::app) fn select_episode_in_season(
        &mut self,
        season_index: usize,
        episode_id: &str,
    ) -> bool {
        let Some(detail) = self.context.series_detail.as_ref() else {
            return false;
        };
        if season_index >= detail.seasons.len() {
            return false;
        }
        self.season_cursor = season_index;
        if self.current_season_episodes_key_present() {
            self.refresh_episode_rows();
        }
        let selected = self.episodes.select_target(&episode_id.to_string());
        if selected {
            self.pane = Pane::Episodes;
        }
        selected
    }
    /// The series item under the component's own cursor, cloned out of the
    /// cached render context. `handle_key`'s Series Enter attaches this to
    /// `ShellRequest::TvActivate` so the shell effect targets the component
    /// selection instead of the mirrored App browse cursor.
    pub(in crate::app) fn selected_item(&self) -> Option<EmbyItem> {
        // Resolve through the same natural/effective order used to build the
        // rail. Stable IDs normally make this equivalent to target lookup;
        // ordinal resolution also keeps malformed duplicate-ID payloads from
        // collapsing two visibly distinct rows onto the first item.
        let mut items: Vec<&EmbyItem> = self.context.list.items.iter().collect();
        items.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));
        items.get(self.carrier.cursor()).cloned().cloned()
    }
    /// The Series snapshot the shell pushed for this frame (`context
    /// .selected_series`), exposed so tests can verify the pushed detail
    /// follows the component's authoritative selection rather than the App
    /// browse cursor.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn selected_series_snapshot(&self) -> Option<&EmbyItem> {
        self.context.selected_series.as_ref()
    }
    /// The episode item under the episode owner's current selection, resolved
    /// from the pushed season detail (design.md D4: the component carries the
    /// stable episode identity; the shell never reads the cursor).
    pub(in crate::app) fn selected_episode_item(&self) -> Option<EmbyItem> {
        if self.flat_episode_mode() {
            return self
                .carrier
                .selected_target()
                .and_then(|target| {
                    self.context
                        .list
                        .items
                        .iter()
                        .find(|item| upcoming_episode_target(item) == *target)
                })
                .cloned();
        }
        self.current_season_episodes()
            .get(self.episodes.cursor())
            .cloned()
    }

    fn flat_episode_mode(&self) -> bool {
        matches!(
            self.context.tv_content_mode,
            Some(
                mbv_core::config::TvContentMode::Latest | mbv_core::config::TvContentMode::Upcoming
            )
        )
    }
    /// Test-only: the episode owner's selectable cursor index, used to prove
    /// the cursor survives a loading refresh where no episode item is
    /// resolvable.
    #[cfg(test)]
    pub(in crate::app) fn episode_cursor(&self) -> usize {
        self.episodes.cursor()
    }
    /// Test-only: the episode owner's resting scroll offset, used to prove
    /// the overlay Workspace's viewport follows cursor/wheel movement.
    #[cfg(test)]
    pub(in crate::app) fn episode_scroll(&self) -> usize {
        self.episodes.scroll()
    }
    /// Test-only: whether the Episodes pane holds the local focus (the
    /// "workspace is active" state).
    #[cfg(test)]
    pub(in crate::app) fn episode_pane_focused(&self) -> bool {
        self.pane == Pane::Episodes
    }
    pub(in crate::app) fn selected_season(&self) -> Option<(String, String)> {
        let series_id = self.context.selected_series.as_ref()?.id.clone();
        let season_id = self
            .context
            .series_detail
            .as_ref()?
            .seasons
            .get(self.season_cursor)?
            .id
            .clone();
        Some((series_id, season_id))
    }
}
include!("interaction.rs");
impl Default for TvContent {
    fn default() -> Self {
        Self::new()
    }
}
impl InlineSearchHost for TvContent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }
    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}
impl LibraryContentOwner for TvContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    fn hero_overlay_target_available(&mut self) -> bool {
        !self.flat_episode_mode() && self.selected_item().is_some()
    }

    fn mini_view_hero_available(&mut self) -> bool {
        !self.is_wide && self.flat_episode_mode() && self.selected_episode_item().is_some()
    }

    fn browser_rows_are_hero_bearing(&mut self) -> bool {
        !self.flat_episode_mode()
    }

    fn inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        Some(self)
    }

    fn inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        Some(self)
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
        self.panel_content()
    }
    /// Translate one resolved slot event from the panel into TV's existing
    /// typed `Msg`s (design D2/D12). The panel hands over the pills, list
    /// and hero-pane input it painted; this owner resolves the row-local
    /// target through its own carriers.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.handle_slot_event(event)
    }
    /// This owner's local key interpretation, forwarded by the focused panel
    /// (the merged component's `handle_key` contract, unchanged: the router
    /// owns every global chord and keeps precedence).
    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }
    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if self.inline_search.is_active()
                && matches!(
                    key.code,
                    tuirealm::event::Key::Esc
                        | tuirealm::event::Key::Enter
                        | tuirealm::event::Key::Backspace
                        | tuirealm::event::Key::Up
                        | tuirealm::event::Key::Down
                        | tuirealm::event::Key::Left
                        | tuirealm::event::Key::Right
                        | tuirealm::event::Key::Char(_)
                ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }
    fn inline_search_active(&self) -> bool {
        self.inline_search.is_active()
    }

    fn launch_selector(
        &self,
        state: &mbv_core::config::TuiLaunchState,
    ) -> Option<super::library_panel::owner::LaunchSelector> {
        if !self.context.show_letter_pills {
            return None;
        }
        let current = self
            .context
            .list
            .letter_filter
            .as_ref()
            .map(|filter| filter.index);
        match state.selector.as_ref() {
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Letter(bucket),
            }) => {
                let target = bucket.to_index();
                (current != Some(target))
                    .then_some(super::library_panel::owner::LaunchSelector::Emby { index: target })
            }
            // No letter pill is represented by an index. The shell uses
            // this out-of-band value for the distinct clear intent.
            _ if current.is_some() => {
                Some(super::library_panel::owner::LaunchSelector::Emby { index: usize::MAX })
            }
            _ => None,
        }
    }

    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.context.list.loading && self.context.list.items.is_empty() {
            return false;
        }
        // The shell applies the selector through App and pushes the resulting
        // content before this item-level re-anchor. Keep selector state
        // owned by that projection rather than mirroring it here.
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Emby { id }) => self.carrier.select_target(id),
            _ => false,
        };
        if !selected {
            self.carrier.select_first();
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        // TV's season pills live in the Hero Workspace and are deliberately
        // excluded from the bounded launch snapshot. Only the main letter
        // Selector and the selected series belong here.
        let selector = self.context.show_letter_pills.then(|| {
            let key = self
                .context
                .list
                .letter_filter
                .as_ref()
                .and_then(|filter| EmbyLetterBucket::from_index(filter.index))
                .map(EmbySelectorKey::Letter)
                .unwrap_or(EmbySelectorKey::Unfiltered);
            SelectorIdentity::Emby { key }
        });
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Emby { id });
        (selector, item)
    }

    fn focus_hero_workspace(&mut self) -> bool {
        if self.flat_episode_mode() {
            // The compact flat-episode hero is passive: its browser carrier
            // remains focused so movement and mode cycling keep reaching the
            // visible list rather than the hidden season workspace.
            return false;
        }
        self.pane = Pane::Episodes;
        true
    }

    fn clear_hero_workspace_focus(&mut self) {
        self.pane = Pane::Series;
    }

    fn set_hero_overlay_open(&mut self, open: bool) {
        self.set_hero_overlay_open(open);
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        if self.flat_episode_mode() {
            (self.hero_overlay_open && !self.is_wide)
                .then(|| self.selected_episode_item())
                .flatten()
                .map(|episode| hero_content_emby(&episode))
        } else {
            self.context.selected_series.as_ref().map(hero_content_emby)
        }
    }
    fn set_hero_image(&mut self, state: HeroImageState) {
        self.context.hero_image = state;
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
#[cfg(test)]
mod tests;
