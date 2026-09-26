use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{LeafKeyResult, Msg, ShellRequest, TerminalObserverEvent};
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
        let playlists_changed = self.playlists != playlists;
        let open_changed = self.open != open || self.open_items != open_items;
        self.playlists = playlists;
        if playlists_changed {
            self.cursor = cursor.min(self.playlists.len().saturating_sub(1));
            self.scroll = scroll.min(self.cursor);
        } else {
            self.cursor = self.cursor.min(self.playlists.len().saturating_sub(1));
            self.scroll = self.scroll.min(self.cursor);
        }
        self.loading = loading;
        self.open = open;
        self.open_items = open_items;
        if open_changed {
            self.open_cursor = open_cursor.min(self.open_items.len().saturating_sub(1));
            self.open_scroll = open_scroll.min(self.open_cursor);
        } else {
            self.open_cursor = self
                .open_cursor
                .min(self.open_items.len().saturating_sub(1));
            self.open_scroll = self.open_scroll.min(self.open_cursor);
        }
        self.open_loading = open_loading;
        self.loaded_id = loaded_id;
    }

    pub(in crate::app) fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    #[cfg(test)]
    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }
    #[cfg(test)]
    pub(in crate::app) fn open_cursor(&self) -> usize {
        self.open_cursor
    }

    fn local_change() -> Option<Msg> {
        None
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.handle_navigation(key) {
            return Self::local_change();
        }
        if let Some(msg) = self.handle_back(key) {
            return Some(msg);
        }
        if let Some(msg) = Self::handle_dismiss(key, self.open.is_none()) {
            return Some(msg);
        }
        if let Some(msg) = Self::handle_global_command(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_open(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_activation(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_rename(key) {
            return Some(msg);
        }
        if let Some(msg) = self.handle_delete(key) {
            return Some(msg);
        }
        self.handle_refresh(key).or_else(Self::local_change)
    }

    fn handle_navigation(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            Key::Up if self.open.is_some() => {
                self.open_cursor = self.open_cursor.saturating_sub(1);
            }
            Key::Up => self.cursor = self.cursor.saturating_sub(1),
            Key::Down if self.open.is_some() => {
                self.open_cursor =
                    (self.open_cursor + 1).min(self.open_items.len().saturating_sub(1));
            }
            Key::Down => {
                self.cursor = (self.cursor + 1).min(self.playlists.len().saturating_sub(1));
            }
            Key::PageUp => self.move_page(-1),
            Key::PageDown => self.move_page(1),
            Key::Home if self.open.is_some() => self.open_cursor = 0,
            Key::Home => self.cursor = 0,
            Key::End if self.open.is_some() => {
                self.open_cursor = self.open_items.len().saturating_sub(1);
            }
            Key::End => self.cursor = self.playlists.len().saturating_sub(1),
            _ => return false,
        }
        true
    }

    fn handle_back(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Left | Key::Esc | Key::Backspace | Key::Function(4) if self.open.is_some() => {}
            _ => return None,
        }
        self.open = None;
        self.open_items.clear();
        Some(Msg::Shell(Box::new(ShellRequest::PlaylistsBack)))
    }

    fn handle_dismiss(key: &KeyEvent, closed: bool) -> Option<Msg> {
        match key.code {
            Key::Esc | Key::Function(4) if closed => {
                Some(Msg::Shell(Box::new(ShellRequest::DismissPlaylists)))
            }
            _ => None,
        }
    }

    fn handle_global_command(key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Function(2) => Some(Msg::Shell(Box::new(ShellRequest::OpenSettings))),
            Key::Function(3) => Some(Msg::Shell(Box::new(ShellRequest::OpenSessions))),
            Key::Char('q') if key.modifiers.is_empty() => {
                Some(Msg::Shell(Box::new(ShellRequest::Quit)))
            }
            _ => None,
        }
    }

    fn handle_open(&self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Right if self.open.is_none() && self.cursor < self.playlists.len() => Some(
                Msg::Shell(Box::new(ShellRequest::PlaylistsOpen(self.cursor))),
            ),
            _ => None,
        }
    }

    fn handle_activation(&self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Enter {
            return None;
        }
        let open = self.open.is_some();
        let index = if open { self.open_cursor } else { self.cursor };
        Some(Msg::Shell(Box::new(ShellRequest::PlaylistsActivate {
            open,
            index,
        })))
    }

    fn handle_rename(&self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Char('n') || !key.modifiers.is_empty() || self.open.is_some() {
            return None;
        }
        (self.cursor < self.playlists.len()).then_some(Msg::Shell(Box::new(
            ShellRequest::PlaylistsRename(self.cursor),
        )))
    }

    fn handle_delete(&self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Char('d') || !key.modifiers.is_empty() || self.open.is_some() {
            return None;
        }
        (self.cursor < self.playlists.len()).then_some(Msg::Shell(Box::new(
            ShellRequest::PlaylistsDelete(self.cursor),
        )))
    }

    fn handle_refresh(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.code != Key::Char('r') {
            return None;
        }
        if self.open.is_some() {
            self.open = None;
            self.open_items.clear();
        }
        Some(Msg::Shell(Box::new(ShellRequest::PlaylistsRefresh)))
    }

    fn move_page(&mut self, direction: i64) {
        let page = i64::from(self.geometry.panel_area.height.saturating_sub(4));
        if self.open.is_some() {
            let last = i64::try_from(self.open_items.len().saturating_sub(1)).unwrap_or(i64::MAX);
            let cursor = i64::try_from(self.open_cursor).unwrap_or(i64::MAX);
            self.open_cursor =
                usize::try_from(cursor.saturating_add(direction * page).clamp(0, last))
                    .expect("clamped cursor is non-negative");
        } else {
            let last = i64::try_from(self.playlists.len().saturating_sub(1)).unwrap_or(i64::MAX);
            let cursor = i64::try_from(self.cursor).unwrap_or(i64::MAX);
            self.cursor = usize::try_from(cursor.saturating_add(direction * page).clamp(0, last))
                .expect("clamped cursor is non-negative");
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
    #[cfg(test)]
    pub(crate) fn first_open_row(&self) -> Rect {
        self.geometry.open_rows[0].0
    }

    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
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
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MouseGesture::RightClick(_) if self.open.is_some() => {
                self.open = None;
                self.open_items.clear();
                Some(Msg::Shell(Box::new(ShellRequest::PlaylistsBack)))
            }
            gesture @ (MouseGesture::Click { at, .. } | MouseGesture::DoubleClick(at)) => {
                if !self.geometry.panel_area.contains(at) {
                    return Some(Msg::Shell(Box::new(ShellRequest::DismissPlaylists)));
                }
                let &(open, index) = self.hit_rows.resolve(at)?;
                if open {
                    self.open_cursor = index;
                } else {
                    self.cursor = index;
                }
                matches!(gesture, MouseGesture::DoubleClick(_)).then_some(Msg::Shell(Box::new(
                    ShellRequest::PlaylistsActivate { open, index },
                )))
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

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))).into_option(),
                None if matches!(
                    key.code,
                    Key::Up
                        | Key::Down
                        | Key::PageUp
                        | Key::PageDown
                        | Key::Home
                        | Key::End
                        | Key::Left
                        | Key::Right
                        | Key::Esc
                        | Key::Backspace
                        | Key::Enter
                        | Key::Char(_)
                ) =>
                {
                    LeafKeyResult::Consumed(None).into_option()
                }
                None => LeafKeyResult::Unhandled.into_option(),
            },
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: Key) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }
    }

    #[test]
    fn closed_playlist_keys_open_and_clamp_navigation() {
        let mut component = PlaylistsComponent::new();
        component.playlists = vec![
            crate::app::tests::make_item("First", "Playlist"),
            crate::app::tests::make_item("Second", "Playlist"),
        ];

        component.handle_key(&key(Key::End));
        assert_eq!(component.cursor, 1);
        assert_eq!(
            component.handle_key(&key(Key::Right)),
            Some(Msg::Shell(Box::new(ShellRequest::PlaylistsOpen(1))))
        );
    }

    #[test]
    fn open_playlist_back_clears_open_items() {
        let mut component = PlaylistsComponent::new();
        component.open = Some(crate::app::tests::make_item("Playlist", "Playlist"));
        component
            .open_items
            .push(crate::app::tests::make_item("Film", "Movie"));

        assert_eq!(
            component.handle_key(&key(Key::Esc)),
            Some(Msg::Shell(Box::new(ShellRequest::PlaylistsBack)))
        );
        assert!(component.open.is_none());
        assert!(component.open_items.is_empty());
    }
}
