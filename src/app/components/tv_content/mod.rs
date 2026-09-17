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
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaListTrailing, MediaSemanticState, RowIntent, ViewportAnchor,
};
use super::mouse::gesture::MouseGestureState;
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent, TvHit};
use crate::app::render::{effective_sort_str, letter_bucket, TvWideRenderCtx};
use crate::app::ui_util::{list_duration_secs, natural_sort_key};
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use ratatui::layout::Position;
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
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
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
}
/// Derives the Emby-specific semantic state for a Narrow series row (mirrors
/// `browser::emby_semantic_state`/`emby_library_content::emby_semantic_state`; the
/// provider-neutral `media_list` layer deliberately stays free of `EmbyItem`,
/// so each projection site carries its own copy).
fn emby_semantic_state(item: &EmbyItem) -> MediaSemanticState {
    if item.playback_position_ticks > 0 && !item.played {
        let progress = if item.runtime_ticks > 0 {
            Some(
                ((item.playback_position_ticks as u64 * 100) / item.runtime_ticks as u64).min(100)
                    as u16,
            )
        } else {
            None
        };
        MediaSemanticState::active(progress)
    } else if item.played {
        MediaSemanticState::Played
    } else {
        MediaSemanticState::Ordinary
    }
}
/// Build the embedded episode `WideMediaList`'s rows from a season's
/// episodes (task 4.2d): the same title/duration formatting the hand-painted
/// table previously rendered, now the canonical control's row content.
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
            MediaListRow::Item {
                target: episode.id.clone(),
                primary: format!("{number}. {}", episode.name),
                secondary: None,
                trailing: None,
                duration: list_duration_secs(episode.runtime_ticks / TICKS_PER_SECOND),
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            }
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
            season_cursor: 0,
            episodes: MediaListCarrier::new(),
            pane: Pane::Series,
            initialized: false,
            last_series_id: None,
            viewport_height: 1,
            last_series_rows: None,
            last_episode_rows: None,
            mouse_gestures: MouseGestureState::new(),
            inline_search: InlineSearch::new(),
            is_wide: true,
            hero_overlay_open: false,
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
    /// Keep the shared owner in its fixed-row presentation and clamp its
    /// viewport for the current geometry. No content or cursor state is copied
    /// between adapters.
    fn ensure_carrier(&mut self) {
        let viewport_height = self.painted_viewport_height();
        self.carrier.sync_viewport(viewport_height);
    }
    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
        self.ensure_carrier();
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
        let is_wide = self.is_wide;
        let rows = sorted_items.iter().enumerate().flat_map(|(index, item)| {
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
                    trailing: (item.production_year > 0)
                        .then(|| MediaListTrailing::Year(item.production_year.to_string())),
                    duration: None,
                    kind: MediaKind::Collection,
                    // Deliberate, known divergence (not a bug to unify away):
                    // Wide's series rail never dimmed on watched/played state
                    // pre-merge (legacy rail parity), so it stays
                    // `Ordinary` here. Narrow was painted by
                    // the prior TV browse row projection, which did
                    // dim watched/in-progress rows via `emby_semantic_state`
                    // (legacy detail-list parity); this reproduces that.
                    semantic_state: if is_wide {
                        MediaSemanticState::Ordinary
                    } else {
                        emby_semantic_state(item)
                    },
                }))
        });
        let rows = rows.collect::<Vec<_>>();
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
        let hero_data = self.context.selected_series.as_ref().map(|series| {
            let mut data = hero_content_emby(series);
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
                active: Some(self.season_cursor.min(detail.seasons.len() - 1)),
            });
        let workspace_focused = self.context.focused && self.pane == Pane::Episodes;
        let selector = if !searching && self.context.show_letter_pills {
            Some(SelectorRow {
                pills: crate::app::render::LetterFilter::labels(),
                active: Some(
                    self.context
                        .list
                        .letter_filter
                        .as_ref()
                        .map(|filter| filter.index)
                        .unwrap_or(0),
                ),
            })
        } else {
            None
        };
        let hero = hero_data.map(|data| HeroContent {
            facts: data.facts,
            overview: data.overview,
            credits: data.credits,
            workspace: Some(Workspace {
                header: None,
                selector: workspace_selector,
                list: &mut self.episodes,
                focused: workspace_focused,
            }),
        });
        let list = if searching {
            ListSlot::Search(&mut self.inline_search)
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            controls: None,
            list,
            hero,
        }
    }
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
    /// Whether letter pills are enabled in the pushed context.
    pub(in crate::app) fn show_letter_pills(&self) -> bool {
        self.context.show_letter_pills
    }
    /// The scroll offset the component tracks for its series list.
    pub(in crate::app) fn scroll(&self) -> usize {
        self.carrier.scroll()
    }
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
    }
    /// Enter episode selection (the wide second-Enter move; the same
    /// "workspace is active" state the Hero overlay's focused Workspace
    /// holds in Narrow).
    pub(in crate::app) fn enter_episode_selection(&mut self) {
        self.episodes.select_first();
        self.pane = Pane::Episodes;
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
    pub(in crate::app) fn selected_series_snapshot(&self) -> Option<&EmbyItem> {
        self.context.selected_series.as_ref()
    }
    /// The episode item under the episode owner's current selection, resolved
    /// from the pushed season detail (design.md D4: the component carries the
    /// stable episode identity; the shell never reads the cursor).
    pub(in crate::app) fn selected_episode_item(&self) -> Option<EmbyItem> {
        self.current_season_episodes()
            .get(self.episodes.cursor())
            .cloned()
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
        self.carrier.clear_selection();
    }

    fn hero_overlay_target_available(&mut self) -> bool {
        self.selected_item().is_some()
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

    fn focus_hero_workspace(&mut self) -> bool {
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
        self.context.selected_series.as_ref().map(hero_content_emby)
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
