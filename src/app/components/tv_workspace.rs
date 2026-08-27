//! Interactive Component for the wide Emby TV workspace.
//!
//! The shell mirrors the App-derived browser/detail snapshot. The component
//! keeps the active pane and the season/episode cursor used to paint the two
//! child targets; legacy keys still forward to App during stage 1.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest, TvHit, TvHitRegion};
use super::user_event::UserEvent;
#[cfg(test)]
use crate::app::layout::LayoutMain;
use crate::app::render::{render_wide_tv_with_ctx, TvWideRenderCtx};
use crate::app::ui_util::move_cursor;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Pane {
    Series,
    Episodes,
}

pub struct TvWorkspaceComponent {
    context: TvWideRenderCtx,
    cursor: usize,
    scroll: usize,
    season_cursor: usize,
    episode_cursor: Option<usize>,
    pane: Pane,
    initialized: bool,
    last_series_id: Option<String>,
    layout: crate::app::layout::LayoutMain,
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
            cursor: 0,
            scroll: 0,
            season_cursor: 0,
            episode_cursor: None,
            pane: Pane::Series,
            initialized: false,
            last_series_id: None,
            layout: Default::default(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
        let series_changed =
            context.selected_series.as_ref().map(|item| &item.id) != self.last_series_id.as_ref();
        if series_changed {
            self.season_cursor = 0;
            self.episode_cursor = None;
            self.pane = Pane::Series;
            self.last_series_id = context.selected_series.as_ref().map(|item| item.id.clone());
        }
        if !self.initialized {
            self.cursor = context.list.cursor();
            self.scroll = context.list.scroll();
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
        self.cursor = self
            .cursor
            .min(self.context.list.item_count().saturating_sub(1));
        let season_count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        self.season_cursor = self.season_cursor.min(season_count.saturating_sub(1));
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn selected_item_id(&self) -> Option<String> {
        self.context
            .list
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll)
            .selected_item()
            .map(|item| item.id.clone())
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

    fn move_episode(&mut self, delta: i64) {
        let count = self
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
            .map_or(0, Vec::len);
        if count > 0 {
            let cursor = self.episode_cursor.unwrap_or(0);
            self.episode_cursor = Some(move_cursor(cursor, delta, count));
        }
    }

    fn move_season(&mut self, delta: i64) {
        let count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        if count > 0 {
            self.season_cursor = move_cursor(self.season_cursor, delta, count);
            self.episode_cursor = Some(0);
        }
    }

    /// Move through the rows painted by the last frame. Grouped TV lists
    /// publish their sorted order and row map just like BrowserComponent;
    /// using raw item indices here would make the local cursor disagree with
    /// the App's letter-group navigation.
    fn move_rows(&mut self, rows: i64) {
        if !self.layout.left_sorted_indices.is_empty() {
            if let Some(delta) = self.letter_vertical_delta(rows) {
                self.move_cursor_delta(delta);
                return;
            }
        }
        self.move_cursor_delta(rows);
    }

    fn move_cursor_delta(&mut self, delta: i64) {
        if !self.layout.left_sorted_indices.is_empty() {
            let sorted = &self.layout.left_sorted_indices;
            let position = sorted
                .iter()
                .position(|&index| index == self.cursor)
                .unwrap_or(0);
            self.cursor = sorted[move_cursor(position, delta, sorted.len())];
        } else if self.context.list.item_count() > 0 {
            self.cursor = move_cursor(self.cursor, delta, self.context.list.item_count());
        }
    }

    fn letter_vertical_delta(&self, rows: i64) -> Option<i64> {
        let item_rows: Vec<&Vec<usize>> = self
            .layout
            .left_item_rows
            .iter()
            .filter(|row| !row.is_empty())
            .collect();
        if item_rows.is_empty() || self.layout.left_sorted_indices.is_empty() {
            return None;
        }
        let (current_row, current_col) =
            item_rows.iter().enumerate().find_map(|(row, items)| {
                items
                    .iter()
                    .position(|&index| index == self.cursor)
                    .map(|col| (row, col))
            })?;
        let target_row = if rows < 0 {
            current_row.saturating_sub(rows.unsigned_abs() as usize)
        } else {
            current_row
                .saturating_add(rows as usize)
                .min(item_rows.len().saturating_sub(1))
        };
        let target = item_rows[target_row]
            .get(current_col)
            .copied()
            .or_else(|| item_rows[target_row].last().copied())?;
        let sorted = &self.layout.left_sorted_indices;
        let current_position = sorted.iter().position(|&index| index == self.cursor)?;
        let target_position = sorted.iter().position(|&index| index == target)?;
        Some(target_position as i64 - current_position as i64)
    }

    fn jump_cursor(&mut self, to_end: bool) {
        if let Some(sorted) = (!self.layout.left_sorted_indices.is_empty())
            .then_some(&self.layout.left_sorted_indices)
        {
            self.cursor = sorted[if to_end { sorted.len() - 1 } else { 0 }];
        } else if self.context.list.item_count() > 0 {
            self.cursor = if to_end {
                self.context.list.item_count() - 1
            } else {
                0
            };
        }
    }

    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        if !self.context.focused {
            return Some(Msg::Legacy(LegacyTerminalEvent::Key(
                to_crossterm_key_event(key),
            )));
        }
        let request = match key.code {
            Key::Left | Key::Char('h') => {
                self.pane = Pane::Series;
                Some(ShellRequest::TvMoveColumn { delta: -1 })
            }
            Key::Right | Key::Char('l') => {
                self.pane = Pane::Episodes;
                Some(ShellRequest::TvMoveColumn { delta: 1 })
            }
            Key::Enter if self.pane == Pane::Series => {
                self.episode_cursor = Some(0);
                self.pane = Pane::Episodes;
                Some(ShellRequest::TvActivate)
            }
            Key::Enter => Some(ShellRequest::TvEpisodeActivate),
            Key::Esc | Key::Backspace => {
                if self.episode_cursor.is_some() {
                    self.episode_cursor = None;
                    self.pane = Pane::Series;
                }
                Some(ShellRequest::TvBack)
            }
            Key::Up | Key::Char('k') if self.pane == Pane::Episodes => {
                self.move_episode(-1);
                Some(ShellRequest::TvEpisodeMove { delta: -1 })
            }
            Key::Down | Key::Char('j') if self.pane == Pane::Episodes => {
                self.move_episode(1);
                Some(ShellRequest::TvEpisodeMove { delta: 1 })
            }
            Key::Char('[')
                if self.pane == Pane::Episodes
                    && !key
                        .modifiers
                        .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.move_season(-1);
                Some(ShellRequest::TvSeasonMove { delta: -1 })
            }
            Key::Char(']')
                if self.pane == Pane::Episodes
                    && !key
                        .modifiers
                        .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.move_season(1);
                Some(ShellRequest::TvSeasonMove { delta: 1 })
            }
            Key::Up | Key::Char('k') => {
                self.move_rows(-1);
                Some(ShellRequest::TvMoveRows { rows: -1 })
            }
            Key::Down | Key::Char('j') => {
                self.move_rows(1);
                Some(ShellRequest::TvMoveRows { rows: 1 })
            }
            Key::PageUp => {
                let rows = -(self
                    .layout
                    .tv_wide_list_area
                    .height
                    .saturating_sub(1)
                    .max(1) as i64);
                self.move_rows(rows);
                Some(ShellRequest::TvMoveRows { rows })
            }
            Key::PageDown => {
                let rows = self
                    .layout
                    .tv_wide_list_area
                    .height
                    .saturating_sub(1)
                    .max(1) as i64;
                self.move_rows(rows);
                Some(ShellRequest::TvMoveRows { rows })
            }
            Key::Home => {
                self.jump_cursor(false);
                Some(ShellRequest::TvJumpCursor { to_end: false })
            }
            Key::End => {
                self.jump_cursor(true);
                Some(ShellRequest::TvJumpCursor { to_end: true })
            }
            Key::Char(c @ ('[' | ']'))
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                Some(ShellRequest::TvCycleLetterPill {
                    delta: if c == '[' { -1 } else { 1 },
                })
            }
            _ => None,
        };
        request.map_or_else(
            || {
                Some(Msg::Legacy(LegacyTerminalEvent::Key(
                    to_crossterm_key_event(key),
                )))
            },
            |request| Some(Msg::Shell(request)),
        )
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
        let mouse = to_crossterm_mouse_event(mouse);
        let position: ratatui::layout::Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
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
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                if let Some(hit) = self.resolve_hit(position) {
                    // Right-click carries the same resolved pane + hit so
                    // the shell applies the pane-appropriate single-click
                    // effect before opening the menu; it never moves the
                    // component's pane or cursors (mirroring the legacy
                    // right-click arm, which only ever opened the menu).
                    return Some(Msg::Shell(ShellRequest::TvClick {
                        region: TvHitRegion::ContextMenu(hit),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            crossterm::event::MouseEventKind::ScrollUp
            | crossterm::event::MouseEventKind::ScrollDown
                if self.layout.left_area.contains(position) =>
            {
                // Wheel scroll over the series list (`left_area` is the
                // right-pane list area this renderer publishes — the exact
                // region the legacy scroll arm hit-tested). The Episodes
                // pane has no legacy wheel behaviour, so those scrolls stay
                // legacy-forwarded (where they no-op).
                let delta: i64 = if matches!(mouse.kind, crossterm::event::MouseEventKind::ScrollUp)
                {
                    -1
                } else {
                    1
                };
                self.move_rows(delta);
                return Some(Msg::Shell(ShellRequest::TvScroll { delta }));
            }
            _ => {}
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
    }

    /// Resolve a workspace position to the pane + hit it lands in, from the
    /// component's own painted geometry. `None` = outside every TV rect
    /// (the clicks that stay `Msg::Legacy`).
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
        self.layout = Default::default();
        let context = self.context.clone().with_local_state(
            self.cursor,
            self.scroll,
            self.season_cursor,
            self.episode_cursor,
        );
        self.scroll = render_wide_tv_with_ctx(frame, area, &context, &mut self.layout);
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
