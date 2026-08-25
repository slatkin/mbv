//! Interactive Component for grouped Music's wide workspace.
//!
//! The shell mirrors album data and cached tracks. Album/track cursor state is
//! local here; legacy keys still forward to App during stage 1.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{AlbumCursorKind, LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::{LayoutMain, LibraryRowTarget};
use crate::app::render::{render_wide_music_group_with_ctx, MusicImagePaint, MusicWideRenderCtx};
use crate::app::ui_util::move_cursor;

pub struct MusicWorkspaceComponent {
    context: MusicWideRenderCtx,
    album_cursor: usize,
    album_columns: usize,
    page_rows: usize,
    album_scroll: usize,
    track_cursor: Option<usize>,
    initialized: bool,
    last_mirrored_cursor: usize,
    last_mirrored_scroll: usize,
    last_mirrored_track: Option<usize>,
    layout: LayoutMain,
    image_paint: Option<MusicImagePaint>,
    inline_track_focus_enabled: bool,
}

impl MusicWorkspaceComponent {
    pub fn new() -> Self {
        Self {
            context: MusicWideRenderCtx::new(
                crate::app::render::LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
                None,
                String::new(),
                Vec::new(),
                0,
                Vec::new(),
                Vec::new(),
                false,
                false,
                None,
                false,
                None,
            ),
            album_cursor: 0,
            album_columns: 1,
            page_rows: 1,
            album_scroll: 0,
            track_cursor: None,
            initialized: false,
            last_mirrored_cursor: 0,
            last_mirrored_scroll: 0,
            last_mirrored_track: None,
            layout: LayoutMain::default(),
            image_paint: None,
            inline_track_focus_enabled: false,
        }
    }

    pub(in crate::app) fn set_inline_track_focus_enabled(&mut self, enabled: bool) {
        self.inline_track_focus_enabled = enabled;
        if !enabled {
            self.track_cursor = None;
        }
    }

    pub(in crate::app) fn set_content(&mut self, context: MusicWideRenderCtx) {
        if !self.initialized {
            self.album_cursor = context.list.cursor();
            self.album_scroll = context.list.scroll();
            self.track_cursor = context.track_cursor;
            self.initialized = true;
        } else {
            if self.album_cursor == self.last_mirrored_cursor {
                self.album_cursor = context.list.cursor();
            }
            if self.album_scroll == self.last_mirrored_scroll {
                self.album_scroll = context.list.scroll();
            }
            if self.track_cursor == self.last_mirrored_track {
                self.track_cursor = context.track_cursor;
            }
        }
        self.context = context;
        self.album_cursor = self
            .album_cursor
            .min(self.context.list.item_count().saturating_sub(1));
        if let Some(cursor) = self.track_cursor {
            let count = self.context.album_tracks.as_ref().map_or(0, Vec::len);
            if count > 0 {
                self.track_cursor = Some(cursor.min(count - 1));
            }
        }
        self.last_mirrored_cursor = self.context.list.cursor();
        self.last_mirrored_scroll = self.context.list.scroll();
        self.last_mirrored_track = self.context.track_cursor;
    }

    pub(in crate::app) fn set_album_columns(&mut self, columns: usize) {
        self.album_columns = columns.max(1);
    }

    pub(in crate::app) fn set_page_rows(&mut self, rows: usize) {
        self.page_rows = rows.max(1);
    }

    pub(in crate::app) fn album_cursor(&self) -> usize {
        self.album_cursor
    }

    fn move_album_rows(&mut self, rows: i64, columns: usize, wrap: bool) -> Option<usize> {
        let order = &self.context.album_order;
        if order.is_empty() {
            return None;
        }
        let position = order
            .iter()
            .position(|&index| index == self.album_cursor)
            .unwrap_or(0);
        let delta = rows.saturating_mul(columns.max(1) as i64);
        let target_position = if wrap {
            move_cursor(position, delta, order.len())
        } else if delta.is_negative() {
            position.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            position
                .saturating_add(delta as usize)
                .min(order.len().saturating_sub(1))
        };
        self.album_cursor = order[target_position];
        Some(self.album_cursor)
    }

    fn can_emit_album_cursor(&self) -> bool {
        self.context.focused && self.track_cursor.is_none() && !self.context.album_order.is_empty()
    }

    pub(in crate::app) fn track_cursor(&self) -> Option<usize> {
        self.track_cursor
    }

    fn move_track(&mut self, delta: i64) {
        let count = self.context.album_tracks.as_ref().map_or(0, Vec::len);
        if count > 0 {
            self.track_cursor = Some(move_cursor(self.track_cursor.unwrap_or(0), delta, count));
        }
    }

    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Enter if self.track_cursor.is_none() => {
                if self.inline_track_focus_enabled
                    && self
                        .context
                        .album_tracks
                        .as_ref()
                        .is_some_and(|tracks| !tracks.is_empty())
                {
                    self.track_cursor = Some(0);
                }
            }
            Key::Esc | Key::Backspace => self.track_cursor = None,
            Key::Up | Key::Char('k') if self.track_cursor.is_some() => self.move_track(-1),
            Key::Down | Key::Char('j') if self.track_cursor.is_some() => self.move_track(1),
            Key::Up | Key::Char('k') if self.can_emit_album_cursor() => {
                let target = self.move_album_rows(-1, self.album_columns, true).unwrap();
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Down | Key::Char('j') if self.can_emit_album_cursor() => {
                let target = self.move_album_rows(1, self.album_columns, true).unwrap();
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Char('h') if self.album_columns > 1 && self.can_emit_album_cursor() => {
                let target = self.move_album_rows(-1, 1, true).unwrap();
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Char('l') if self.album_columns > 1 && self.can_emit_album_cursor() => {
                let target = self.move_album_rows(1, 1, true).unwrap();
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Home if self.can_emit_album_cursor() => {
                let target = self.context.album_order[0];
                self.album_cursor = target;
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Jump,
                }));
            }
            Key::End if self.can_emit_album_cursor() => {
                let target = *self.context.album_order.last().unwrap();
                self.album_cursor = target;
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Jump,
                }));
            }
            Key::PageUp if self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(-(self.page_rows as i64), self.album_columns, false)
                    .unwrap();
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Page,
                }));
            }
            Key::PageDown if self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(self.page_rows as i64, self.album_columns, false)
                    .unwrap();
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Page,
                }));
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
            if let Some(track) = self.layout.wide_music_track_at(position) {
                self.track_cursor = Some(track);
            } else if self.layout.wide_music_browser_area.contains(position) {
                let row = position
                    .y
                    .saturating_sub(self.layout.wide_music_browser_area.y)
                    as usize;
                if let Some(Some(LibraryRowTarget::Album(album))) =
                    self.layout.left_row_targets.get(row)
                {
                    self.album_cursor = *album;
                }
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
    }

    pub(in crate::app) fn take_image_paint(&mut self) -> Option<MusicImagePaint> {
        self.image_paint.take()
    }
}

impl Default for MusicWorkspaceComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MusicWorkspaceComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.layout = LayoutMain::default();
        let context = self.context.clone().with_local_state(
            self.album_cursor,
            self.album_scroll,
            self.track_cursor,
        );
        let output = render_wide_music_group_with_ctx(frame, area, &context, &mut self.layout);
        self.album_scroll = output.final_scroll;
        self.image_paint = output.image_paint;
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

impl AppComponent<Msg, UserEvent> for MusicWorkspaceComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
