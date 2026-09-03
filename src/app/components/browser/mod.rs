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

use mbv_core::api::EmbyItem;

use super::browser_narrow::NarrowBrowseExtras;
use super::component_id::BrowserKind;
use super::media_list::{
    InlineMediaBrowser, MediaListRow, MediaSemanticState, ViewportAnchor, WideMediaList,
};
use super::msg::{BrowserHitRegion, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::render::{effective_sort_str, shared_hero_presentation, HomeImagePaint};

mod content;
mod keyboard;
mod navigation;
mod paint;

pub(in crate::app) use content::{BrowserContent, BrowserIdentity};

pub struct BrowserComponent {
    kind: BrowserKind,
    /// Position-free content the shell pushed (task 3.7). The legacy
    /// `LibraryListRenderCtx` is rebuilt on demand from this plus the
    /// control-owned `cursor`/`scroll` at a single private site.
    context: BrowserContent,
    /// The browse identity the last shell content push carried (task 3.7).
    /// `push_emby_browser_content` re-seeds position through `apply_position`
    /// only when this changes; within one identity (pagination, loading
    /// completion, refresh, cursor echo) no position crosses the boundary.
    last_identity: Option<BrowserIdentity>,
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
    preserved_anchor: Option<ViewportAnchor<String>>,
    /// Persistent canonical control for the applicable Hero-on-left Wide rails
    /// (Movies, home-video feed view). Fed from `set_content`, painted by
    /// `render_wide_movies`. Targets are item indices into `context.items`
    /// (Browser's existing typed row identity); task 3.7 removes the mirrored
    /// cursor/scroll, task 3.5c re-points navigation onto this control.
    wide_list: WideMediaList<usize>,
    /// Persistent canonical control for the applicable Narrow hero-bearing
    /// browse paths. Driven by `render_narrow_browse_with_ctx` instead of a
    /// per-frame `InlineMediaBrowser::new()`.
    inline_browser: InlineMediaBrowser<usize>,
}

impl BrowserComponent {
    pub fn new() -> Self {
        Self::new_for_kind(BrowserKind::Generic)
    }

    pub fn new_for_kind(kind: BrowserKind) -> Self {
        Self {
            kind,
            context: BrowserContent::default(),
            last_identity: None,
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
            preserved_anchor: None,
            wide_list: WideMediaList::new(),
            inline_browser: InlineMediaBrowser::new(),
        }
    }

    /// Records the shell-resolved narrow-browse extras for the next `view()`
    /// (task 3.3). Pushed each frame by `render_emby_browser_component`.
    pub(in crate::app) fn set_narrow_extras(&mut self, extras: NarrowBrowseExtras) {
        self.narrow_extras = extras;
    }

    /// Records the position-free content push (task 3.7). Carries no cursor or
    /// scroll: an ordinary push never moves the control. Position is re-seeded
    /// only through the identity-gated `apply_position`. The one exception is
    /// the `ViewportAnchor` breakpoint seam, whose preserved target is
    /// re-resolved against the new item list here.
    pub(in crate::app) fn set_content(&mut self, content: BrowserContent, focused: bool) {
        self.context = content;
        self.focused = focused;
        let anchor_target = self
            .preserved_anchor
            .as_ref()
            .map(|anchor| anchor.selected_target.clone());
        if let Some(cursor) = anchor_target
            .and_then(|target| self.context.items.iter().position(|item| item.id == target))
        {
            self.cursor = cursor;
        }
        // Clamp the control-owned cursor to the new item count (a within-identity
        // refresh may return fewer items, e.g. inline search). This keeps the
        // invariant, not a position re-seed: `BrowserContent` has no cursor.
        self.cursor = self.cursor.min(self.context.item_count().saturating_sub(1));
        self.feed_wide_list();
    }

    /// Explicit, identity-gated resting-position re-seed (task 3.7). The shell
    /// calls this from `push_emby_browser_content` ONLY when the browse
    /// identity changed (drill-in, go-back parent restore, letter-filter
    /// reset, sort change, feed/home-video group switch). Within one identity
    /// no position crosses the boundary, so pagination, loading completion,
    /// ordinary refresh, and the component's own `BrowserCursorIndex` echo
    /// leave the control-owned cursor and scroll untouched.
    pub(in crate::app) fn apply_position(&mut self, cursor: usize, scroll: usize) {
        self.cursor = cursor.min(self.context.item_count().saturating_sub(1));
        self.scroll = scroll;
        self.feed_wide_list();
    }

    /// Records the browse identity of the current shell content push and
    /// reports whether it differs from the previous push for this browser
    /// (task 3.7). A `true` result gates the `apply_position` re-seed.
    pub(in crate::app) fn note_browse_identity(&mut self, identity: BrowserIdentity) -> bool {
        let changed = self.last_identity.as_ref() != Some(&identity);
        self.last_identity = Some(identity);
        changed
    }

    /// Rebuild the persistent `WideMediaList` from the mirrored content for the
    /// applicable Wide rails (Movies, home-video, feed-group view), mirroring
    /// the routing `render_generic_movies_home_video_rows_with_ctx` applied:
    /// letter-grouped rows for a search-free library at or above 50 items (or
    /// with an active letter pill), plain rows otherwise. Non-applicable
    /// kinds (non-hero two-column Generic, Music, books) leave the control
    /// untouched; `view()` never paints it for them.
    fn feed_wide_list(&mut self) {
        if !(matches!(self.kind, BrowserKind::Movies | BrowserKind::HomeVideos)
            || self.context.has_group_pills())
        {
            return;
        }
        let ctx = &self.context;
        let row_for = |index: usize, item: &EmbyItem| -> MediaListRow<usize> {
            let primary = if item.is_folder && item.item_type == "Folder" && item.total_count > 0 {
                format!("{} \u{b7} {} items", item.display_name(), item.total_count)
            } else if item.is_folder && item.unplayed_item_count > 0 && item.item_type != "Series" {
                format!("{} [{}]", item.display_name(), item.unplayed_item_count)
            } else {
                item.display_name()
            };
            MediaListRow::Item {
                target: index,
                primary,
                trailing: (!item.is_folder && item.production_year > 0)
                    .then(|| item.production_year.to_string()),
                duration: None,
                // The legacy Wide rail painters colour every row through
                // `focused_or_subtle` with no played/active dimming.
                semantic_state: MediaSemanticState::Ordinary,
            }
        };
        let grouped =
            !ctx.is_search_active() && (ctx.true_total() >= 50 || ctx.letter_filter.is_some());
        if grouped {
            let items = ctx
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| (effective_sort_str(item).to_string(), row_for(index, item)))
                .collect();
            self.wide_list.set_letter_grouped_content(
                items,
                ctx.true_total(),
                ctx.letter_filter.is_some(),
            );
        } else {
            let rows = ctx
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| row_for(index, item))
                .collect();
            self.wide_list.set_content(rows);
        }
        let cursor = self.cursor.min(ctx.item_count().saturating_sub(1));
        self.wide_list.select_target(&cursor);
        self.wide_list.set_scroll(self.scroll);
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    /// The scroll offset the last `view()` painted the list at. The control
    /// owns it: `set_content` carries no position, so an ordinary content
    /// push never overwrites it. The shell reads this back only at navigation
    /// events (folder drill-in, `BrowserBack`) and teardown, through
    /// `persist_emby_browser_scroll` -> `persist_library_scroll`, to record
    /// the shell-owned resting position (design D3). It is not a per-frame
    /// mirror.
    pub(in crate::app) fn scroll(&self) -> usize {
        self.scroll
    }

    pub(in crate::app) fn viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        self.active_viewport_anchor(viewport_height).or_else(|| {
            let item = self.context.items.get(self.cursor)?;
            Some(ViewportAnchor {
                selected_target: item.id.clone(),
                selected_row_offset: self
                    .cursor
                    .saturating_sub(self.scroll)
                    .min(viewport_height.saturating_sub(1)),
            })
        })
    }

    pub(in crate::app) fn apply_viewport_anchor(&mut self, anchor: ViewportAnchor<String>) {
        // Apply the explicit target immediately when content is already loaded;
        // the painted view still consumes the pending anchor to place the row.
        if let Some(cursor) = self
            .context
            .items
            .iter()
            .position(|item| item.id == anchor.selected_target)
        {
            self.cursor = cursor;
        }
        self.preserved_anchor = Some(anchor.clone());
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
            if !self.apply_active_viewport_anchor(&anchor, area.height as usize) {
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
        }
        let mut context = self
            .context
            .clone()
            .with_cursor_scroll(self.cursor, self.scroll);
        if let Some(items) = self.narrow_extras.feed_items.as_ref() {
            let feed = BrowserContent {
                items: items.clone(),
                total_count: items.len(),
                group_pills: true,
                loading: context.loading,
                ..BrowserContent::default()
            };
            context = feed
                .with_cursor_scroll(self.cursor.min(items.len().saturating_sub(1)), self.scroll);
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
                &mut self.inline_browser,
            );
            self.image_paint = image_paint;
            // Keep the active control's resting viewport in lockstep with the
            // painter's resolved flow; the parent field remains only the
            // shell's navigation/teardown persistence seam.
            if self.uses_inline_control() {
                self.inline_browser.set_scroll(scroll);
            }
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
