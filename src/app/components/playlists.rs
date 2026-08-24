use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_playlists_content, PlaylistsRenderGeometry};
use mbv_core::api::EmbyItem;

pub struct PlaylistsComponent {
    playlists: Vec<EmbyItem>,
    cursor: usize,
    scroll: usize,
    loading: bool,
    open: Option<EmbyItem>,
    open_items: Vec<EmbyItem>,
    open_cursor: usize,
    open_scroll: usize,
    open_loading: bool,
    loaded_id: Option<String>,
    panel_area: Option<Rect>,
    geometry: PlaylistsRenderGeometry,
}

impl PlaylistsComponent {
    pub fn new() -> Self {
        Self {
            playlists: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
            open: None,
            open_items: Vec::new(),
            open_cursor: 0,
            open_scroll: 0,
            open_loading: false,
            loaded_id: None,
            panel_area: None,
            geometry: PlaylistsRenderGeometry::default(),
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        playlists: Vec<EmbyItem>,
        cursor: usize,
        scroll: usize,
        loading: bool,
        open: Option<EmbyItem>,
        open_items: Vec<EmbyItem>,
        open_cursor: usize,
        open_scroll: usize,
        open_loading: bool,
        loaded_id: Option<String>,
    ) {
        self.playlists = playlists;
        self.cursor = self
            .cursor
            .max(cursor)
            .min(self.playlists.len().saturating_sub(1));
        self.scroll = self.scroll.max(scroll).min(self.cursor);
        self.loading = loading;
        self.open = open;
        self.open_items = open_items;
        self.open_cursor = self
            .open_cursor
            .max(open_cursor)
            .min(self.open_items.len().saturating_sub(1));
        self.open_scroll = self.open_scroll.max(open_scroll).min(self.open_cursor);
        self.open_loading = open_loading;
        self.loaded_id = loaded_id;
    }

    pub(in crate::app) fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }
    pub(in crate::app) fn open_cursor(&self) -> usize {
        self.open_cursor
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Up => {
                if self.open.is_some() {
                    self.open_cursor = self.open_cursor.saturating_sub(1);
                } else {
                    self.cursor = self.cursor.saturating_sub(1);
                }
            }
            Key::Down => {
                if self.open.is_some() {
                    self.open_cursor =
                        (self.open_cursor + 1).min(self.open_items.len().saturating_sub(1));
                } else {
                    self.cursor = (self.cursor + 1).min(self.playlists.len().saturating_sub(1));
                }
            }
            Key::PageUp => self.move_page(-1),
            Key::PageDown => self.move_page(1),
            Key::Home => {
                if self.open.is_some() {
                    self.open_cursor = 0;
                } else {
                    self.cursor = 0;
                }
            }
            Key::End => {
                if self.open.is_some() {
                    self.open_cursor = self.open_items.len().saturating_sub(1);
                } else {
                    self.cursor = self.playlists.len().saturating_sub(1);
                }
            }
            Key::Left if self.open.is_some() => {
                self.open = None;
                self.open_items.clear();
            }
            Key::Esc | Key::Backspace if self.open.is_some() => {
                self.open = None;
                self.open_items.clear();
            }
            _ => {}
        }
        Some(Msg::Shell(ShellRequest::PlaylistsKey(
            to_crossterm_key_event(key),
        )))
    }

    fn move_page(&mut self, direction: i64) {
        let page = self.geometry.content_area.height.saturating_sub(4) as i64;
        if self.open.is_some() {
            let last = self.open_items.len().saturating_sub(1) as i64;
            self.open_cursor = (self.open_cursor as i64 + direction * page).clamp(0, last) as usize;
        } else {
            let last = self.playlists.len().saturating_sub(1) as i64;
            self.cursor = (self.cursor as i64 + direction * page).clamp(0, last) as usize;
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let mouse = to_crossterm_mouse_event(mouse);
        let position: Position = (mouse.column, mouse.row).into();
        if matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
            if let Some((open, index)) = self.geometry.hit_test(position) {
                if open {
                    self.open_cursor = index;
                } else {
                    self.cursor = index;
                }
            }
        }
        Some(Msg::Shell(ShellRequest::PlaylistsMouse(mouse)))
    }
}

impl Default for PlaylistsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PlaylistsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        render_playlists_content(
            frame,
            area,
            self.panel_area,
            &self.playlists,
            &mut self.cursor,
            &mut self.scroll,
            self.loading,
            self.open.as_ref(),
            &self.open_items,
            &mut self.open_cursor,
            &mut self.open_scroll,
            self.open_loading,
            self.loaded_id.as_deref(),
            &mut self.geometry,
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

impl AppComponent<Msg, UserEvent> for PlaylistsComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
