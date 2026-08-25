//! Interactive Component for Search inside one Emby browser.
//!
//! The shell supplies the validated, ordered candidate pool. Plain browser
//! searches use `Items`; recursive music searches use `Albums`. App retains
//! the search worker, album index, and activation effects until group 5.
//! The shell passes the placement rect at `view` time (the deleted per-frame
//! `sync_inline_search` area mirror); the component paints only into that
//! rect, exactly like every other mounted component.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyModifiers, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::to_crossterm_mouse_event;
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_generic_movies_home_video_rows_with_ctx, LibraryListRenderCtx};
use crate::app::ui_util::move_cursor;

#[derive(Clone)]
pub(in crate::app) enum SearchPool {
    Items(Vec<mbv_core::api::EmbyItem>),
    Albums(Vec<crate::app::AlbumSearchEntry>),
}

impl SearchPool {
    fn filtered_items(&self, query: &str) -> Vec<mbv_core::api::EmbyItem> {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        if query.chars().count() < 2 {
            return Vec::new();
        }
        let matcher = SkimMatcherV2::default();
        match self {
            Self::Items(items) => items
                .iter()
                .filter(|item| matcher.fuzzy_match(&item.display_name(), query).is_some())
                .cloned()
                .collect(),
            Self::Albums(entries) => entries
                .iter()
                .filter(|entry| matcher.fuzzy_match(&entry.search_text, query).is_some())
                .map(|entry| {
                    let mut item = entry.album.clone();
                    item.name = entry.display_label.clone();
                    item
                })
                .collect(),
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
    layout: crate::app::layout::LayoutMain,
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
            layout: Default::default(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, pool: SearchPool, loading: bool, focused: bool) {
        self.pool = pool;
        if loading {
            self.loading = true;
        }
        self.cursor = self.cursor.min(
            self.pool
                .filtered_items(&self.query)
                .len()
                .saturating_sub(1),
        );
        self.focused = focused;
    }

    pub(in crate::app) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub(in crate::app) fn selected_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.pool
            .filtered_items(&self.query)
            .get(self.cursor)
            .cloned()
    }

    fn move_cursor(&mut self, delta: i64) {
        self.cursor = move_cursor(
            self.cursor,
            delta,
            self.pool.filtered_items(&self.query).len(),
        );
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
            Key::End => {
                self.cursor = self
                    .pool
                    .filtered_items(&self.query)
                    .len()
                    .saturating_sub(1)
            }
            Key::Enter => {
                if let Some(item) = self.selected_item() {
                    return Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id: item.id,
                        item_type: item.item_type,
                    }));
                }
            }
            Key::Esc => return Some(Msg::Shell(ShellRequest::InlineSearchDismiss)),
            Key::Char(c) => {
                self.query.push(c);
                self.cursor = 0;
                self.scroll = 0;
            }
            Key::Backspace => {
                self.query.pop();
                self.cursor = 0;
                self.scroll = 0;
            }
            _ => {}
        }
        Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
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
                self.cursor = move_cursor(row, 0, self.pool.filtered_items(&self.query).len());
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
    }
}

impl Default for InlineSearchComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for InlineSearchComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.layout = Default::default();
        let items = self.pool.filtered_items(&self.query);
        let context = LibraryListRenderCtx::from_items(items, self.cursor, self.scroll)
            .with_search(self.query.clone(), self.loading);
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
        let items = vec![make_item("One", "Movie"), make_item("Only", "Movie")];
        component.set_content(SearchPool::Items(items.clone()), false, true);
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('x'),
            modifiers: KeyModifiers::NONE,
        }));
        component.query = "on".into();
        component.set_content(SearchPool::Items(items), false, true);
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.query, "on");
        assert_eq!(component.cursor, 1);
    }

    #[test]
    fn inline_library_search_renders_plain_candidates_without_app() {
        let mut component = InlineSearchComponent::new();
        component.query = "one".into();
        component.set_content(
            SearchPool::Items(vec![make_item("One", "Movie")]),
            false,
            true,
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

    #[test]
    fn inline_library_search_enter_emits_activation_message() {
        let mut component = InlineSearchComponent::new();
        let item = make_item("One", "Movie");
        component.query = "one".into();
        component.set_content(SearchPool::Items(vec![item.clone()]), false, true);

        let message = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));

        assert_eq!(
            message,
            Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                id: item.id,
                item_type: item.item_type,
            }))
        );
    }
}
