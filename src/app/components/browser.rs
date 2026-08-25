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
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
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
        // Task 5.3d, Emby browser effect decoupling: the selected-item
        // keyboard effects resolve their target from the component's own
        // local cursor/content and ride a typed `ShellRequest` carrying the
        // owned `EmbyItem`, so the Model/App effect acts on that supplied
        // item directly (never by copying the component cursor into a
        // `BrowseLevel.cursor` and re-reading it). `focused` preserves the
        // legacy Library-panel gate exactly (`effective_panel_focus() ==
        // Library` → these keys reach `handle_lib_key`); when no item is
        // selected (empty nav level) or while unfocused, the key is forwarded
        // to the legacy bridge so legacy resolution (e.g. Enter on the
        // library root) is preserved unchanged. A typed request is returned
        // in place of the raw legacy key, never in addition to it — no
        // double execution.
        if self.focused {
            let selected = self.selected_effect_item();
            let request = match key.code {
                crossterm::event::KeyCode::Enter => {
                    selected.map(|item| ShellRequest::BrowserActivate { item })
                }
                crossterm::event::KeyCode::Char('p')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    selected.map(|item| ShellRequest::BrowserPlay { item })
                }
                crossterm::event::KeyCode::Char('a')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    selected.map(|item| ShellRequest::BrowserEnqueue { item })
                }
                crossterm::event::KeyCode::Char('w')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    selected.map(|item| ShellRequest::BrowserToggleWatched { item })
                }
                // '.' opens the context menu for the component-selected item
                // (task 5.3d, Emby browser context-menu decoupling). No
                // modifier guard: the legacy `handle_global_view_key` arm it
                // replaces matched `Char('.')` with any modifiers, so this
                // preserves the legacy '.' modifier behavior exactly.
                crossterm::event::KeyCode::Char('.') => {
                    selected.map(|item| ShellRequest::BrowserContextMenu { item })
                }
                // Ctrl+S shuffles the component-selected item (task 5.3d,
                // Emby browser shuffle decoupling). Control-modifier guarded
                // exactly as the legacy `handle_lib_key` arm it replaces; when
                // no item is selected the key is forwarded to the legacy
                // bridge below, which shuffles the current browse-level parent
                // through `shuffle_play` exactly as before.
                crossterm::event::KeyCode::Char('s')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    selected.map(|item| ShellRequest::BrowserShuffle { item })
                }
                _ => None,
            };
            // The component owns the selection: the item is resolved at the
            // component-local cursor in the mirrored content, never a re-read
            // of an App field.
            if let Some(request) = request {
                return Some(Msg::Shell(request));
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Key(key)))
    }

    /// Resolve the item at the component's own local cursor over the mirrored
    /// content (task 5.3d, Emby browser effect decoupling). The mirrored
    /// `context` still carries the App cursor/scroll values; the component's
    /// local `self.cursor` is authoritative for effect targets, so the item
    /// is resolved at that cursor — never by re-reading an App field. `None`
    /// when the list is empty (forwarded to the legacy bridge by the caller).
    fn selected_effect_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.context
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll)
            .selected_item()
            .cloned()
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
                    // Resolve the clicked row from the component's own painted
                    // geometry *before* building the region, so the emitted
                    // cursor matches the row under the click (not the
                    // pre-click cursor). The inline hero is already on the
                    // selected item, so it carries the current cursor.
                    let in_hero = self.layout.inline_hero_area.contains(position);
                    if !in_hero {
                        if let Some(resolved) = self.resolve_left_cursor(col, row) {
                            self.cursor = resolved;
                        }
                    }
                    let region = if in_hero {
                        BrowserHitRegion::InlineHero(self.cursor)
                    } else {
                        BrowserHitRegion::LeftRow(self.cursor)
                    };
                    return Some(Msg::Shell(ShellRequest::BrowserClick { region, col, row }));
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                if self.layout.left_area.contains(position)
                    || self.layout.inline_hero_area.contains(position)
                {
                    // Resolve the row under the click before opening the menu;
                    // a blank/gap click leaves the cursor unchanged
                    // (`resolve_left_cursor` returns None for headers/gaps).
                    let in_hero = self.layout.inline_hero_area.contains(position);
                    if !in_hero {
                        if let Some(resolved) = self.resolve_left_cursor(col, row) {
                            self.cursor = resolved;
                        }
                    }
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
    /// painted `LayoutMain`, mirroring the legacy `App::click_set_cursor`
    /// Emby branch: the exact cell is picked when the list is two-column, and
    /// header/gap screen rows are `None` (no-op). Returns `None` for clicks
    /// outside the list area or on a header/gap cell, leaving the cursor
    /// unchanged. The `BrowserClick` shell arm consumes the resolved target.
    fn resolve_left_cursor(&self, col: u16, row: u16) -> Option<usize> {
        let la = self.layout.left_area;
        if !la.contains((col, row).into()) {
            return None;
        }
        let click_y = (row.saturating_sub(la.y)) as usize;
        let display_row = self.scroll + click_y;
        // Cell-aware two-column resolution: pick the exact column under the
        // click. Single-column and header rows fall back to the row map below.
        if let Some(items) = self.layout.left_item_rows.get(display_row) {
            if items.len() > 1 {
                let cols = self
                    .layout
                    .left_item_rows
                    .iter()
                    .map(Vec::len)
                    .max()
                    .unwrap_or(1);
                let cell_w = library_cell_width(la, cols) as usize;
                let x = (col.saturating_sub(la.x)) as usize;
                let stride = cell_w + LIBRARY_COLUMN_GAP as usize;
                let cell = x / stride;
                if cell < items.len() && x % stride < cell_w {
                    return items.get(cell).copied();
                }
                return None;
            }
        }
        self.layout.left_row_map.get(click_y).copied().flatten()
    }

    #[cfg(test)]
    pub(crate) fn test_layout(&self) -> &LayoutMain {
        &self.layout
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
