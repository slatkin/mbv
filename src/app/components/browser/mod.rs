//! Interactive Component for the generic Emby browser rows.
//!
//! The shell projects the active list source into this component. Generic,
//! Movies, and home-video rows use this one logical shared media-list owner
//! per mounted component (design.md D1): a non-hero generic catalog paints
//! the Grid presentation, Movies/home-video change between the Wide and
//! Inline presentations across the hero breakpoint, and a hero-bearing
//! generic catalog paints Inline — never two owners side by side. Music,
//! TV/series workspaces, and album-track presentation remain on their own
//! paths until their owning tasks convert them.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use mbv_core::api::EmbyItem;

use super::browser_narrow::NarrowBrowseControl;
use super::browser_narrow::NarrowBrowseExtras;
use super::component_id::BrowserKind;
use super::inline_search::{InlineSearch, InlineSearchHost, InlineSearchMouse};
use super::media_list::{
    letter_grouped_rows, MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState,
    Presentation, ViewportAnchor,
};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{Msg, ShellRequest, TerminalObserverEvent};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::render::{effective_sort_str, wide_hero_presentation, HomeImagePaint};

mod content;
mod keyboard;
mod navigation;
mod paint;
mod state;

pub(in crate::app) use content::{BrowserContent, BrowserIdentity};

pub struct BrowserComponent {
    kind: BrowserKind,
    /// Position-free content pushed by the shell.
    context: BrowserContent,
    /// Identity carried by the last shell content push; it gates re-anchoring.
    last_identity: Option<BrowserIdentity>,
    focused: bool,
    layout: LayoutMain,
    /// Whether the component's kind and painted geometry select Wide hero layout.
    wide_movies: bool,
    /// Whether the wide layout's pill row is a home-video count label.
    wide_movies_home_video: bool,
    /// Whether the wide layout shows the letter-range pill row.
    wide_movies_letter_pills: bool,
    /// Runtime terminal-capability flag (config-derived), set by the shell so
    /// the component can paint the hero text like every other surface.
    use_nerd_fonts: bool,
    images_enabled: bool,
    /// The hero cover image `view()` computed but could not paint itself (no
    /// `App`/image-cache authority); the shell takes it right after
    /// `application.view()` and paints it via `App::paint_home_image`
    /// (mirrors `HomeComponent`, task 5.3d.17a).
    image_paint: Option<HomeImagePaint>,
    /// Shell-resolved narrow-browse extras (count label, letter pills, inline
    /// movie/series hero) for the `browser_narrow` composer, pushed each frame
    /// by `render_emby_browser_component` (task 3.3).
    narrow_extras: NarrowBrowseExtras,
    /// Discrete navigation/restoration re-anchor (design.md D1): re-selects
    /// its target when new content makes it available again.
    preserved_anchor: Option<ViewportAnchor<String>>,
    /// The one shared canonical owner of this logical row flow, carried by
    /// exactly one of the three persistent presentations (design.md D1).
    carrier: MediaListCarrier<String>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
    /// Irregular painted chrome — the selector-pill row — as last-push-wins
    /// rectangles (design.md D6). Repopulated in `view()` from the pill rects
    /// the narrow/wide composer just painted into `self.layout.selector_tabs`.
    pill_regions: HitRegions<usize>,
    /// The embedded Inline Search control (design.md D1). `BrowserComponent`
    /// is its sole event boundary: it gets keyboard/mouse first refusal while
    /// active and is painted at the existing list composition point instead
    /// of the ordinary rows. Section 2 embeds this alongside the still-live
    /// to talk to this control instead and Section 4 deletes the overlay.
    inline_search: InlineSearch,
}

/// Derives the Emby-specific semantic state for a browse row. Both projection
/// sites (Inline and Wide) call this so the two trees cannot drift; the
/// provider-neutral `media_list` layer deliberately stays free of `EmbyItem`.
fn emby_semantic_state(item: &EmbyItem) -> MediaSemanticState {
    if item.playback_position_ticks > 0 && !item.played {
        let progress = if item.runtime_ticks > 0 {
            Some(
                ((item.playback_position_ticks as u64 * 100) / item.runtime_ticks as u64).min(100)
                    as u16,
            )
        } else {
            None
        };
        MediaSemanticState::active(progress)
    } else if item.played {
        MediaSemanticState::Played
    } else {
        MediaSemanticState::Ordinary
    }
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
            focused: false,
            layout: LayoutMain::default(),
            wide_movies: false,
            wide_movies_home_video: false,
            wide_movies_letter_pills: false,
            use_nerd_fonts: false,
            images_enabled: true,
            image_paint: None,
            narrow_extras: NarrowBrowseExtras::default(),
            preserved_anchor: None,
            carrier: MediaListCarrier::new(Presentation::Inline),
            mouse_gestures: MouseGestureState::new(),
            pill_regions: HitRegions::new(),
            inline_search: InlineSearch::new(),
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
    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_content(&mut self, content: BrowserContent) {
        self.context = content;
        self.feed_owner();
        self.reanchor_content();
    }
    /// Explicit, identity-gated resting-position re-seed (task 3.7). The shell
    /// calls this from `push_emby_browser_content` ONLY when the browse
    /// identity changed (drill-in, go-back parent restore, letter-filter
    /// reset, sort change, feed/home-video group switch). Within one identity
    /// no position crosses the boundary, so pagination, loading completion,
    /// ordinary refresh, and the component's own `BrowserCursorIndex` echo
    /// leave the owner-owned selection and scroll untouched.
    pub(in crate::app) fn apply_position(&mut self, cursor: usize, scroll: usize) {
        self.feed_owner();
        self.ensure_carrier();
        let target = self
            .context
            .items
            .get(cursor.min(self.context.item_count().saturating_sub(1)))
            .map(|item| item.id.clone());
        if let Some(target) = target.as_ref() {
            self.carrier.select_target(target);
        }
        self.carrier.set_scroll(scroll);
    }

    /// Records the browse identity of the current shell content push and
    /// reports whether it differs from the previous push for this browser
    /// (task 3.7). A `true` result gates the `apply_position` re-seed.
    pub(in crate::app) fn note_browse_identity(&mut self, identity: BrowserIdentity) -> bool {
        let changed = self.last_identity.as_ref() != Some(&identity);
        self.last_identity = Some(identity);
        changed
    }
    /// Project the mirrored content into provider-neutral rows: letter-grouped
    /// `Heading`/`Spacer`/`Item` rows for a search-free library at or above 50
    /// items (or with an active letter pill), natural-sorted plain rows
    /// otherwise (the order every presentation over the shared owner shows).
    fn project_rows(&self) -> Vec<MediaListRow<String>> {
        let ctx = &self.context;
        let row_for = |item: &EmbyItem| -> MediaListRow<String> {
            let primary = if item.is_folder && item.item_type == "Folder" && item.total_count > 0 {
                format!("{} \u{b7} {} items", item.display_name(), item.total_count)
            } else if item.is_folder && item.unplayed_item_count > 0 && item.item_type != "Series" {
                format!("{} [{}]", item.display_name(), item.unplayed_item_count)
            } else {
                item.display_name()
            };
            let semantic_state = emby_semantic_state(item);
            MediaListRow::Item {
                target: item.id.clone(),
                primary,
                trailing: (!item.is_folder && item.production_year > 0)
                    .then(|| item.production_year.to_string()),
                duration: None,
                kind: MediaKind::Collection,
                semantic_state,
            }
        };
        let grouped =
            !ctx.is_search_active() && (ctx.true_total() >= 50 || ctx.letter_filter.is_some());
        if grouped {
            // Letter-grouped rows are naturally sorted with per-bucket
            // Heading/Spacer rows (design.md D2).
            let pairs: Vec<(String, MediaListRow<String>)> = ctx
                .items
                .iter()
                .map(|item| (effective_sort_str(item).to_string(), row_for(item)))
                .collect();
            letter_grouped_rows(pairs, ctx.true_total(), ctx.letter_filter.is_some())
        } else {
            // Plain rows keep the source order the shell projected: the
            // shell-owned resting cursor is a `context.items` index, and the
            // one owner's selectable index matches it exactly here.
            ctx.items.iter().map(row_for).collect()
        }
    }

    /// Feed the one shared owner (wherever it currently resides) from the
    /// mirrored content. `set_content` on the owner preserves the selected
    /// target and locally clamps otherwise (design.md D3).
    fn feed_owner(&mut self) {
        let rows = self.project_rows();
        self.carrier.set_content(rows);
    }

    /// The presentation the component's kind, breakpoint, and painted chrome
    /// select right now (design.md D2).
    fn active_presentation(&self) -> Presentation {
        if self.wide_movies {
            Presentation::Wide
        } else if self.uses_inline_control() {
            Presentation::Inline
        } else {
            Presentation::Grid
        }
    }

    /// Move the shared owner into the active presentation when they diverge.
    /// A responsive presentation change reads the same owner and preserves
    /// only the outgoing selected-row viewport offset (design.md D3); no
    /// cursor, scroll, or selection is ever copied between presentations.
    pub(in crate::app) fn ensure_carrier(&mut self) {
        let target = self.active_presentation();
        let viewport_height = self.painted_viewport_height();
        self.carrier.ensure(target, viewport_height);
    }
    /// Handle a mouse event against the component's painted browse geometry.
    ///
    /// Gesture recognition (click / double-click / right-click / wheel) comes
    /// from the private `MouseGestureState` (ADR 0024, design.md D3). Row
    /// identity comes only from the active presentation's retained
    /// current-frame geometry (design.md D6): the Wide/Inline/Grid adapters each
    /// resolve their own cells, and no parent row map or cell arithmetic runs
    /// beside them. The component emits a semantic `Msg` with a resolved
    /// target — never raw coordinates — except the context-menu anchor, which
    /// is display geometry it legitimately forwards (design.md D4).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // Inline Search gets first refusal while active (design.md D6): it
        // is painted over the same area the ordinary list would occupy, so
        // the ordinary list never mutates for points there.
        if self.inline_search.is_active() {
            return match self.inline_search.handle_mouse(mouse) {
                Some(InlineSearchMouse::ContextMenu) => self
                    .inline_search
                    .selected_item()
                    .map(|item| Msg::Shell(ShellRequest::BrowserContextMenu { item })),
                Some(InlineSearchMouse::Consumed) => {
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                None => None,
            };
        }
        // Browse does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                let claimed = self.carrier.claims_point(self.layout.left_area, at)
                    || self.layout.inline_hero_area.contains(at);
                if !claimed {
                    return None;
                }
                // The normalized gesture delta is one selectable row. The
                // existing cursor request preserves library-position writes;
                // the control itself owns the resulting viewport.
                let index = self.move_cursor_delta(delta);
                Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index }))
            }
            MouseGesture::Click(at) => {
                if let Some(&pill) = self.pill_regions.resolve(at) {
                    return Some(Msg::Shell(ShellRequest::BrowserPillClick { target: pill }));
                }
                if !self.claim_list_point(at) {
                    return None;
                }
                Some(Msg::Shell(ShellRequest::BrowserRowClick {
                    target: self.selected_row_target(),
                }))
            }
            MouseGesture::DoubleClick(at) => {
                if let Some(&pill) = self.pill_regions.resolve(at) {
                    return Some(Msg::Shell(ShellRequest::BrowserPillClick { target: pill }));
                }
                self.claim_list_point(at);
                self.selected_row_target().map(|target| {
                    Msg::Shell(ShellRequest::BrowserRowActivate {
                        target: Some(target),
                    })
                })
            }
            MouseGesture::RightClick(at) => {
                if !self.claim_list_point(at) {
                    return None;
                }
                Some(Msg::Shell(ShellRequest::BrowserRowContextMenu {
                    target: self.selected_row_target(),
                    anchor: (mouse.column, mouse.row),
                }))
            }
            MouseGesture::Drag { .. } | MouseGesture::DragEnd => None,
        }
    }

    /// If `at` lands inside the painted list or inline-hero region, move the
    /// selection to the row under it (a blank/gap click leaves the selection
    /// unchanged, matching the legacy behaviour) and return `true`.
    fn claim_list_point(&mut self, at: Position) -> bool {
        if !(self.layout.left_area.contains(at) || self.layout.inline_hero_area.contains(at)) {
            return false;
        }
        let Some(target) = self.resolve_row_target(at) else {
            return false;
        };
        self.ensure_carrier();
        self.carrier.select_target(&target)
    }

    /// The stable target under `point`, resolved only from the retained
    /// current-frame geometry of the canonical presentation that painted the
    /// active list (design.md D6). The inline hero covers the selected item,
    /// so a hero click carries the current selection.
    fn resolve_row_target(&self, point: Position) -> Option<String> {
        self.carrier.resolve_current_point(point).cloned()
    }

    fn selected_row_target(&self) -> Option<String> {
        self.carrier.selected_target().cloned()
    }

    #[cfg(test)]
    pub(crate) fn test_layout(&self) -> &LayoutMain {
        &self.layout
    }

    #[cfg(test)]
    pub(crate) fn test_inline_targets(&self) -> (Rect, Vec<Option<String>>) {
        let inline = self.carrier.inline();
        let base = inline.current_content_rect().unwrap_or_default();
        let detail = inline.current_detail_rect().map_or(0, |rect| rect.height);
        let area = Rect {
            y: base.y.saturating_add(detail),
            height: base.height.saturating_sub(detail),
            ..base
        };
        let offset = inline
            .current_flow_offset()
            .unwrap_or(0)
            .saturating_add(detail as usize);
        let targets = (offset..offset.saturating_add(area.height as usize))
            .map(|row| inline.current_flow_target_at(row).flatten().cloned())
            .collect();
        (area, targets)
    }

    #[cfg(test)]
    pub(crate) fn test_inline_target_position(&self, target: String) -> Option<Position> {
        let (area, _) = self.test_inline_targets();
        (0..area.height)
            .map(|row| Position::new(area.x, area.y + row))
            .find(|point| self.carrier.inline().resolve_current_point(*point) == Some(&target))
    }

    /// Test-only cursor seed (task 5.3d.16): `set_content` no longer mirrors
    /// the shell cursor, so tests position the authoritative owner selection
    /// directly before exercising navigation.
    #[cfg(test)]
    pub(crate) fn set_cursor_for_test(&mut self, cursor: usize) {
        if let Some(item) = self.context.items.get(cursor) {
            let target = item.id.clone();
            self.carrier.select_target(&target);
        }
    }

    /// Test-only reset of the private gesture recognizer, so a synchronous
    /// test loop can drive successive wheel/click events without the
    /// throttle/double-click window collapsing them.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }
}

impl Default for BrowserComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineSearchHost for BrowserComponent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }

    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}

impl Component for BrowserComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let wide = (matches!(self.kind, BrowserKind::Movies | BrowserKind::HomeVideos)
            || self.narrow_extras.feed_items.is_some())
            && wide_hero_presentation(area).is_some();
        self.wide_movies = wide;
        // One shared owner per logical row flow (design.md D1): a responsive
        // presentation change reconfigures the same owner and preserves only
        // the outgoing selected-row viewport offset — no owner-to-owner
        // anchor transfer, no cursor/scroll seeding from a shell mirror.
        self.ensure_carrier();
        self.layout = LayoutMain::default();
        let mut context = self
            .context
            .clone()
            .with_cursor_scroll(self.cursor(), self.scroll());
        if let Some(items) = self.narrow_extras.feed_items.as_ref() {
            let feed = BrowserContent {
                items: items.clone(),
                total_count: items.len(),
                group_pills: true,
                loading: context.loading,
                ..BrowserContent::default()
            };
            context = feed.with_cursor_scroll(
                self.cursor().min(items.len().saturating_sub(1)),
                self.scroll(),
            );
        }
        // Task 5.3d.17a: when the wide Movies/home-video Wide hero layout
        // is active (this component's own `kind` AND the area is wide enough
        // for the shared split), paint the full hero + pills + list layout
        // itself instead of just the inner list rows; otherwise keep the
        // narrow list-row behavior.
        if wide {
            self.render_wide_movies(frame, area, &context);
        } else if self.inline_search.is_active() {
            // Normal/non-Hero catalogs pass their whole list area to the
            // shared search painter (design.md D3); the ordinary narrow
            // composer does not also paint it.
            let items = self.inline_search.ordered_items();
            let query = self.inline_search.query().to_string();
            let loading = self.inline_search.loading();
            let cursor = self.inline_search.cursor();
            let scroll_in = self.inline_search.scroll();
            let areas = crate::app::render::arrangements::wide_hero::pill_bar_areas(area);
            let (pills_area, list_area) = (areas.pills_area, areas.content_area);
            let columns = crate::app::library_column_width::library_column_count(list_area.width);
            let new_scroll = crate::app::render::render_inline_search(
                frame,
                pills_area,
                list_area,
                &query,
                loading,
                items,
                cursor,
                scroll_in,
                self.focused,
                columns,
                self.inline_search.layout_mut(),
            );
            self.inline_search.set_scroll(new_scroll);
            self.image_paint = None;
        } else {
            // Narrow generic/Movies/home-video: the component owns the full
            // surface via the `browser_narrow` composer (task 3.3). The active
            // canonical presentation (Inline or Grid) paints the rows; the
            // composer returns only the poster image still needing paint (the
            // shell executes it via `App::paint_home_image`, mirroring the
            // wide path and `HomeComponent`).
            let control = if self.uses_inline_control() {
                NarrowBrowseControl::Inline(self.carrier.inline_mut())
            } else {
                NarrowBrowseControl::Grid(self.carrier.grid_mut())
            };
            let (_scroll, image_paint) = crate::app::render::render_narrow_browse_with_ctx(
                frame,
                area,
                &context,
                &self.narrow_extras,
                self.focused,
                &mut self.layout,
                control,
            );
            self.image_paint = image_paint;
        }

        // Adopt the selector-pill rects the composer just painted into the
        // irregular-chrome registry (design.md D6). `selector_tabs` is the
        // composer's own painted output for this frame.
        self.pill_regions.clear();
        for (rect, target) in &self.layout.selector_tabs {
            self.pill_regions.push(*rect, *target);
        }
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for BrowserComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        // Keep the shared owner in the presentation the component currently
        // selects before any row-local input touches it (design.md D1).
        match event {
            Event::Keyboard(key) => {
                self.ensure_carrier();
                self.handle_tui_key(*key)
            }
            Event::Mouse(mouse) => {
                self.ensure_carrier();
                self.handle_mouse(mouse)
            }
            _ => None,
        }
    }
}
