//! Interactive Component for Search inside one Emby browser.
//!
//! The shell supplies the validated, ordered candidate pool. Plain browser
//! searches use `Items`; recursive music searches use `Albums`. App retains
//! the search worker, album index, and activation effects until group 5.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyModifiers, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg};
use super::user_event::UserEvent;
use crate::app::render::{render_generic_movies_home_video_rows_with_ctx, LibraryListRenderCtx};
use crate::app::ui_util::move_cursor;

#[derive(Clone)]
pub(in crate::app) enum SearchPool {
    Items(Vec<mbv_core::api::EmbyItem>),
    Albums(Vec<crate::app::AlbumSearchEntry>),
}

impl SearchPool {
    fn items(&self) -> Vec<mbv_core::api::EmbyItem> {
        match self {
            Self::Items(items) => items.clone(),
            Self::Albums(entries) => entries
                .iter()
                .map(|entry| {
                    let mut item = entry.album.clone();
                    item.name = entry.display_label.clone();
                    item
                })
                .collect(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Items(items) => items.len(),
            Self::Albums(entries) => entries.len(),
        }
    }
}

pub struct InlineSearchComponent {
    query: String,
    pool: SearchPool,
    loading: bool,
    cursor: usize,
    scroll: usize,
    focused: bool,
    area: Rect,
    layout: crate::app::layout::LayoutMain,
    initialized: bool,
    last_mirrored_query: String,
    last_mirrored_cursor: usize,
    last_mirrored_scroll: usize,
}

impl InlineSearchComponent {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            pool: SearchPool::Items(Vec::new()),
            loading: false,
            cursor: 0,
            scroll: 0,
            focused: false,
            area: Rect::default(),
            layout: Default::default(),
            initialized: false,
            last_mirrored_query: String::new(),
            last_mirrored_cursor: 0,
            last_mirrored_scroll: 0,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        query: String,
        pool: SearchPool,
        loading: bool,
        cursor: usize,
        scroll: usize,
        focused: bool,
        area: Rect,
    ) {
        if !self.initialized {
            self.query = query.clone();
            self.cursor = cursor;
            self.scroll = scroll;
            self.initialized = true;
        } else {
            if self.query == self.last_mirrored_query {
                self.query = query.clone();
            }
            if self.cursor == self.last_mirrored_cursor {
                self.cursor = cursor;
            }
            if self.scroll == self.last_mirrored_scroll {
                self.scroll = scroll;
            }
        }
        self.pool = pool;
        self.loading = loading;
        self.cursor = self.cursor.min(self.pool.len().saturating_sub(1));
        self.focused = focused;
        self.area = area;
        self.last_mirrored_query = query;
        self.last_mirrored_cursor = cursor;
        self.last_mirrored_scroll = scroll;
    }

    fn move_cursor(&mut self, delta: i64) {
        self.cursor = move_cursor(self.cursor, delta, self.pool.len());
    }

    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        if key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL)
        {
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        }
        match key.code {
            Key::Up => self.move_cursor(-1),
            Key::Down => self.move_cursor(1),
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.pool.len().saturating_sub(1),
            Key::Char(c) => self.query.push(c),
            Key::Backspace => {
                self.query.pop();
            }
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
            if self.layout.left_area.contains(position) {
                let row = position.y.saturating_sub(self.layout.left_area.y) as usize;
                self.cursor = move_cursor(row, 0, self.pool.len());
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
    }
}

impl Default for InlineSearchComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for InlineSearchComponent {
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        self.layout = Default::default();
        let context = LibraryListRenderCtx::from_items(self.pool.items(), self.cursor, self.scroll)
            .with_search(self.query.clone(), self.loading);
        self.scroll = render_generic_movies_home_video_rows_with_ctx(
            frame,
            self.area,
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

impl AppComponent<Msg, UserEvent> for InlineSearchComponent {
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
    use crate::app::tests::make_item;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::Component;
    use tuirealm::event::{Event, KeyEvent, KeyModifiers};

    #[test]
    fn inline_library_search_query_and_cursor_survive_shell_mirrors() {
        let mut component = InlineSearchComponent::new();
        let items = vec![make_item("One", "Movie"), make_item("Two", "Movie")];
        component.set_content(
            "one".into(),
            SearchPool::Items(items.clone()),
            false,
            0,
            0,
            true,
            Rect::new(0, 0, 40, 5),
        );
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('x'),
            modifiers: KeyModifiers::NONE,
        }));
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        component.set_content(
            "one".into(),
            SearchPool::Items(items),
            false,
            0,
            0,
            true,
            Rect::new(0, 0, 40, 5),
        );
        assert_eq!(component.query, "onex");
        assert_eq!(component.cursor, 1);
    }

    #[test]
    fn inline_library_search_renders_plain_candidates_without_app() {
        let mut component = InlineSearchComponent::new();
        component.set_content(
            "one".into(),
            SearchPool::Items(vec![make_item("One", "Movie")]),
            false,
            0,
            0,
            true,
            Rect::new(0, 0, 40, 5),
        );
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        assert!(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() == "O"));
    }
}
