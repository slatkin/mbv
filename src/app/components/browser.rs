//! Interactive Component for the generic Emby browser rows.
//!
//! The shell mirrors the active list source into this component. Generic,
//! Movies, and home-video rows use the existing typed render seam; music,
//! TV/series, and album-track presentation remain on their legacy branches
//! until their owning tasks convert them.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{BrowserHitRegion, LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::{render_generic_movies_home_video_rows_with_ctx, LibraryListRenderCtx};
use crate::app::ui_util::move_cursor;

pub struct BrowserComponent {
    context: LibraryListRenderCtx,
    cursor: usize,
    scroll: usize,
    focused: bool,
    initialized: bool,
    last_mirrored_cursor: usize,
    last_mirrored_scroll: usize,
    layout: LayoutMain,
}

impl BrowserComponent {
    pub fn new() -> Self {
        Self {
            context: LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
            cursor: 0,
            scroll: 0,
            focused: false,
            initialized: false,
            last_mirrored_cursor: 0,
            last_mirrored_scroll: 0,
            layout: LayoutMain::default(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: LibraryListRenderCtx, focused: bool) {
        if !self.initialized {
            self.cursor = context.cursor();
            self.scroll = context.scroll();
            self.initialized = true;
        } else {
            if self.cursor == self.last_mirrored_cursor {
                self.cursor = context.cursor();
            }
            if self.scroll == self.last_mirrored_scroll {
                self.scroll = context.scroll();
            }
        }
        self.context = context;
        self.cursor = self.cursor.min(self.context.item_count().saturating_sub(1));
        self.last_mirrored_cursor = self.context.cursor();
        self.last_mirrored_scroll = self.context.scroll();
        self.focused = focused;
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn handle_crossterm_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<Msg> {
        let count = self.context.item_count();
        match key.code {
            crossterm::event::KeyCode::Char('/') if key.modifiers.is_empty() => {
                return Some(Msg::Shell(super::msg::ShellRequest::OpenInlineSearch));
            }
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                self.cursor = move_cursor(self.cursor, -1, count)
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                self.cursor = move_cursor(self.cursor, 1, count)
            }
            crossterm::event::KeyCode::PageUp => {
                self.cursor = move_cursor(
                    self.cursor,
                    -(self.layout.left_area.height.max(1) as i64),
                    count,
                )
            }
            crossterm::event::KeyCode::PageDown => {
                self.cursor = move_cursor(
                    self.cursor,
                    self.layout.left_area.height.max(1) as i64,
                    count,
                )
            }
            crossterm::event::KeyCode::Home => self.cursor = 0,
            crossterm::event::KeyCode::End => self.cursor = count.saturating_sub(1),
            _ => {}
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Key(key)))
    }

    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        self.handle_crossterm_key(to_crossterm_key_event(key))
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let mouse = to_crossterm_mouse_event(mouse);
        let col = mouse.column;
        let row = mouse.row;
        let position: Position = (col, row).into();
        // The component owns *where* a browse click lands: it hit-tests
        // against its own painted geometry (`self.layout`, rebuilt every
        // `view`) and emits a typed `Msg::Shell` naming the region. It holds
        // no double-click or scroll timing — the shell decides *when* a click
        // counts against `App`'s own timing fields. Clicks outside every
        // browse rect are forwarded as a raw legacy event so `App::handle_mouse`
        // keeps handling the surrounding chrome (tabs, playback pills, queue,
        // the un-migrated tv/music surfaces).
        match mouse.kind {
            crossterm::event::MouseEventKind::ScrollDown
            | crossterm::event::MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, crossterm::event::MouseEventKind::ScrollUp)
                {
                    -1
                } else {
                    1
                };
                if self.layout.left_area.contains(position) {
                    return Some(Msg::Shell(ShellRequest::BrowserScroll { delta }));
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                // Selector-tab pills sit inside the left area; claim them
                // before the row-select hit-test.
                for (rect, target) in self.layout.selector_tabs.iter() {
                    if rect.contains(position) {
                        return Some(Msg::Shell(ShellRequest::BrowserClick {
                            region: BrowserHitRegion::SelectorTab(*target),
                            col,
                            row,
                        }));
                    }
                }
                if self.layout.left_area.contains(position)
                    || self.layout.inline_hero_area.contains(position)
                {
                    let region = if self.layout.inline_hero_area.contains(position) {
                        BrowserHitRegion::InlineHero(self.cursor)
                    } else {
                        BrowserHitRegion::LeftRow(self.cursor)
                    };
                    if let BrowserHitRegion::LeftRow(_) = region {
                        if let Some(cursor) = self.resolve_left_cursor(col, row) {
                            self.cursor = cursor;
                        }
                    }
                    return Some(Msg::Shell(ShellRequest::BrowserClick { region, col, row }));
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                if self.layout.left_area.contains(position)
                    || self.layout.inline_hero_area.contains(position)
                {
                    return Some(Msg::Shell(ShellRequest::BrowserClick {
                        region: BrowserHitRegion::ContextMenu(self.cursor),
                        col,
                        row,
                    }));
                }
            }
            _ => {}
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
    }

    /// Resolve the list item under `(col, row)` from the component's own
    /// `left_row_map` (single-column screen-row → item index). This is the
    /// local highlight only; the authoritative cursor set (two-column
    /// cell-target, header/gap no-ops, position save) stays in
    /// The `BrowserClick` shell arm consumes the resolved cursor target.
    fn resolve_left_cursor(&self, col: u16, row: u16) -> Option<usize> {
        let la = self.layout.left_area;
        if !la.contains((col, row).into()) {
            return None;
        }
        let click_y = (row - la.y) as usize;
        self.layout.left_row_map.get(click_y).copied().flatten()
    }
}

impl Default for BrowserComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for BrowserComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.layout = LayoutMain::default();
        let context = self
            .context
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll);
        self.scroll = render_generic_movies_home_video_rows_with_ctx(
            frame,
            area,
            &context,
            self.focused,
            &mut self.layout,
        );
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

impl AppComponent<Msg, UserEvent> for BrowserComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
