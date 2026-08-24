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
use super::msg::{LegacyTerminalEvent, Msg};
use super::user_event::UserEvent;
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
    last_mirrored_cursor: usize,
    last_mirrored_scroll: usize,
    last_mirrored_season: usize,
    last_mirrored_episode: Option<usize>,
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
            last_mirrored_cursor: 0,
            last_mirrored_scroll: 0,
            last_mirrored_season: 0,
            last_mirrored_episode: None,
            layout: Default::default(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: TvWideRenderCtx) {
        if !self.initialized {
            self.cursor = context.list.cursor();
            self.scroll = context.list.scroll();
            self.season_cursor = context.season_cursor;
            self.episode_cursor = context.episode_cursor;
            self.pane = if context.episode_cursor.is_some() {
                Pane::Episodes
            } else {
                Pane::Series
            };
            self.initialized = true;
        } else {
            if self.cursor == self.last_mirrored_cursor {
                self.cursor = context.list.cursor();
            }
            if self.scroll == self.last_mirrored_scroll {
                self.scroll = context.list.scroll();
            }
            if self.season_cursor == self.last_mirrored_season {
                self.season_cursor = context.season_cursor;
            }
            if self.episode_cursor == self.last_mirrored_episode {
                self.episode_cursor = context.episode_cursor;
                self.pane = if context.episode_cursor.is_some() {
                    Pane::Episodes
                } else {
                    Pane::Series
                };
            }
        }
        self.context = context;
        self.cursor = self
            .cursor
            .min(self.context.list.item_count().saturating_sub(1));
        self.last_mirrored_cursor = self.context.list.cursor();
        self.last_mirrored_scroll = self.context.list.scroll();
        self.last_mirrored_season = self.context.season_cursor;
        self.last_mirrored_episode = self.context.episode_cursor;
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
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

    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Left | Key::Char('h') => self.pane = Pane::Series,
            Key::Right | Key::Char('l') => self.pane = Pane::Episodes,
            Key::Up | Key::Char('k') if self.pane == Pane::Episodes => self.move_episode(-1),
            Key::Down | Key::Char('j') if self.pane == Pane::Episodes => self.move_episode(1),
            Key::Up | Key::Char('k') => {
                self.cursor = move_cursor(self.cursor, -1, self.context.list.item_count())
            }
            Key::Down | Key::Char('j') => {
                self.cursor = move_cursor(self.cursor, 1, self.context.list.item_count())
            }
            Key::Char('[') | Key::Char(']') => {}
            _ => {}
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Key(
            to_crossterm_key_event(key),
        )))
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let mouse = to_crossterm_mouse_event(mouse);
        if matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
            let position: ratatui::layout::Position = (mouse.column, mouse.row).into();
            if let Some((_, index)) = self
                .layout
                .tv_wide_episode_rows
                .iter()
                .find(|(rect, _)| rect.contains(position))
            {
                self.pane = Pane::Episodes;
                self.episode_cursor = Some(*index);
            } else if self.layout.tv_wide_right_area.contains(position) {
                self.pane = Pane::Series;
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
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
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
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
