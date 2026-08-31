//! Interactive Component for the generic Emby browser rows.
//!
//! The shell mirrors the active list source into this component. Generic,
//! Movies, and home-video rows use the existing typed render seam; music,
//! TV/series, and album-track presentation remain on their legacy branches
//! until their owning tasks convert them.

use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::browser_narrow::NarrowBrowseExtras;
use super::component_id::BrowserKind;
use super::msg::{BrowserHitRegion, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::palette;
use crate::app::render::{
    hero_on_left_list_panel_border, hero_on_left_right_pane, padded_rect,
    prepare_wide_emby_hero_card, render_count_label,
    render_generic_movies_home_video_rows_with_ctx, render_home_hero_content, render_pill_bar,
    render_search_box, shared_hero_presentation, wide_library_panes, HeroData, HomeImagePaint,
    LetterFilter, LibraryListRenderCtx, PillBar, PANE_PAD_X, PANE_PAD_Y,
};

#[path = "browser_navigation.rs"]
mod browser_navigation;

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
        }
    }

    /// Records the shell-resolved narrow-browse extras for the next `view()`
    /// (task 3.3). Pushed each frame by `render_emby_browser_component`.
    pub(in crate::app) fn set_narrow_extras(&mut self, extras: NarrowBrowseExtras) {
        self.narrow_extras = extras;
    }

    pub(in crate::app) fn set_content(&mut self, context: LibraryListRenderCtx, focused: bool) {
        self.context = context;
        // Sync component cursor/scroll from App cursor. In the new architecture,
        // `set_content` is always called after the App cursor has been updated
        // (either by the component's own request or by an external change like
        // tab switch or go_back), so we can always sync from the context.
        self.cursor = self
            .context
            .cursor()
            .min(self.context.item_count().saturating_sub(1));
        self.scroll = self.context.scroll();
        self.focused = focused;
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

    /// Paints the wide Movies/home-video hero-on-left layout: a read-only
    /// shared Emby hero card on the left and the letter-pill/count/search
    /// row plus the one-column list in the right rail. Mirrors the deleted
    /// legacy wide renderer so the picture is unchanged.
    /// Returns the final list scroll (the component owns its cursor/scroll,
    /// so it records it instead of writing the App nav level).
    fn render_wide_movies(
        &mut self,
        f: &mut Frame,
        area: Rect,
        ctx: &LibraryListRenderCtx,
    ) -> usize {
        let body_area = if self.narrow_extras.feed_items.is_some() {
            crate::app::render::render_wide_feed_layer(
                f,
                area,
                &self.narrow_extras,
                &mut self.layout,
            );
            Rect {
                y: area.y.saturating_add(2),
                height: area.height.saturating_sub(2),
                ..area
            }
        } else {
            area
        };

        let left_content_area = Rect {
            height: body_area.height.saturating_sub(1),
            ..body_area
        };
        let Some(panes) = wide_library_panes(body_area, PANE_PAD_X, PANE_PAD_Y) else {
            // Breakpoint no longer fits: fall back to the plain list rows.
            return render_generic_movies_home_video_rows_with_ctx(
                f,
                area,
                ctx,
                self.focused,
                &mut self.layout,
            );
        };
        let mut left_panel = panes.left_panel;
        let right_panel = panes.right_panel;
        left_panel.height = left_content_area.height;

        // Library-side separator row below the left pane, matching Music and
        // Home's wide layout.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            Rect {
                x: left_panel.x,
                y: left_panel.bottom(),
                width: left_panel.width,
                height: 1,
            },
        );

        let left_area = panes.left_area;
        let right_area = panes.right_area;
        self.layout.movies_wide_right_area = right_area;

        // Left pane: read-only shared hero card (not an interactive hero —
        // `layout.hero_area` stays unset so the left pane is outside mouse
        // geometry, mirroring the legacy wide renderer).
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_RESTING)),
            left_panel,
        );
        let hero_content = padded_rect(left_area, PANE_PAD_X, 0);
        let hero_data = ctx.selected_item().and_then(|item| {
            prepare_wide_emby_hero_card(item, hero_content).map(
                |(meta_layout, meta_area, img_area)| {
                    HeroData::Emby(
                        Box::new(item.clone()),
                        meta_area,
                        meta_area,
                        img_area,
                        meta_layout,
                    )
                },
            )
        });

        // Right rail: pill row + one-column list.
        let right_pane = hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        let pills_area = right_pane.pills_area;
        let list_panel = right_pane.list_panel;

        if ctx.is_search_active() {
            render_search_box(
                f,
                pills_area,
                ctx.search_query.as_deref().unwrap_or_default(),
                ctx.search_loading,
            );
        } else if self.wide_movies_home_video {
            render_count_label(f, pills_area, ctx.total_count);
        } else if self.wide_movies_letter_pills {
            self.render_letter_pills_row(f, pills_area, ctx);
        }

        if list_panel.height > 0 {
            let list_bg = palette::resolve_surface_focus(self.focused);
            f.render_widget(
                Block::default().style(Style::default().bg(list_bg)),
                list_panel,
            );
        }
        let list_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);

        let final_scroll = render_generic_movies_home_video_rows_with_ctx(
            f,
            list_area,
            ctx,
            self.focused,
            &mut self.layout,
        );

        // Paint the shared hero text last (after the list); defer the cover
        // image paint to the shell, which owns the image-cache authority.
        if let Some(hero_data) = &hero_data {
            self.image_paint =
                render_home_hero_content(f, hero_data, true, self.focused, self.use_nerd_fonts);
        } else {
            self.image_paint = None;
        }

        hero_on_left_list_panel_border(f, list_panel, self.focused);
        final_scroll
    }

    fn render_letter_pills_row(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        ctx: &LibraryListRenderCtx,
    ) {
        if row_area.width == 0 {
            self.layout.selector_tabs = Vec::new();
            return;
        }
        let selected_pos = ctx.letter_filter.as_ref().map(|flt| flt.index).unwrap_or(0);
        let labels = LetterFilter::labels();
        let ids: Vec<usize> = (0..labels.len()).collect();
        self.layout.selector_tabs = render_pill_bar(
            f,
            row_area,
            PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos,
                prefix: Some(" ⌘ "),
            },
        );
    }

    pub(in crate::app) fn handle_tui_key(&mut self, key: KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Char('/') if key.modifiers.is_empty() => {
                return Some(Msg::Shell(super::msg::ShellRequest::OpenInlineSearch));
            }
            _ => {}
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        // Local keyboard navigation routes through typed `ShellRequest`s:
        // the component mutates only its own cursor, then returns the
        // request in place of the raw key so the shell drives the App cursor
        // through the same methods as the legacy `handle_lib_key` arms.
        // Unfocused browsers leave every chord untouched; the central router
        // handles destination-independent behavior.
        if self.focused {
            match key.code {
                Key::Up | Key::Char('k') => {
                    let index = self.move_rows(-1);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                Key::Down | Key::Char('j') => {
                    let index = self.move_rows(1);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                Key::PageUp => {
                    let rows = -self.page_rows();
                    let index = self.move_rows(rows);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                Key::PageDown => {
                    let rows = self.page_rows();
                    let index = self.move_rows(rows);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                Key::Home => {
                    let index = self.jump_cursor(false);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                Key::End => {
                    let index = self.jump_cursor(true);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                // Column navigation applies only to a painted list with
                // more than one column (the legacy
                // `current_library_columns(lib_idx) > 1` guard). A
                // one-column list leaves Left/Right/h/l unbound locally,
                // matching `handle_lib_key`'s one-column behavior.
                Key::Left | Key::Char('h') if self.columns() > 1 => {
                    let index = self.move_cursor_delta(-1);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                Key::Right | Key::Char('l') if self.columns() > 1 => {
                    let index = self.move_cursor_delta(1);
                    return Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }));
                }
                _ => {}
            }
        }
        // The selected-item effects resolve targets from the component's own
        // local cursor/content and return typed requests. `focused` preserves
        // the legacy Library-panel gate exactly; an empty list or an
        // unclaimed chord returns `None` for the central router to handle.
        if self.focused {
            let selected = self.selected_effect_item();
            let request = match key.code {
                Key::Enter => selected.map(|item| ShellRequest::BrowserActivate { item }),
                Key::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    selected.map(|item| ShellRequest::BrowserPlay { item })
                }
                Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    selected.map(|item| ShellRequest::BrowserEnqueue { item })
                }
                Key::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    selected.map(|item| ShellRequest::BrowserToggleWatched { item })
                }
                // Bare `.` opens the context menu for the component-selected
                // item (task 5.3d, Emby browser context-menu decoupling).
                // Modified `.` (e.g. Ctrl+.) is not claimed here.
                Key::Char('.') if key.modifiers.is_empty() => {
                    selected.map(|item| ShellRequest::BrowserContextMenu { item })
                }
                // Ctrl+S shuffles the component-selected item. Control-
                // modifier guarded exactly as the legacy `handle_lib_key`
                // arm; with no selected item this chord remains unclaimed.
                Key::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    selected.map(|item| ShellRequest::BrowserShuffle { item })
                }
                // Ctrl+`r` rescans the focused library; bare `r` refreshes
                // it. The CONTROL arm comes first so it can never be shadowed
                // by the bare arm, preserving legacy precedence.
                Key::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(ShellRequest::BrowserRescan)
                }
                Key::Char('r') => Some(ShellRequest::BrowserRefresh),
                // Esc or Backspace go back through the browse history (task
                // 5.3d, Emby browser back): uses a typed request for the
                // focused browser. No modifier guard — the legacy
                // `handle_lib_key` `Esc | Backspace` arm matched any
                // modifiers, so this preserves that modifier-insensitive
                // behavior exactly. The shell owns the effect (`go_back`) and
                // derives the active library index from its own tab state.
                Key::Esc | Key::Backspace => Some(ShellRequest::BrowserBack),
                // `[`/`]` cycle the letter-range pill row for the focused
                // generic/Movies/home-video browser. A typed request carries
                // the delta, and the shell derives the active Emby library
                // index from its own tab state.
                Key::Char(c @ ('[' | ']'))
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    let delta = if c == '[' { -1 } else { 1 };
                    // The shell-projected content decides which pill row this
                    // chord drives: a feed/home-video group picker
                    // (`is_feed_home_video_group_view`, task 2.2) cycles its
                    // group pills; every other browse surface cycles its
                    // letter-range pills.
                    Some(if self.context.has_group_pills() {
                        ShellRequest::BrowserCycleGroup { delta }
                    } else {
                        ShellRequest::BrowserCycleLetterPill { delta }
                    })
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
        None
    }

    /// Resolve the item at the component's own local cursor over the mirrored
    /// content. The local cursor is authoritative for effect targets; no App
    /// cursor is re-read.
    fn selected_effect_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.context
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll)
            .selected_item()
            .cloned()
    }

    /// Columns the last painted list packs per row for cursor movement
    /// (task 5.3d prep): the wide Movies/home-videos hero-on-left right
    /// rail is always one column (mirroring `App::current_library_columns`,
    /// selected by this component's own `kind` and painted geometry in
    /// `view()`), otherwise the pane-derived `library_column_count` of the
    /// painted list area. The
    /// Browser mount gate excludes the TV (wide TV, season grids) and feed
    /// home-video-group special cases, so no other legacy branch applies to
    /// this component.
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
                    return Some(Msg::Shell(ShellRequest::BrowserScroll { delta }));
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
