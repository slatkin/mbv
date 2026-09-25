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
    MediaListTrailing, MediaSemanticState, RowIntent,
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
mod episode_rows;
mod keyboard;
mod navigation;
mod panel_owner;
mod tree_projection;

#[cfg(test)]
use episode_rows::build_episode_rows;
pub(in crate::app) use episode_rows::upcoming_episode_target;
use episode_rows::{build_latest_episode_rows, upcoming_episode_rows};
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
    latest_marker: bool,
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
            latest_marker: false,
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

    pub(in crate::app) fn set_latest_marker(&mut self, marker: bool) {
        self.latest_marker = marker;
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

    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
        self.ensure_carrier();
        let episode_mode = Self::is_episode_mode(&context);
        let rows = Self::content_rows(&context, self.inline_search.is_active());
        self.refresh_tree(&context, episode_mode);
        let seed_series_id = self.project_series_rows(rows, &context);
        let series_changed = self.update_selected_series(&context);
        self.initialize_selection(&context, series_changed);
        self.replace_context(context);
        // First mount seeds the tree from the same shell stable target the
        // carrier seeds from: `reconcile_selection` otherwise defaults the
        // tree onto root 0 and the next push resolves (and fetches) that
        // show instead. Flat episode modes and Inline Search reconcile no
        // tree, so there the select finds no matching root and no-ops.
        if let Some(target) = seed_series_id {
            self.select_series_target(&target);
        }
        self.refresh_current_season();
    }

    fn is_episode_mode(context: &TvWideRenderCtx) -> bool {
        matches!(
            context.tv_content_mode,
            Some(
                mbv_core::config::TvContentMode::Latest | mbv_core::config::TvContentMode::Upcoming
            )
        )
    }

    fn content_rows(
        context: &TvWideRenderCtx,
        inline_search_active: bool,
    ) -> Vec<MediaListRow<String>> {
        match context.tv_content_mode {
            Some(mbv_core::config::TvContentMode::Latest) => {
                build_latest_episode_rows(&context.list.items)
            }
            Some(mbv_core::config::TvContentMode::Upcoming) => {
                upcoming_episode_rows(&context.list.items, time::OffsetDateTime::now_utc().date())
            }
            Some(
                mbv_core::config::TvContentMode::All | mbv_core::config::TvContentMode::Range(_),
            )
            | None => Self::series_rows(context, inline_search_active),
        }
    }

    fn series_rows(
        context: &TvWideRenderCtx,
        inline_search_active: bool,
    ) -> Vec<MediaListRow<String>> {
        let grouped = !inline_search_active
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
                Self::series_heading(&sorted_items, index, bucket_total, grouped)
                    .into_iter()
                    .chain(std::iter::once(MediaListRow::Item {
                        target: item.id.clone(),
                        primary: item.display_name(),
                        secondary: None,
                        trailing: (item.production_year > 0)
                            .then(|| MediaListTrailing::Gutter(item.production_year.to_string())),
                        duration: None,
                        kind: MediaKind::Collection,
                        semantic_state: MediaSemanticState::from_emby(item),
                    }))
            })
            .collect()
    }

    fn series_heading(
        sorted_items: &[&EmbyItem],
        index: usize,
        bucket_total: usize,
        grouped: bool,
    ) -> Vec<MediaListRow<String>> {
        if !grouped {
            return Vec::new();
        }
        let current = letter_bucket(sorted_items[index], bucket_total);
        let previous = index
            .checked_sub(1)
            .map(|i| letter_bucket(sorted_items[i], bucket_total));
        if previous.as_deref() == Some(current.as_str()) {
            return Vec::new();
        }
        let heading = MediaListRow::Heading { text: current };
        if previous.is_some() {
            vec![MediaListRow::Spacer, heading]
        } else {
            vec![heading]
        }
    }

    fn refresh_tree(&mut self, context: &TvWideRenderCtx, episode_mode: bool) {
        // Keep the show hierarchy settled beside (not instead of) the flat
        // Library Panel slot. Flat episode modes and Inline Search must not
        // disturb the retained tree's selection or expansion.
        if !episode_mode && !self.inline_search.is_active() {
            let projection = Self::tree_projection(context);
            // Reconciliation is atomic; if malformed service data still
            // produces a collision, keep the last valid tree instead of
            // taking down the TUI during refresh.
            let _ = self.browser.reconcile(projection);
        }
    }

    fn project_series_rows(
        &mut self,
        rows: Vec<MediaListRow<String>>,
        context: &TvWideRenderCtx,
    ) -> Option<String> {
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
        if self.initialized {
            if let Some(target) = restore_target {
                self.carrier.select_target(&target);
            }
            None
        } else {
            // First mount seeds from the shell's stable target, not its
            // numeric display cursor (design.md D4/D5).
            context.list.items.get(context.list.cursor()).map(|item| {
                self.carrier.select_target(&item.id);
                item.id.clone()
            })
        }
    }

    fn update_selected_series(&mut self, context: &TvWideRenderCtx) -> bool {
        let series_changed =
            context.selected_series.as_ref().map(|item| &item.id) != self.last_series_id.as_ref();
        if series_changed {
            self.season_cursor = 0;
            self.episodes.set_content(Vec::new());
            self.pane = Pane::Series;
            self.last_series_id = context.selected_series.as_ref().map(|item| item.id.clone());
        }
        series_changed
    }

    fn initialize_selection(&mut self, context: &TvWideRenderCtx, series_changed: bool) {
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
    }

    fn replace_context(&mut self, context: TvWideRenderCtx) {
        // Content projection never carries framework focus; preserve the
        // component-owned value across the shell snapshot swap.
        let focused = self.context.focused;
        self.context = context;
        self.context.focused = focused;
    }

    fn refresh_current_season(&mut self) {
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
    #[cfg(test)]
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
    #[cfg(test)]
    pub(crate) fn selected_tree_target(&self) -> Option<&TvTreeTarget> {
        self.browser.selected_target()
    }

    #[cfg(test)]
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

        let mut target_by_id = std::collections::HashMap::new();
        for (show_target, show) in Self::show_targets(&self.context.list.items) {
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
}
mod interaction;
impl Default for TvContent {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests;
