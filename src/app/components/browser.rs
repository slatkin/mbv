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
use tuirealm::event::{Event, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{BrowserHitRegion, LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{
    library_cell_width, library_column_count, LIBRARY_COLUMN_GAP,
};
use crate::app::palette;
use crate::app::render::{
    hero_on_left_list_panel_border, hero_on_left_right_pane, padded_rect,
    prepare_wide_emby_hero_card, render_count_label,
    render_generic_movies_home_video_rows_with_ctx, render_home_hero_content, render_pill_bar,
    render_search_box, shared_hero_presentation, wide_library_panes, HeroData, HomeImagePaint,
    LetterFilter, LibraryListRenderCtx, PillBar, PANE_PAD_X, PANE_PAD_Y,
};
use crate::app::ui_util::move_cursor;

pub struct BrowserComponent {
    context: LibraryListRenderCtx,
    cursor: usize,
    scroll: usize,
    focused: bool,
    layout: LayoutMain,
    /// Shell projection (task 5.3d prep, D14 temporary adapter): whether the
    /// current App layout rendered the dedicated Movies/home-videos
    /// hero-on-left right rail. The component's own `LayoutMain` does not
    /// publish `movies_wide_right_area` (the render seam is given only the
    /// list rect), so the shell mirrors `App::layout.main`
    /// `.is_wide_movies_active()` here each sync — the same signal legacy
    /// `App::current_library_columns` reads to force the right rail to one
    /// column for both the wide Movies library and the wide home-videos
    /// presentation (both populate `movies_wide_right_area`).
    wide_movies: bool,
    /// Whether the wide layout's pill row is a home-video count label (vs. a
    /// letter-range pill row). Fed by the shell each draw (task 5.3d.17a).
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
}

impl BrowserComponent {
    pub fn new() -> Self {
        Self {
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
        }
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

    /// Shell projection (task 5.3d prep, D14 temporary adapter): record
    /// whether the current App layout is the wide Movies/home-videos
    /// hero-on-left presentation (`App::layout.main.is_wide_movies_active()`
    /// — `movies_wide_right_area` is set by the wide renderer for both
    /// dedicated Movies and home-videos libraries), so `columns()` returns
    /// one exactly like the legacy `App::current_library_columns` does. The
    /// component cannot read this from its own `LayoutMain`: the render seam
    /// is given only the list rect and never publishes the rail geometry —
    /// the same reason `MusicWorkspaceComponent` has its page size pushed in
    /// (`set_page_rows`). `home_video`/`letter_pills` tell the component which
    /// pill row to paint in the wide right rail (task 5.3d.17a).
    pub(in crate::app) fn set_wide_movies(
        &mut self,
        wide: bool,
        home_video: bool,
        letter_pills: bool,
    ) {
        self.wide_movies = wide;
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
        let left_content_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        let Some(panes) = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y) else {
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

    /// Renders the letter-range pill row (task 5.3d.17a): a direct copy of
    /// `App::render_letter_pills_row` (screens/pills.rs) using the component's
    /// own `letter_filter`, so the wide right rail's pills no longer depend on
    /// the legacy renderer.
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

    pub(in crate::app) fn handle_crossterm_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<Msg> {
        match key.code {
            crossterm::event::KeyCode::Char('/') if key.modifiers.is_empty() => {
                return Some(Msg::Shell(super::msg::ShellRequest::OpenInlineSearch));
            }
            _ => {}
        }
        // Local keyboard navigation routes through typed `ShellRequest`s
        // (task 5.3d): the component mutates only its own `self.cursor`
        // through the row/column helpers below, then returns the typed
        // request in place of the raw key so the shell drives the App
        // cursor through the same `App::move_lib_cursor_rows` /
        // `App::move_lib_cursor` / `App::jump_lib_cursor` methods the
        // legacy `handle_lib_key` movement arms call — never in addition to
        // the raw key (no double movement). Focused-gated exactly like the
        // legacy Library-panel gate: while unfocused (Queue/playback own
        // panel focus) the component does not touch its cursor and the raw
        // key passes through unchanged, keeping those surfaces
        // authoritative.
        if self.focused {
            match key.code {
                crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                    self.move_rows(-1);
                    return Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows: -1 }));
                }
                crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                    self.move_rows(1);
                    return Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows: 1 }));
                }
                crossterm::event::KeyCode::PageUp => {
                    let rows = -self.page_rows();
                    self.move_rows(rows);
                    return Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows }));
                }
                crossterm::event::KeyCode::PageDown => {
                    let rows = self.page_rows();
                    self.move_rows(rows);
                    return Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows }));
                }
                crossterm::event::KeyCode::Home => {
                    self.jump_cursor(false);
                    return Some(Msg::Shell(ShellRequest::BrowserJumpCursor {
                        to_end: false,
                    }));
                }
                crossterm::event::KeyCode::End => {
                    self.jump_cursor(true);
                    return Some(Msg::Shell(ShellRequest::BrowserJumpCursor { to_end: true }));
                }
                // Column navigation applies only to a painted list with
                // more than one column (the legacy
                // `current_library_columns(lib_idx) > 1` guard): a
                // one-column list leaves Left/Right/h/l unbound locally
                // (legacy `handle_lib_key` does not claim them in 1-col
                // either — they fall through to other CONTEXT_STACK
                // handlers), so they emit no movement request and the raw
                // key is still forwarded to the legacy bridge below.
                crossterm::event::KeyCode::Left | crossterm::event::KeyCode::Char('h')
                    if self.columns() > 1 =>
                {
                    self.move_cursor_delta(-1);
                    return Some(Msg::Shell(ShellRequest::BrowserMoveColumn { delta: -1 }));
                }
                crossterm::event::KeyCode::Right | crossterm::event::KeyCode::Char('l')
                    if self.columns() > 1 =>
                {
                    self.move_cursor_delta(1);
                    return Some(Msg::Shell(ShellRequest::BrowserMoveColumn { delta: 1 }));
                }
                _ => {}
            }
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
                // Ctrl+`r` rescans the focused library; bare or Alt+`r`
                // refreshes it (task 5.3d, Emby browser refresh/rescan). The
                // CONTROL arm comes first so it can never be shadowed by the
                // bare arm below, and the bare arm also covers Alt+`r` (no
                // CONTROL modifier), exactly matching the legacy `handle_lib_key`
                // ordering — Alt+`r` refreshes, it does not rescan. Neither
                // carries a selected item: the shell derives the active library
                // index from its own tab state. Any other modified character is
                // forwarded to the legacy bridge below.
                crossterm::event::KeyCode::Char('r')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    Some(ShellRequest::BrowserRescan)
                }
                crossterm::event::KeyCode::Char('r') => Some(ShellRequest::BrowserRefresh),
                // Esc or Backspace go back through the browse history (task
                // 5.3d, Emby browser back): moves off `Msg::Legacy` for the
                // focused browser. No modifier guard — the legacy
                // `handle_lib_key` `Esc | Backspace` arm matched any
                // modifiers, so this preserves that modifier-insensitive
                // behavior exactly. The shell owns the effect (`go_back`) and
                // derives the active library index from its own tab state.
                crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Backspace => {
                    Some(ShellRequest::BrowserBack)
                }
                // `[`/`]` cycle the letter-range pill row for the focused
                // generic/Movies/home-video browser (task 5.3d, Emby browser
                // selector cycling): a typed request carries the delta (-1 for
                // `[`, +1 for `]`), and the shell derives the active Emby
                // library index from its own tab state and runs
                // `App::cycle_letter_pill` on it — whose existing
                // `should_show_letter_pills` no-op guard and wrap/select
                // behavior are preserved unchanged. That is the whole effect
                // for this component: its mount gate already excludes Music
                // and feed-home-video group views, the two branches the
                // legacy `handle_key_emby_library` consumed before it reached
                // the letter pills. Neither CONTROL nor ALT (exactly the
                // legacy guard); Ctrl/Alt brackets fall through to the legacy
                // bridge below via `_ => None`.
                crossterm::event::KeyCode::Char(c @ ('[' | ']'))
                    if !key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                        && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                {
                    let delta = if c == '[' { -1 } else { 1 };
                    Some(ShellRequest::BrowserCycleLetterPill { delta })
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
        Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
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

    /// Columns the last painted list packs per row for cursor movement
    /// (task 5.3d prep): the wide Movies/home-videos hero-on-left right
    /// rail is always one column (mirroring `App::current_library_columns`,
    /// fed in by the shell via `set_wide_movies` — the component's own
    /// layout does not publish `movies_wide_right_area`), otherwise the
    /// pane-derived `library_column_count` of the painted list area. The
    /// Browser mount gate excludes the TV (wide TV, season grids) and feed
    /// home-video-group special cases, so no other legacy branch applies to
    /// this component.
    fn columns(&self) -> usize {
        if self.wide_movies {
            1
        } else {
            library_column_count(self.layout.left_area.width)
        }
    }

    /// Painted item rows the pager moves per PageUp/PageDown, mirroring
    /// `App::lib_page_size`: the painted list area's height minus its top
    /// count/search header line, floored at one row (list rows are
    /// single-line).
    fn page_rows(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    /// Move the component-local cursor `item_rows` displayed item rows down
    /// (positive) or up (negative), mirroring `App::move_lib_cursor_rows` for
    /// the generic/Movies/home-video browser (task 5.3d prep): letter-
    /// grouped lists resolve the target through the last painted
    /// `left_item_rows`/`left_sorted_indices` (headers/gaps skipped, a
    /// ragged target row falls back to its last item), and flat lists stride
    /// by the painted column count. The legacy stale-layout fallback
    /// (sorted present but cursor unpainted) moves in sorted order by the
    /// multiplied delta, exactly like `App`.
    fn move_rows(&mut self, item_rows: i64) {
        if !self.layout.left_sorted_indices.is_empty() {
            if let Some(delta) = self.letter_vertical_delta(item_rows) {
                self.move_cursor_delta(delta);
                return;
            }
        }
        self.move_cursor_delta(item_rows * self.columns() as i64);
    }

    /// Move the component-local cursor by `delta` items, mirroring
    /// `App::move_lib_cursor`: sorted display order when the last painted
    /// list is letter-grouped, raw item order otherwise.
    fn move_cursor_delta(&mut self, delta: i64) {
        if !self.layout.left_sorted_indices.is_empty() {
            self.move_sorted_cursor(delta);
        } else {
            self.move_raw_cursor(delta);
        }
    }

    /// Move in the letter-grouped display order: the cursor's position in
    /// `left_sorted_indices` is the authority, exactly as
    /// `App::move_lib_cursor_inner`'s sorted branch moves the App cursor.
    fn move_sorted_cursor(&mut self, delta: i64) {
        let sorted = &self.layout.left_sorted_indices;
        if sorted.is_empty() {
            return;
        }
        let pos = sorted.iter().position(|&i| i == self.cursor).unwrap_or(0);
        let new_pos = move_cursor(pos, delta, sorted.len());
        self.cursor = sorted[new_pos];
    }

    /// Move in raw item order, mirroring `App::move_lib_cursor_inner`'s
    /// fallback branch on `lvl.items.len()`; a zero-count list stays put.
    fn move_raw_cursor(&mut self, delta: i64) {
        let count = self.context.item_count();
        if count > 0 {
            self.cursor = move_cursor(self.cursor, delta, count);
        }
    }

    /// Flat (sorted-order) delta that lands the component cursor on the
    /// item `item_rows` rows up/down from its current display row, per the
    /// last painted item rows — the component-local mirror of
    /// `App::letter_vertical_delta` (which reads the App nav cursor; this
    /// reads the component's own `self.cursor`). Headers/spacers/fillers do
    /// not participate: the target is the `item_rows`-th *item row* away,
    /// keeping the cursor's column (a ragged target row falls back to its
    /// last item; moving past the end clamps to the last item). `None` when
    /// the layout is stale (cursor not found), letting the caller fall back
    /// to flat arithmetic.
    fn letter_vertical_delta(&self, item_rows: i64) -> Option<i64> {
        let all_rows = &self.layout.left_item_rows;
        if all_rows.is_empty() || self.layout.left_sorted_indices.is_empty() {
            return None;
        }
        let item_row_list: Vec<&Vec<usize>> = all_rows.iter().filter(|r| !r.is_empty()).collect();
        if item_row_list.is_empty() {
            return None;
        }
        let (cur_row, cur_col) = item_row_list.iter().enumerate().find_map(|(r, row)| {
            row.iter()
                .position(|&i| i == self.cursor)
                .map(|col| (r, col))
        })?;
        let row_count = item_row_list.len();
        let target_row = if item_rows < 0 {
            cur_row.saturating_sub(item_rows.unsigned_abs() as usize)
        } else {
            cur_row
                .saturating_add(item_rows as usize)
                .min(row_count.saturating_sub(1))
        };
        let target = item_row_list[target_row]
            .get(cur_col)
            .copied()
            .or_else(|| item_row_list[target_row].last().copied())?;

        // Single pass over `sorted` for both positions instead of two
        // separate `.position()` scans — this runs on every j/k/Up/Down
        // keypress in letter-grouped view, so halving the work (and
        // early-exiting once both are found) matters on large libraries.
        let mut cur_pos = None;
        let mut target_pos = None;
        for (pos, &idx) in self.layout.left_sorted_indices.iter().enumerate() {
            if idx == self.cursor {
                cur_pos = Some(pos);
            }
            if idx == target {
                target_pos = Some(pos);
            }
            if cur_pos.is_some() && target_pos.is_some() {
                break;
            }
        }
        Some(target_pos? as i64 - cur_pos? as i64)
    }

    /// Home/End jump to the first/last item in sorted display order when
    /// the last painted list is letter-grouped, else the raw first/last —
    /// mirroring `App::jump_lib_cursor` minus the feed-home-video-group
    /// branch the Browser mount gate excludes.
    fn jump_cursor(&mut self, to_end: bool) {
        if !self.layout.left_sorted_indices.is_empty() {
            let n = self.layout.left_sorted_indices.len();
            self.cursor = self.layout.left_sorted_indices[if to_end { n - 1 } else { 0 }];
        } else {
            let count = self.context.item_count();
            if count > 0 {
                self.cursor = if to_end { count - 1 } else { 0 };
            }
        }
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
        Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
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
        let context = self
            .context
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll);
        // Task 5.3d.17a: when the wide Movies/home-video hero-on-left layout
        // is active (shell projection `wide_movies` AND the area is wide
        // enough for the shared split), paint the full hero + pills + list
        // layout itself instead of just the inner list rows; otherwise keep
        // the narrow list-row behavior.
        let wide = self.wide_movies && shared_hero_presentation(area).is_some();
        self.scroll = if wide {
            self.render_wide_movies(frame, area, &context)
        } else {
            // Reset image_paint to None in narrow layout to prevent stale
            // hero images from being painted after a wide→narrow resize
            // (reviewer P1 finding).
            self.image_paint = None;
            render_generic_movies_home_video_rows_with_ctx(
                frame,
                area,
                &context,
                self.focused,
                &mut self.layout,
            )
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
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
