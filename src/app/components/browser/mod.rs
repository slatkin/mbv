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
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::browser_narrow::NarrowBrowseExtras;
use super::component_id::BrowserKind;
use super::media_list::ViewportAnchor;
use super::msg::{BrowserHitRegion, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::render::{shared_hero_presentation, HomeImagePaint, LibraryListRenderCtx};

mod keyboard;
mod navigation;
mod paint;

pub struct BrowserComponent {
    kind: BrowserKind,
    context: LibraryListRenderCtx,
    cursor: usize,
    scroll: usize,
    focused: bool,
    layout: LayoutMain,
    /// Whether the component's own BrowserKey kind and painted geometry select
    /// the hero-on-left layout. The value is derived in `view()` rather than
    /// projected from the App layout.
    wide_movies: bool,
    /// Whether the wide layout's pill row is a home-video count label (vs. a
    /// letter-range pill row). Fed by the shell from validated content.
    wide_movies_home_video: bool,
    /// Whether the wide layout shows the letter-range pill row. Fed by the
    /// shell each draw (task 5.3d.17a).
    wide_movies_letter_pills: bool,
    /// Runtime terminal-capability flag (config-derived), set by the shell so
    /// the component can paint the hero text like every other surface.
    use_nerd_fonts: bool,
    /// The hero cover image `view()` computed but could not paint itself (no
    /// `App`/image-cache authority); the shell takes it right after
    /// `application.view()` and paints it via `App::paint_home_image`
    /// (mirrors `HomeComponent`, task 5.3d.17a).
    image_paint: Option<HomeImagePaint>,
    /// Shell-resolved narrow-browse extras (count label, letter pills, inline
    /// movie/series hero) for the `browser_narrow` composer, pushed each frame
    /// by `render_emby_browser_component` (task 3.3).
    narrow_extras: NarrowBrowseExtras,
    pending_anchor: Option<ViewportAnchor<String>>,
}

impl BrowserComponent {
    pub fn new() -> Self {
        Self::new_for_kind(BrowserKind::Generic)
    }

    pub fn new_for_kind(kind: BrowserKind) -> Self {
        Self {
            kind,
            context: LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
            cursor: 0,
            scroll: 0,
            focused: false,
            layout: LayoutMain::default(),
            wide_movies: false,
            wide_movies_home_video: false,
            wide_movies_letter_pills: false,
            use_nerd_fonts: false,
            image_paint: None,
            narrow_extras: NarrowBrowseExtras::default(),
            pending_anchor: None,
        }
    }

    /// Records the shell-resolved narrow-browse extras for the next `view()`
    /// (task 3.3). Pushed each frame by `render_emby_browser_component`.
    pub(in crate::app) fn set_narrow_extras(&mut self, extras: NarrowBrowseExtras) {
        self.narrow_extras = extras;
    }

    pub(in crate::app) fn set_content(&mut self, context: LibraryListRenderCtx, focused: bool) {
        self.context = context;
        self.focused = focused;
        if let Some(anchor) = self.pending_anchor.as_ref() {
            if let Some(cursor) = self
                .context
                .items
                .iter()
                .position(|item| item.id == anchor.selected_target)
            {
                self.cursor = cursor;
                return;
            }
        }
        // Sync component cursor/scroll from App cursor. In the new architecture,
        // `set_content` is always called after the App cursor has been updated
        // (either by the component's own request or by an external change like
        // tab switch or go_back), so we can always sync from the context.
        self.cursor = self
            .context
            .cursor()
            .min(self.context.item_count().saturating_sub(1));
        self.scroll = self.context.scroll();
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    /// The scroll offset the last `view()` painted the list at. The shell
    /// reads this after `application.view()` and persists it back into the
    /// App nav level (task 5.3d.17b): the legacy wide renderer wrote
    /// `level.scroll = final_scroll`, and `set_content` overwrites the
    /// component's own `self.scroll` next frame from the App nav level, so
    /// without the write-back the rendered scroll would be lost on resize /
    /// first paint.
    pub(in crate::app) fn scroll(&self) -> usize {
        self.scroll
    }

    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        let item = self.context.items.get(self.cursor)?;
        Some(ViewportAnchor {
            selected_target: item.id.clone(),
            selected_row_offset: self
                .cursor
                .saturating_sub(self.scroll)
                .min(viewport_height.saturating_sub(1)),
        })
    }

    pub(in crate::app) fn apply_viewport_anchor(&mut self, anchor: ViewportAnchor<String>) {
        self.pending_anchor = Some(anchor);
    }

    pub(in crate::app) fn painted_viewport_height(&self) -> usize {
        self.layout.left_area.height as usize
    }

    /// Records the wide layout's pill-row presentation from validated shell
    /// content; whether the layout is wide is derived locally in `view()`.
    pub(in crate::app) fn set_wide_movies(&mut self, home_video: bool, letter_pills: bool) {
        self.wide_movies_home_video = home_video;
        self.wide_movies_letter_pills = letter_pills;
    }

    /// Runtime terminal-capability flag (task 5.3d.17a): mirrors
    /// `HomeComponent::set_use_nerd_fonts` so the component can paint the
    /// wide hero text.
    pub(in crate::app) fn set_use_nerd_fonts(&mut self, use_nerd_fonts: bool) {
        self.use_nerd_fonts = use_nerd_fonts;
    }

    /// Takes the hero cover image (if any) `view()` computed but could not
    /// paint itself. The shell calls this right after `application.view()`
    /// returns and paints it via `App::paint_home_image` (mirrors
    /// `HomeComponent::take_image_paint`, task 5.3d.17a).
    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    /// Handle a mouse event against the component's painted browse geometry.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
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
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    -1
                } else {
                    1
                };
                if self.layout.left_area.contains(position) {
                    let rows = self.layout.left_item_rows.len();
                    let viewport = self.layout.left_area.height as usize;
                    let max_offset = rows.saturating_sub(viewport);
                    self.scroll = self
                        .scroll
                        .saturating_add_signed(
                            delta.clamp(isize::MIN as i64, isize::MAX as i64) as isize
                        )
                        .min(max_offset);
                    return Some(Msg::Shell(ShellRequest::BrowserScroll {
                        delta,
                        offset: self.scroll,
                    }));
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
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
            MouseEventKind::Down(MouseButton::Right)
                if (self.layout.left_area.contains(position)
                    || self.layout.inline_hero_area.contains(position)) =>
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
            _ => {}
        }
        None
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

    /// Test-only cursor seed (task 5.3d.16): `set_content` no longer mirrors
    /// the shell cursor, so tests position the authoritative local cursor
    /// directly before exercising navigation.
    #[cfg(test)]
    pub(crate) fn set_cursor_for_test(&mut self, cursor: usize) {
        self.cursor = cursor;
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
        if let Some(anchor) = self.pending_anchor.take() {
            if let Some(cursor) = self
                .context
                .items
                .iter()
                .position(|item| item.id == anchor.selected_target)
            {
                self.cursor = cursor;
                self.scroll = cursor.saturating_sub(anchor.selected_row_offset).min(
                    self.context
                        .items
                        .len()
                        .saturating_sub(area.height as usize),
                );
            }
        }
        let mut context = self
            .context
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll);
        if let Some(items) = self.narrow_extras.feed_items.as_ref() {
            context = LibraryListRenderCtx::from_items(
                items.clone(),
                self.cursor.min(items.len().saturating_sub(1)),
                self.scroll,
            )
            .with_group_pills(true)
            .with_loading(context.loading);
        }
        // Task 5.3d.17a: when the wide Movies/home-video hero-on-left layout
        // is active (this component's own `kind` AND the area is wide enough
        // for the shared split), paint the full hero + pills + list layout
        // itself instead of just the inner list rows; otherwise keep the
        // narrow list-row behavior.
        let wide = (matches!(self.kind, BrowserKind::Movies | BrowserKind::HomeVideos)
            || self.narrow_extras.feed_items.is_some())
            && shared_hero_presentation(area).is_some();
        self.wide_movies = wide;
        self.scroll = if wide {
            self.render_wide_movies(frame, area, &context)
        } else {
            // Narrow generic/Movies/home-video: the component owns the full
            // surface via the `browser_narrow` composer (task 3.3). It returns
            // the landed scroll and the poster image still needing paint (the
            // shell executes it via `App::paint_home_image`, mirroring the
            // wide path and `HomeComponent`).
            let (scroll, image_paint) = crate::app::render::render_narrow_browse_with_ctx(
                frame,
                area,
                &context,
                &self.narrow_extras,
                self.focused,
                &mut self.layout,
            );
            self.image_paint = image_paint;
            scroll
        };
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
            Event::Keyboard(key) => self.handle_tui_key(*key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
