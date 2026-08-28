//! Interactive Component for grouped Music's wide workspace.
//!
//! The shell mirrors album data and cached tracks. Album/track cursor state is
//! local here; cross-authority effects use typed shell requests.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{AlbumCursorKind, Msg, ShellRequest};
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
    /// Selected-album identity from the last pushed context. When it changes
    /// (group switch, recursive-album activation, position restore), inline
    /// track focus must reset: a focused track index refers to the previous
    /// album's track list.
    last_album_id: Option<String>,
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
            last_album_id: None,
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
        let album_changed = self.last_album_id.as_deref()
            != context
                .selected_album
                .as_ref()
                .map(|album| album.id.as_str());
        if !self.initialized {
            self.album_cursor = context.list.cursor();
            self.album_scroll = context.list.scroll();
            self.track_cursor = context.track_cursor;
            self.last_album_id = context
                .selected_album
                .as_ref()
                .map(|album| album.id.clone());
            self.initialized = true;
        } else {
            if self.album_cursor == self.last_mirrored_cursor {
                self.album_cursor = context.list.cursor();
            }
            if self.album_scroll == self.last_mirrored_scroll {
                self.album_scroll = context.list.scroll();
            }
            // Inline track focus is owned here; the only external resets are
            // the selected-album identity changing and narrow mode disabling
            // the feature (both leave `track_cursor` `None`).
            if album_changed {
                self.track_cursor = None;
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
        self.context.focused && self.track_cursor.is_none()
    }

    /// Whether the active panel is the Library panel (track-mode keys are
    /// only tracked while the Library panel owns the keys -- with the Queue
    /// panel focused, Up/Down etc. keep their queue meaning).
    fn library_panel_active(&self) -> bool {
        self.context.focused
    }

    pub(in crate::app) fn track_cursor(&self) -> Option<usize> {
        self.track_cursor
    }

    #[cfg(test)]
    pub(in crate::app) fn album_tracks_loading(&self) -> bool {
        self.context.album_tracks_loading
    }

    /// Whether inline track focus can be entered right now: wide mode
    /// (`inline_track_focus_enabled`) with the selected album's tracks
    /// cached. Narrow mode keeps `track_cursor` `None` by construction.
    fn can_enter_track_focus(&self) -> bool {
        self.inline_track_focus_enabled
            && self.context.focused
            && self
                .context
                .album_tracks
                .as_ref()
                .is_some_and(|tracks| !tracks.is_empty())
    }

    /// Shell-driven entry into inline track focus (recursive album
    /// activation): enters only when the feature is enabled and the selected
    /// album's tracks are cached; a no-op in narrow mode.
    pub(in crate::app) fn enter_track_focus(&mut self) {
        if self.can_enter_track_focus() {
            self.track_cursor = Some(0);
        }
    }

    /// Shell-driven clear of inline track focus (position restore): the
    /// deleted track-focus-clear rehome.
    pub(in crate::app) fn clear_track_focus(&mut self) {
        self.track_cursor = None;
    }

    fn move_track(&mut self, delta: i64) {
        let count = self.context.album_tracks.as_ref().map_or(0, Vec::len);
        if count > 0 {
            self.track_cursor = Some(move_cursor(self.track_cursor.unwrap_or(0), delta, count));
        }
    }

    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        if !self.context.focused {
            return None;
        }
        match key.code {
            // Activation while an inline album track is focused: play the
            // focused track through the album queue path. The shell resolves
            // the track from `track_cursor()` (target resolution lives at
            // the shell/component boundary, not in `App`).
            Key::Enter if self.track_cursor.is_some() => {
                return Some(Msg::Shell(ShellRequest::MusicTrackActivate));
            }
            // Ctrl+P keeps its "play current" meaning: with a focused track
            // that is the track, exactly like Enter.
            Key::Char('p')
                if key.modifiers.contains(KeyModifiers::CONTROL) && self.track_cursor.is_some() =>
            {
                return Some(Msg::Shell(ShellRequest::MusicTrackActivate));
            }
            // Enter on an album row (Library panel): enter inline track
            // focus when wide with cached tracks; otherwise request the
            // narrow album activation effect from the shell.
            Key::Enter if self.track_cursor.is_none() => {
                if self.can_enter_track_focus() {
                    self.track_cursor = Some(0);
                    return None;
                }
                return Some(Msg::Shell(ShellRequest::MusicAlbumActivate));
            }
            // Exit inline track focus locally; the key must not reach the
            // unprefixed panel's Esc/Stop semantics.
            Key::Esc | Key::Backspace if self.track_cursor.is_some() => {
                self.track_cursor = None;
                return None;
            }
            // Track moves are local to the component while a track is
            // focused and the Library panel owns the keys; with the Queue
            // panel focused the keys are left unclaimed for the central
            // router.
            Key::Up | Key::Char('k')
                if self.track_cursor.is_some() && self.library_panel_active() =>
            {
                self.move_track(-1);
                return None;
            }
            Key::Down | Key::Char('j')
                if self.track_cursor.is_some() && self.library_panel_active() =>
            {
                self.move_track(1);
                return None;
            }
            // Enqueue / context menu target the focused track while one is
            // focused (Library panel); otherwise leave the key unhandled.
            Key::Char('a')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.track_cursor.is_some()
                    && self.library_panel_active() =>
            {
                return Some(Msg::Shell(ShellRequest::MusicTrackEnqueue));
            }
            Key::Char('.') if self.track_cursor.is_some() && self.library_panel_active() => {
                return Some(Msg::Shell(ShellRequest::MusicTrackContextMenu));
            }
            Key::Up | Key::Char('k') if self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(-1, self.album_columns, true)
                    .unwrap_or(self.album_cursor);
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Down | Key::Char('j') if self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(1, self.album_columns, true)
                    .unwrap_or(self.album_cursor);
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Char('h') if self.album_columns > 1 && self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(-1, 1, true)
                    .unwrap_or(self.album_cursor);
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Char('l') if self.album_columns > 1 && self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(1, 1, true)
                    .unwrap_or(self.album_cursor);
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Move,
                }));
            }
            Key::Home if self.can_emit_album_cursor() => {
                let target = self
                    .context
                    .album_order
                    .first()
                    .copied()
                    .unwrap_or(self.album_cursor);
                self.album_cursor = target;
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Jump,
                }));
            }
            Key::End if self.can_emit_album_cursor() => {
                let target = self
                    .context
                    .album_order
                    .last()
                    .copied()
                    .unwrap_or(self.album_cursor);
                self.album_cursor = target;
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Jump,
                }));
            }
            Key::PageUp if self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(-(self.page_rows as i64), self.album_columns, false)
                    .unwrap_or(self.album_cursor);
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Page,
                }));
            }
            Key::PageDown if self.can_emit_album_cursor() => {
                let target = self
                    .move_album_rows(self.page_rows as i64, self.album_columns, false)
                    .unwrap_or(self.album_cursor);
                return Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                    target,
                    kind: AlbumCursorKind::Page,
                }));
            }
            _ => None,
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
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
        None
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
