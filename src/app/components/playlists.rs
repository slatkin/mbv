use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_playlists_content, PlaylistsRenderGeometry, PlaylistsViewState};
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
    /// Irregular painted chrome (task 5.2, design.md D6): playlist rows and
    /// open-playlist item rows, repopulated in `view()` from the geometry
    /// the painter just produced. Tag = `(open, row index)`; open rows are
    /// pushed last so they win overlaps, matching the retired
    /// `PlaylistsRenderGeometry::hit_test` ordering.
    hit_rows: HitRegions<(bool, usize)>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    /// Owns the double-click window the hand-rolled `last_click` field used
    /// to keep.
    mouse_gestures: MouseGestureState,
}

/// Owned snapshot of playlist state, handed to the component whenever the
/// shell refreshes it. Grouped into one value because the fields always
/// travel together and the component mirrors them all.
pub(in crate::app) struct PlaylistsContent {
    pub playlists: Vec<EmbyItem>,
    pub cursor: usize,
    pub scroll: usize,
    pub loading: bool,
    pub open: Option<EmbyItem>,
    pub open_items: Vec<EmbyItem>,
    pub open_cursor: usize,
    pub open_scroll: usize,
    pub open_loading: bool,
    pub loaded_id: Option<String>,
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
            hit_rows: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    pub(in crate::app) fn set_content(&mut self, content: PlaylistsContent) {
        let PlaylistsContent {
            playlists,
            cursor,
            scroll,
            loading,
            open,
            open_items,
            open_cursor,
            open_scroll,
            open_loading,
            loaded_id,
        } = content;
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

    fn local_change() -> Option<Msg> {
        None
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
                return Some(Msg::Shell(ShellRequest::PlaylistsBack));
            }
            Key::Esc | Key::Backspace | Key::Function(4) if self.open.is_some() => {
                self.open = None;
                self.open_items.clear();
                return Some(Msg::Shell(ShellRequest::PlaylistsBack));
            }
            Key::Esc | Key::Function(4) => {
                return Some(Msg::Shell(ShellRequest::DismissPlaylists));
            }
            Key::Function(2) => return Some(Msg::Shell(ShellRequest::OpenSettings)),
            Key::Function(3) => return Some(Msg::Shell(ShellRequest::OpenSessions)),
            Key::Char('q') if key.modifiers.is_empty() => {
                return Some(Msg::Shell(ShellRequest::Quit));
            }
            Key::Right if self.open.is_none() => {
                return (self.cursor < self.playlists.len())
                    .then_some(Msg::Shell(ShellRequest::PlaylistsOpen(self.cursor)));
            }
            Key::Enter => {
                let open = self.open.is_some();
                let index = if open { self.open_cursor } else { self.cursor };
                return Some(Msg::Shell(ShellRequest::PlaylistsActivate { open, index }));
            }
            Key::Char('n') if key.modifiers.is_empty() && self.open.is_none() => {
                return (self.cursor < self.playlists.len())
                    .then_some(Msg::Shell(ShellRequest::PlaylistsRename(self.cursor)));
            }
            Key::Char('d') if key.modifiers.is_empty() && self.open.is_none() => {
                return (self.cursor < self.playlists.len())
                    .then_some(Msg::Shell(ShellRequest::PlaylistsDelete(self.cursor)));
            }
            Key::Char('r') => {
                if self.open.is_some() {
                    self.open = None;
                    self.open_items.clear();
                }
                return Some(Msg::Shell(ShellRequest::PlaylistsRefresh));
            }
            _ => return Self::local_change(),
        }
        Self::local_change()
    }

    fn move_page(&mut self, direction: i64) {
        let page = self.geometry.panel_area.height.saturating_sub(4) as i64;
        if self.open.is_some() {
            let last = self.open_items.len().saturating_sub(1) as i64;
            self.open_cursor = (self.open_cursor as i64 + direction * page).clamp(0, last) as usize;
        } else {
            let last = self.playlists.len().saturating_sub(1) as i64;
            self.cursor = (self.cursor as i64 + direction * page).clamp(0, last) as usize;
        }
    }

    /// Mouse handling (task 5.2): recognition via the component's own
    /// `MouseGestureState` (ADR 0024, design.md D3) — including the
    /// double-click window the hand-rolled `last_click` field used to own;
    /// row geometry via `HitRegions` (D6). Behaviour unchanged from the
    /// ad-hoc handler: the wheel steps the visible list's cursor, a
    /// right-click on an open playlist goes back, an outside click
    /// dismisses, a row click selects, and a double click activates (the
    /// Enter equivalent).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { delta, .. } => {
                if self.open.is_some() {
                    self.open_cursor = if delta < 0 {
                        self.open_cursor.saturating_sub(1)
                    } else {
                        (self.open_cursor + 1).min(self.open_items.len().saturating_sub(1))
                    };
                } else {
                    self.cursor = if delta < 0 {
                        self.cursor.saturating_sub(1)
                    } else {
                        (self.cursor + 1).min(self.playlists.len().saturating_sub(1))
                    };
                }
                None
            }
            MouseGesture::RightClick(_) if self.open.is_some() => {
                self.open = None;
                self.open_items.clear();
                Some(Msg::Shell(ShellRequest::PlaylistsBack))
            }
            gesture @ (MouseGesture::Click(at) | MouseGesture::DoubleClick(at)) => {
                if self.panel_area.is_some_and(|area| !area.contains(at)) {
                    return Some(Msg::Shell(ShellRequest::DismissPlaylists));
                }
                let &(open, index) = self.hit_rows.resolve(at)?;
                if open {
                    self.open_cursor = index;
                } else {
                    self.cursor = index;
                }
                matches!(gesture, MouseGesture::DoubleClick(_))
                    .then_some(Msg::Shell(ShellRequest::PlaylistsActivate { open, index }))
            }
            _ => None,
        }
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
            PlaylistsViewState {
                panel_area: self.panel_area,
                playlists: &self.playlists,
                playlists_cursor: &mut self.cursor,
                playlists_scroll: &mut self.scroll,
                playlists_loading: self.loading,
                playlists_open: self.open.as_ref(),
                open_items: &self.open_items,
                open_cursor: &mut self.open_cursor,
                open_scroll: &mut self.open_scroll,
                open_loading: self.open_loading,
                loaded_id: self.loaded_id.as_deref(),
                geometry: &mut self.geometry,
            },
        );
        // Adopt the rows the painter just produced into the irregular-chrome
        // registry (task 5.2, design.md D6). Open rows are pushed last so
        // they win overlaps, matching the old `hit_test` ordering.
        self.hit_rows.clear();
        for (rect, index) in &self.geometry.playlist_rows {
            self.hit_rows.push(*rect, (false, *index));
        }
        for (rect, index) in &self.geometry.open_rows {
            self.hit_rows.push(*rect, (true, *index));
        }
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
