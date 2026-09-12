//! Interactive Component for the merged TV content owner (task 8.1,
//! unify-screens-under-panel-components; design.md D12).
//!
//! One mounted component now owns TV at every breakpoint: the series list
//! (Wide and Inline presentations over one shared `MediaListCarrier`), the
//! episode list, the season cursor, and the Inline Search session. Wide and
//! Narrow both paint through the Library panel skeleton; a breakpoint flip is
//! an ordinary `set_presentation` on the shared owner, not a component
//! hand-off.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};

use super::inline_search::{InlineSearch, InlineSearchHost, InlineSearchMouse};
use super::library_panel::{
    hero_content_emby, render_narrow_skeleton, render_wide_skeleton, HeroContent,
    LibraryPanelContent, ListSlot, PanelHeroImagePaint, SelectorRow, SkeletonHits,
    WideSkeletonGeometry, Workspace,
};
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowLocalInput,
    ViewportAnchor,
};
use super::mouse::gesture::MouseGestureState;
use super::mouse::hit::HitRegions;
use super::msg::{Msg, ShellRequest, TerminalObserverEvent, TvHit};
use super::user_event::UserEvent;
#[cfg(test)]
use crate::app::layout::LayoutMain;
use crate::app::render::{effective_sort_str, letter_bucket, TvWideRenderCtx};
use crate::app::ui_util::{list_duration_secs, natural_sort_key};
#[cfg(test)]
use tuirealm::event::Key;

mod keyboard;
mod mouse;
mod navigation;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Pane {
    Series,
    Episodes,
}

pub struct TvWorkspaceComponent {
    context: TvWideRenderCtx,
    /// The one shared series-row owner, holding both the Wide and Inline
    /// presentations (design.md D12): a breakpoint flip moves the same
    /// owner between them, preserving only the outgoing selected-row
    /// viewport offset.
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
    layout: crate::app::layout::LayoutMain,
    /// The Wide frame's retained panel-skeleton geometry (task 8.2): the
    /// mounted component's own hit resolution and viewport arithmetic read
    /// the rects the shared skeleton painted.
    wide_geometry: Option<WideSkeletonGeometry>,
    /// The Wide hero image paint the shared skeleton retained (task 8.2): the
    /// shell paints the projected protocol into the reserved box after
    /// `view` returns (the same defer-the-pixel-paint seam the Library panel
    /// uses).
    panel_image_paint: Option<PanelHeroImagePaint>,
    viewport_height: usize,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
    /// Irregular Wide Episodes-pane chrome — season pills only (design.md
    /// D6), repopulated in `view()` from the geometry the wide-TV painter
    /// just produced. Both panes now have an embedded canonical control, so
    /// row identity comes from each control's retained current-frame
    /// geometry; the blank Episodes-pane fallback is resolved directly
    /// against the retained Wide skeleton geometry in `resolve_hit`.
    tv_chrome: HitRegions<TvHit>,
    /// The embedded Inline Search control (design.md D1). See
    /// `BrowserComponent::inline_search` for the migration-phase notes.
    inline_search: InlineSearch,
    /// The breakpoint the shell pushed for this frame (`App::
    /// wide_tv_library_area`): `true` paints the pane-based Wide workspace,
    /// `false` paints the flat Narrow series list. Defaults to `true` so a
    /// component built and viewed without an explicit push (existing unit
    /// tests) keeps painting the Wide workspace.
    is_wide: bool,
    /// Narrow-only letter-pill hit regions — last-push-wins rectangles
    /// (design.md D6), repopulated in `view()` from the pill rects the
    /// Narrow composer just painted into `self.layout.selector_tabs`.
    pill_regions: HitRegions<usize>,
}

/// Derives the Emby-specific semantic state for a Narrow series row (mirrors
/// `browser::emby_semantic_state`/`browser_content::emby_semantic_state`; the
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
                trailing: None,
                duration: list_duration_secs(episode.runtime_ticks / TICKS_PER_SECOND),
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            }
        })
        .collect()
}

impl TvWorkspaceComponent {
    pub fn new() -> Self {
        let context = TvWideRenderCtx::new(
            crate::app::render::LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
            None,
            None,
            0,
            None,
            false,
        );
        Self {
            context,
            carrier: MediaListCarrier::new(Presentation::Wide),
            season_cursor: 0,
            episodes: MediaListCarrier::new(Presentation::Wide),
            pane: Pane::Series,
            initialized: false,
            last_series_id: None,
            layout: Default::default(),
            wide_geometry: None,
            panel_image_paint: None,
            viewport_height: 1,
            mouse_gestures: MouseGestureState::new(),
            tv_chrome: HitRegions::new(),
            inline_search: InlineSearch::new(),
            is_wide: true,
            pill_regions: HitRegions::new(),
        }
    }

    /// Records the session-only Wide hero list-pane width override for the
    /// next `view()`. Pushed each frame by `render_tv_workspace_component`;
    /// it is a layout fact, not content, so it never enters the event-scoped
    /// `set_content` projection.
    pub(in crate::app) fn set_list_pane_width(&mut self, list_pane_width: Option<u16>) {
        self.context.list.list_pane_width = list_pane_width;
    }

    /// Records this frame's breakpoint (design.md D12): `true` selects the
    /// Wide pane-based workspace, `false` the flat Narrow series list. Must
    /// be pushed before `set_content` so the carrier's presentation switch
    /// (`ensure_carrier`) sees the current frame's breakpoint.
    pub(in crate::app) fn set_is_wide(&mut self, is_wide: bool) {
        self.is_wide = is_wide;
    }

    /// The presentation the shared owner holds for this frame's breakpoint.
    fn active_presentation(&self) -> Presentation {
        if self.is_wide {
            Presentation::Wide
        } else {
            Presentation::Inline
        }
    }

    /// Move the shared owner into the active presentation when they diverge.
    /// A responsive presentation change reads the same owner and preserves
    /// only the outgoing selected-row viewport offset (design.md D12); no
    /// cursor, scroll, or selection is ever copied between presentations.
    fn ensure_carrier(&mut self) {
        let target = self.active_presentation();
        let viewport_height = self.painted_viewport_height();
        self.carrier.set_presentation(target, viewport_height);
    }

    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
        self.ensure_carrier();
        let grouped = !context.list.is_search_active()
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
                    trailing: (item.production_year > 0).then(|| item.production_year.to_string()),
                    duration: None,
                    kind: MediaKind::Collection,
                    // Deliberate, known divergence (not a bug to unify away):
                    // Wide's series rail never dimmed on watched/played state
                    // pre-merge (legacy rail parity), so it stays
                    // `Ordinary` here. Narrow was painted by
                    // `BrowserComponent::project_rows` pre-merge, which did
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
        self.carrier.set_content(rows);
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

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
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
        self.episodes.set_content(rows);
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
            workspace: Some(Workspace {
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
    /// (`App::narrow_browse_extras`), mirroring `BrowserComponent::cursor`
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

    pub(in crate::app) fn painted_viewport_height(&self) -> usize {
        let painted = if self.is_wide {
            self.wide_geometry
                .as_ref()
                .map_or(0, |geometry| geometry.list_area.height) as usize
        } else {
            self.layout.left_area.height as usize
        };
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

    /// Take the Wide hero image paint the shared skeleton retained (task
    /// 8.2): the shell paints the projected protocol into its reserved box
    /// right after `view` returns (design D9).
    pub(in crate::app) fn take_panel_image_paint(&mut self) -> Option<PanelHeroImagePaint> {
        self.panel_image_paint.take()
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

    /// Handle a mouse event against the component's painted workspace
    /// geometry, dispatching by this frame's breakpoint (design.md D12).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Inline Search gets first refusal while active (design.md D6): it
        // is painted over the same area the series rail (Wide) or the
        // ordinary list (Narrow) would occupy, so neither ever mutates for
        // points there.
        if self.inline_search.is_active() {
            return match self.inline_search.handle_mouse(mouse) {
                Some(InlineSearchMouse::ContextMenu) => self
                    .inline_search
                    .selected_item()
                    .map(|item| Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item })),
                Some(InlineSearchMouse::Consumed) => {
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                None => None,
            };
        }
        // TV does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        if self.is_wide {
            self.handle_mouse_wide(mouse)
        } else {
            self.handle_mouse_narrow(mouse)
        }
    }

    /// Move the component's local pane + pane cursor to the clicked `hit`
    /// (Wide only). A click in the unfocused pane moves local focus there; a
    /// click in the already-focused pane keeps it. Clicking a season pill
    /// also selects that season; blank Episodes-pane space is consumed
    /// without changing the pane. Right-clicks never call this.
    fn apply_pane_click(&mut self, hit: TvHit, at: Position) {
        match hit {
            TvHit::SeasonTab(index) => {
                self.pane = Pane::Episodes;
                self.season_cursor = index;
                self.refresh_episode_rows();
                self.episodes.select_first();
            }
            TvHit::EpisodeRow(target) => {
                self.pane = Pane::Episodes;
                self.episodes
                    .delegate(RowLocalInput::Click(at), Some(target));
            }
            TvHit::SeriesRow(target) => {
                self.pane = Pane::Series;
                self.carrier
                    .delegate(RowLocalInput::Click(at), Some(target));
            }
            TvHit::EpisodesPane | TvHit::LetterPill(_) => {}
        }
    }

    /// Resolve a Wide workspace position to the pane + hit it lands in, from
    /// the component's own painted geometry. `None` = outside every TV rect
    /// (the clicks that remain unhandled).
    fn resolve_hit(&self, position: Position) -> Option<TvHit> {
        if let Some(hit) = self.tv_chrome.resolve(position).cloned() {
            return Some(hit);
        }
        if self.carrier.claims_current_point(position) {
            return self
                .carrier
                .resolve_current_point(position)
                .cloned()
                .map(TvHit::SeriesRow);
        }
        if self.episodes.claims_current_point(position) {
            return self
                .episodes
                .resolve_current_point(position)
                .cloned()
                .map(TvHit::EpisodeRow);
        }
        if self
            .wide_geometry
            .as_ref()
            .is_some_and(|geometry| geometry.hero.contains(position))
        {
            return Some(TvHit::EpisodesPane);
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn test_layout(&self) -> &LayoutMain {
        &self.layout
    }

    /// The last painted Wide skeleton geometry (task 8.2), for the
    /// panel-output test path.
    #[cfg(test)]
    pub(in crate::app) fn test_wide_geometry(&self) -> Option<&WideSkeletonGeometry> {
        self.wide_geometry.as_ref()
    }

    /// The season-pill hit regions the shared Workspace slot painted (task
    /// 8.2), for the season-click test path.
    #[cfg(test)]
    pub(crate) fn test_season_hits(&self) -> &HitRegions<TvHit> {
        &self.tv_chrome
    }

    #[cfg(test)]
    pub(crate) fn test_episode_claim_rect(&self) -> Option<Rect> {
        self.episodes.wide().current_claim_rect()
    }

    /// Test-only cursor seed (mirrors `BrowserComponent::set_cursor_for_test`):
    /// seeds the shared owner's stable target from a raw `context.list.items`
    /// index, for tests driving the merged component directly.
    #[cfg(test)]
    pub(crate) fn set_cursor_for_test(&mut self, cursor: usize) {
        if let Some(item) = self.context.list.items.get(cursor) {
            let target = item.id.clone();
            self.carrier.select_target(&target);
        }
    }

    /// Test-only: the shared owner's current rows' semantic states, in
    /// display order.
    #[cfg(test)]
    pub(crate) fn test_row_semantic_states(&self) -> Vec<MediaSemanticState> {
        self.carrier
            .rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item { semantic_state, .. } => Some(semantic_state.clone()),
                _ => None,
            })
            .collect()
    }
}

impl Default for TvWorkspaceComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineSearchHost for TvWorkspaceComponent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }

    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}

impl Component for TvWorkspaceComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.viewport_height = area.height as usize;
        self.ensure_carrier();
        self.layout = Default::default();
        self.wide_geometry = None;
        self.panel_image_paint = None;
        if self.is_wide {
            // The Library panel's shared Wide skeleton (task 8.2): the
            // component supplies typed content and keeps only its own painted
            // hit stores; the hero image is the shell-projected
            // `HeroImageState`, never a paint-time fetch (design D9).
            let browser_focused = self.context.focused && self.pane == Pane::Series;
            let list_pane_width = self.context.list.list_pane_width;
            let mut content = self.panel_content();
            let mut hits = SkeletonHits::default();
            if let Some(geometry) = render_wide_skeleton(
                frame,
                area,
                &mut content,
                browser_focused,
                list_pane_width,
                &mut hits,
            ) {
                self.panel_image_paint = geometry.hero_image.clone();
                self.wide_geometry = Some(geometry);
                // Adopt the season-pill chrome the shared Workspace slot
                // just produced into the irregular-chrome registry
                // (design.md D6); both list rows and the blank Episodes-pane
                // fallback resolve directly in `resolve_hit`.
                self.tv_chrome.clear();
                for (rect, index) in hits.workspace_selector.regions() {
                    self.tv_chrome.push(*rect, TvHit::SeasonTab(*index));
                }
                return;
            }
            // A Wide surface is owned exclusively by the Wide skeleton. If
            // it declines (for example during a transient geometry change),
            // clear its chrome and stop; never paint the Narrow skeleton over
            // a Wide surface (8.2 P2).
            self.tv_chrome.clear();
            return;
        }

        // Narrow uses the same panel content producer and the panel's shared
        // Inline skeleton as Wide. Episodes remain in the Workspace content
        // and are therefore reachable only through SelectionModal.
        let browser_focused = self.context.focused;
        let mut content = self.panel_content();
        let mut hits = SkeletonHits::default();
        let geometry =
            render_narrow_skeleton(frame, area, &mut content, browser_focused, &mut hits);
        self.panel_image_paint = geometry.inline_hero_image.clone();
        self.layout.left_area = geometry.list_area;
        self.layout.inline_hero_area = geometry.inline_hero.unwrap_or_default();
        self.layout.hero_area = geometry.inline_hero.unwrap_or_default();
        self.layout.selected_item_rect = geometry.selected;
        self.layout.selector_tabs = hits.selector.regions().to_vec();
        self.pill_regions.clear();
        for (rect, target) in hits.selector.regions() {
            self.pill_regions.push(*rect, *target);
        }
        self.tv_chrome.clear();
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.context.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for TvWorkspaceComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => {
                self.ensure_carrier();
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
                self.ensure_carrier();
                self.handle_mouse(mouse)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::LibraryListRenderCtx;
    use crate::app::tests::make_item;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::Component;
    use tuirealm::event::{Event, KeyEvent, KeyModifiers};

    /// Task 4.2d: the embedded episode `WideMediaList` field replaces the
    /// old `Option<usize>` episode cursor. This exercises the same
    /// component-local persistence through the canonical control -- moving
    /// the cursor via keyboard, then re-syncing the same series/season data,
    /// must preserve it (target-preserving `WideMediaList::set_content`).
    #[test]
    fn tv_workspace_keeps_episode_pane_cursor_local_between_syncs() {
        let mut component = TvWorkspaceComponent::new();
        component.set_focused(true);
        let mut series = make_item("Series", "Series");
        series.id = "series-id".into();
        let mut season = make_item("Season 1", "Season");
        season.id = "season-1".into();
        let episode = |name: &str, id: &str| {
            let mut item = make_item(name, "Episode");
            item.id = id.into();
            item
        };
        let detail = crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [(
                "season-1".into(),
                vec![
                    episode("Episode 1", "episode-1"),
                    episode("Episode 2", "episode-2"),
                ],
            )]
            .into_iter()
            .collect(),
        };
        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![series.clone()], 0, 0),
            Some(series.clone()),
            Some(detail.clone()),
            0,
            None,
            false,
        ));
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::NONE,
        }));
        let message = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::TvEpisodeMove { delta: 1 }))
        ));
        assert_eq!(component.episodes.cursor(), 1);

        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![series.clone()], 0, 0),
            Some(series),
            Some(detail),
            0,
            None,
            false,
        ));
        assert_eq!(component.episodes.cursor(), 1);
    }

    #[test]
    fn tv_workspace_series_change_resets_local_selection() {
        let mut component = TvWorkspaceComponent::new();
        component.set_focused(true);
        let mut season_one = make_item("Season 1", "Season");
        season_one.id = "season-1".into();
        let mut season_two = make_item("Season 2", "Season");
        season_two.id = "season-2".into();
        let detail = crate::app::SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes: std::collections::HashMap::new(),
        };
        let mut series_a = make_item("Series A", "Series");
        series_a.id = "series-a".into();
        let mut series_b = make_item("Series B", "Series");
        series_b.id = "series-b".into();

        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![series_a.clone()], 0, 0),
            Some(series_a),
            Some(detail.clone()),
            0,
            None,
            false,
        ));
        component.move_season(1);

        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![series_b.clone()], 0, 0),
            Some(series_b),
            Some(detail),
            0,
            None,
            false,
        ));

        assert_eq!(component.season_cursor, 0);
        assert!(component.episodes.is_empty());
        assert!(matches!(component.pane, Pane::Series));
    }

    #[test]
    fn tv_workspace_renders_the_wide_workspace_without_app() {
        let mut component = TvWorkspaceComponent::new();
        component.set_focused(true);
        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0, 0),
            None,
            None,
            0,
            None,
            false,
        ));
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        assert!(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() == "S"));
    }

    #[test]
    fn tv_workspace_renders_the_narrow_series_list_without_app() {
        let mut component = TvWorkspaceComponent::new();
        component.set_is_wide(false);
        component.set_focused(true);
        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0, 0),
            None,
            None,
            0,
            None,
            false,
        ));
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        assert!(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() == "S"));
    }
}
