//! Interactive Component for the wide Emby TV workspace.
//!
//! The shell mirrors the App-derived browser/detail snapshot. The component
//! keeps the active pane and the season/episode cursor used to paint the two
//! child targets; cross-authority effects use typed shell requests.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use mbv_core::api::EmbyItem;

use super::media_list::{MediaListRow, MediaSemanticState, WideMediaList};
use super::msg::{Msg, ShellRequest, TvHit, TvHitRegion};
use super::user_event::UserEvent;
#[cfg(test)]
use crate::app::layout::LayoutMain;
use crate::app::render::{
    effective_sort_str, letter_bucket, render_wide_tv_with_ctx, HomeImagePaint, TvWideRenderCtx,
};
use crate::app::ui_util::natural_sort_key;
#[cfg(test)]
use tuirealm::event::Key;

mod keyboard;
mod navigation;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Pane {
    Series,
    Episodes,
}

pub struct TvWorkspaceComponent {
    context: TvWideRenderCtx,
    list: WideMediaList<String>,
    cursor: usize,
    season_cursor: usize,
    episode_cursor: Option<usize>,
    pane: Pane,
    initialized: bool,
    last_series_id: Option<String>,
    layout: crate::app::layout::LayoutMain,
    image_paint: Option<HomeImagePaint>,
    pending_anchor: Option<super::media_list::ViewportAnchor<String>>,
    viewport_height: usize,
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
            false,
        );
        Self {
            context,
            list: WideMediaList::new(),
            cursor: 0,
            season_cursor: 0,
            episode_cursor: None,
            pane: Pane::Series,
            initialized: false,
            last_series_id: None,
            layout: Default::default(),
            image_paint: None,
            pending_anchor: None,
            viewport_height: 1,
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
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
                    semantic_state: if item.played {
                        MediaSemanticState::Played
                    } else {
                        MediaSemanticState::Ordinary
                    },
                }))
        });
        let rows = rows.collect::<Vec<_>>();
        // The canonical cursor is in the rendered (natural-sort) order. Seed
        // the local list from that order on first mount; thereafter preserve
        // the stable target already owned by the component.
        let restore_target = self.list.selected_target().cloned();
        self.list.set_content(rows);
        if !self.initialized {
            self.list.select_index(context.list.cursor());
        } else if let Some(target) = restore_target {
            self.list.select_target(&target);
        }
        let series_changed =
            context.selected_series.as_ref().map(|item| &item.id) != self.last_series_id.as_ref();
        if series_changed {
            self.season_cursor = 0;
            self.episode_cursor = None;
            self.pane = Pane::Series;
            self.last_series_id = context.selected_series.as_ref().map(|item| item.id.clone());
        }
        if !self.initialized {
            if !series_changed {
                self.season_cursor = context.season_cursor;
                self.episode_cursor = context.episode_cursor;
                self.pane = if context.episode_cursor.is_some() {
                    Pane::Episodes
                } else {
                    Pane::Series
                };
            }
            self.initialized = true;
        }
        self.context = context;
        self.cursor = self.list.cursor();
        let season_count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        self.season_cursor = self.season_cursor.min(season_count.saturating_sub(1));
        if let Some(episode_cursor) = self.episode_cursor {
            // Missing detail or episode data means the refresh is still loading;
            // do not discard the component-local selection in that interval.
            let Some(episodes) = self
                .context
                .series_detail
                .as_ref()
                .and_then(|detail| detail.seasons.get(self.season_cursor))
                .and_then(|season| {
                    self.context
                        .series_detail
                        .as_ref()?
                        .episodes
                        .get(&season.id)
                })
            else {
                return;
            };
            self.episode_cursor =
                (!episodes.is_empty()).then(|| episode_cursor.min(episodes.len() - 1));
        }
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.list.cursor()
    }

    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<super::media_list::ViewportAnchor<String>> {
        self.list.viewport_anchor(viewport_height)
    }

    pub(in crate::app) fn painted_viewport_height(&self) -> usize {
        self.viewport_height
    }

    /// Whether letter pills are enabled in the pushed context.
    pub(in crate::app) fn show_letter_pills(&self) -> bool {
        self.context.show_letter_pills
    }

    /// The scroll offset the component tracks for its series list. Read by
    /// the breakpoint hand-off so the resting `BrowseLevel` scroll matches
    /// the wide workspace before the narrow `BrowserComponent` adopts it.
    pub(in crate::app) fn scroll(&self) -> usize {
        self.list.scroll()
    }

    /// One-shot re-anchor of the series cursor/scroll to a shell-owned
    /// resting position (breakpoint hand-off, migrate-narrow-browse task 2.3
    /// / D5). Mirrors `MusicWorkspaceComponent::re_anchor`: an ordinary
    /// `set_content` keeps the component's divergent local cursor, so the
    /// shell re-anchors explicitly when the active-destination pointer flips
    /// back to this kept-mounted component.
    pub(in crate::app) fn apply_viewport_anchor(
        &mut self,
        anchor: super::media_list::ViewportAnchor<String>,
    ) {
        if self.list.select_target(&anchor.selected_target) {
            self.cursor = self.list.cursor();
        }
        self.pending_anchor = Some(anchor);
    }

    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    pub(in crate::app) fn selected_item_id(&self) -> Option<String> {
        let target = self.list.selected_target()?;
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
        items.get(self.list.cursor()).cloned().cloned()
    }

    /// The Series snapshot the shell pushed for this frame (`context
    /// .selected_series`), exposed so tests can verify the pushed detail
    /// follows the component's authoritative selection rather than the App
    /// browse cursor.
    pub(in crate::app) fn selected_series_snapshot(&self) -> Option<&EmbyItem> {
        self.context.selected_series.as_ref()
    }

    /// Return the component-owned selection needed to activate an episode.
    /// The shell uses these cursors to resolve the episode from App's cache;
    /// it never re-reads the library cursor for this action.
    pub(in crate::app) fn episode_activation_selection(&self) -> Option<(String, usize, usize)> {
        Some((
            self.context.selected_series.as_ref()?.id.clone(),
            self.season_cursor,
            self.episode_cursor?,
        ))
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

    /// The component owns *where* a TV event lands: it hit-tests its painted
    /// panes (`tv_wide_season_tabs`, `tv_wide_episode_rows`, and the two
    /// pane rects — all rebuilt every `view`) and resolves pane + hit into a
    /// typed region. A click in a pane moves the component's local focus
    /// there (its `pane` plus the pane cursors below); the shell decides
    /// *when* a click counts (App's 400ms double-click window, 30ms wheel
    /// throttle) via App's shared fields — the component holds no timing
    /// state.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let position: ratatui::layout::Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self.resolve_hit(position) {
                    // A click in the unfocused pane moves local focus there;
                    // a click in the already-focused pane keeps it (the hit
                    // only re-targets the pane's cursor). Clicking a season
                    // pill also selects that season; blank Episodes-pane
                    // space is consumed without changing the pane.
                    match hit {
                        TvHit::SeasonTab(index) => {
                            self.pane = Pane::Episodes;
                            self.season_cursor = index;
                            self.episode_cursor = Some(0);
                        }
                        TvHit::EpisodeRow(index) => {
                            self.pane = Pane::Episodes;
                            self.episode_cursor = Some(index);
                        }
                        TvHit::SeriesRow(index) => {
                            self.pane = Pane::Series;
                            self.cursor = index;
                        }
                        TvHit::EpisodesPane => {}
                    }
                    return Some(Msg::Shell(ShellRequest::TvClick {
                        region: TvHitRegion::Hit(hit),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(hit) = self.resolve_hit(position) {
                    // Right-click carries the same resolved pane + hit so
                    // the shell applies the pane-appropriate single-click
                    // effect before opening the menu; it never moves the
                    // component's pane or cursors.
                    return Some(Msg::Shell(ShellRequest::TvClick {
                        region: TvHitRegion::ContextMenu(hit),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                if self.layout.left_area.contains(position) =>
            {
                // Wheel scroll over the series list (`left_area` is the
                // right-pane list area this renderer publishes — the exact
                // region the legacy scroll arm hit-tested). The Episodes
                // pane has no wheel behaviour, so those scrolls remain
                // unhandled.
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    -1
                } else {
                    1
                };
                self.move_rows(delta);
                return Some(Msg::Shell(ShellRequest::TvScroll { delta }));
            }
            _ => {}
        }
        None
    }

    /// Resolve a workspace position to the pane + hit it lands in, from the
    /// component's own painted geometry. `None` = outside every TV rect
    /// (the clicks that remain unhandled).
    fn resolve_hit(&self, position: ratatui::layout::Position) -> Option<TvHit> {
        if let Some((_, index)) = self
            .layout
            .tv_wide_season_tabs
            .iter()
            .find(|(rect, _)| rect.contains(position))
        {
            return Some(TvHit::SeasonTab(*index));
        }
        if let Some((_, index)) = self
            .layout
            .tv_wide_episode_rows
            .iter()
            .find(|(rect, _)| rect.contains(position))
        {
            return Some(TvHit::EpisodeRow(*index));
        }
        if self.layout.tv_wide_left_area.contains(position) {
            return Some(TvHit::EpisodesPane);
        }
        if self.layout.tv_wide_right_area.contains(position) {
            // Resolve the series row under the click from the painted
            // `left_row_map` relative to the painted series list (None for a
            // header/gap cell → keep the current series cursor, matching the
            // legacy blank-space click no-op).
            let click_y = (position.y.saturating_sub(self.layout.tv_wide_list_area.y)) as usize;
            let target = self
                .layout
                .left_row_map
                .get(click_y)
                .copied()
                .flatten()
                .unwrap_or(self.cursor);
            return Some(TvHit::SeriesRow(target));
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn test_layout(&self) -> &LayoutMain {
        &self.layout
    }
}

impl Default for TvWorkspaceComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TvWorkspaceComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.viewport_height = area.height as usize;
        self.layout = Default::default();
        self.image_paint = None;
        if let Some(anchor) = self.pending_anchor.take() {
            self.list
                .apply_viewport_anchor(&anchor, area.height as usize);
        }
        let context = self.context.clone().with_local_state(
            self.list.cursor(),
            self.list.scroll(),
            self.season_cursor,
            self.episode_cursor,
        );
        let (scroll, image_paint) =
            render_wide_tv_with_ctx(frame, area, &context, &mut self.layout, &self.list);
        self.list.set_scroll(scroll);
        self.cursor = self.list.cursor();
        self.image_paint = image_paint;
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

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
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
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

    #[test]
    fn tv_workspace_keeps_episode_pane_cursor_local_between_syncs() {
        let mut component = TvWorkspaceComponent::new();
        let mut list = LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0, 0);
        list = list.with_cursor_scroll(0, 0);
        component.set_content(TvWideRenderCtx::new(
            list,
            None,
            None,
            0,
            Some(0),
            true,
            false,
        ));
        let message = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::TvEpisodeMove { delta: 1 }))
        ));
        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0, 0),
            None,
            None,
            0,
            Some(0),
            true,
            false,
        ));
        assert_eq!(component.episode_cursor, Some(0));
    }

    #[test]
    fn tv_workspace_series_change_resets_local_selection() {
        let mut component = TvWorkspaceComponent::new();
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
            true,
            false,
        ));
        component.move_season(1);

        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![series_b.clone()], 0, 0),
            Some(series_b),
            Some(detail),
            0,
            None,
            true,
            false,
        ));

        assert_eq!(component.season_cursor, 0);
        assert!(component.episode_cursor.is_none());
        assert!(matches!(component.pane, Pane::Series));
    }

    #[test]
    fn tv_workspace_renders_the_wide_workspace_without_app() {
        let mut component = TvWorkspaceComponent::new();
        component.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0, 0),
            None,
            None,
            0,
            None,
            true,
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
