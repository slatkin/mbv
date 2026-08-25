//! Interactive Component for the cross-Service Home destination.
//!
//! Task 3.4's confirmed scope is the render conversion: this component owns
//! the flat cursor spanning Continue Watching + Newest sections, the
//! selected section (pill), the list scroll offset, and painted geometry
//! (row hitmap, pill targets), and paints via the shared
//! `render_home_content` orchestration (`render/components/home.rs`) so it
//! cannot drift from the legacy `App::render_home_list` path. Keyboard/mouse
//! input authority stays on the legacy `App::handle_key`/`handle_mouse`
//! `CONTEXT_STACK` for now (converting it safely requires the precedence-
//! safe TuiRealm subscription wiring `key_policy.rs` documents as deferred
//! to per-surface conversion, not yet built). Content is mirrored from the
//! shell, while cursor/section/scroll state remains local to this component.
//! `handle_crossterm_key`/`handle_crossterm_mouse` and the typed
//! `ShellRequest::Home*` messages below are the live input path: as the
//! Library parent's active child on the Home tab, the component receives
//! terminal events and claims Home's own gestures (task 5.3d, home hit_test;
//! keyboard still falls through to the legacy `App::handle_key`).

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyEvent, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{HomeHitRegion, LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::HomeImagePaint;
use crate::app::types_playback::HomeLatestSource;
use crate::app::ui_util::move_cursor;
use mbv_core::playback_queue::QueueItem;

/// The Interactive Component for the Home destination.
pub struct HomeComponent {
    continue_items: Vec<QueueItem>,
    latest: Vec<(String, HomeLatestSource, Vec<QueueItem>)>,
    loading: bool,
    section: usize,
    cursor: usize,
    scroll: usize,
    focused: bool,
    /// Runtime terminal-capability flag (config-derived, not per-render
    /// content); set once by the shell after construction.
    use_nerd_fonts: bool,
    panel_area: Option<Rect>,
    hitmap: Vec<(Rect, usize)>,
    pill_targets: Vec<(Rect, usize)>,
    /// The cover image (if any) `view()` computed but could not paint
    /// itself (no `App`/image-cache authority); the shell takes it via
    /// `take_image_paint` right after `application.view()` returns and
    /// paints it using `App::paint_home_image`.
    image_paint: Option<HomeImagePaint>,
    /// The list area (`render_home_content`'s `left_area`) `view()` painted
    /// the rows into. Rebuilt every `view` like `hitmap`/`pill_targets`; this
    /// is Home's whole claim rect, so a click or wheel anywhere inside it is
    /// reported to the shell as a typed `HomeClick`/`HomeScroll` (the shell
    /// decides `App`'s cursor/focus/activation meaning). The component holds
    /// no double-click or scroll timing state — `App` owns that.
    list_area: Rect,
    /// The selected row's painted rect (`render_home_content`'s
    /// `selected_item_rect`), retained for the shell to anchor the Home
    /// context menu against what the component actually painted rather than
    /// the legacy `AppLayout` copy (task 5.3d, Home menu-placement geometry).
    /// `None` when this render produced no selection rect, matching the
    /// legacy copy's own optionality.
    selected_item_rect: Option<Rect>,
    /// The hero panel `render_home_content` painted this `view` (its
    /// `hero_area`), retained so the single painter's own geometry is
    /// observable to characterization tests without any layout mirror (task
    /// 5.3d, Home legacy underpaint removal). `None` when this render
    /// painted no hero (too short, or no hero item).
    hero_area: Option<Rect>,
}

impl HomeComponent {
    pub fn new() -> Self {
        Self {
            continue_items: Vec::new(),
            latest: Vec::new(),
            loading: false,
            section: 0,
            cursor: 0,
            scroll: 0,
            focused: false,
            use_nerd_fonts: false,
            panel_area: None,
            hitmap: Vec::new(),
            pill_targets: Vec::new(),
            image_paint: None,
            list_area: Rect::default(),
            selected_item_rect: None,
            hero_area: None,
        }
    }

    /// Replace the shell-owned content snapshot. Section/cursor clamp to
    /// the new content (this is the async section clamp; the component is
    /// the sole owner of the numeric section).
    pub(in crate::app) fn set_content(
        &mut self,
        continue_items: Vec<QueueItem>,
        latest: Vec<(String, HomeLatestSource, Vec<QueueItem>)>,
        loading: bool,
    ) {
        self.continue_items = continue_items;
        self.latest = latest;
        self.loading = loading;
        self.clamp_section_and_cursor();
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    pub(in crate::app) fn set_use_nerd_fonts(&mut self, use_nerd_fonts: bool) {
        self.use_nerd_fonts = use_nerd_fonts;
    }

    /// Takes the cover image (if any) `view()` computed but could not
    /// paint itself. The shell calls this right after `application.view()`
    /// returns and paints it via `App::paint_home_image`.
    pub(in crate::app) fn take_image_paint(
        &mut self,
    ) -> Option<crate::app::render::HomeImagePaint> {
        self.image_paint.take()
    }

    /// Restore a persisted pill selection once a section matching `source`
    /// exists, mirroring the `home_section_pending` restore the shell applies
    /// on `push_home_content`. Returns `true` once restored (the shell clears the
    /// pending marker afterward).
    pub(in crate::app) fn restore_section(&mut self, source: &HomeLatestSource) -> bool {
        if let Some(idx) = self.latest.iter().position(|(_, s, _)| s == source) {
            self.section = idx + 1;
            self.clamp_section_and_cursor();
            true
        } else {
            false
        }
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn section(&self) -> usize {
        self.section
    }

    /// The semantic `HomeLatestSource` of a numeric section index: `None` for
    /// Continue Watching (section 0, the empty-string persistence sentinel),
    /// otherwise the selected latest section's source. Resolving by section
    /// here keeps the off-by-one rule in the component (the sole numeric
    /// section owner); the shell persists this identity, never the index
    /// (task 5.3d).
    pub(in crate::app) fn source_for_section(&self, section: usize) -> Option<HomeLatestSource> {
        if section == 0 {
            return None;
        }
        self.latest
            .get(section - 1)
            .map(|(_, source, _)| source.clone())
    }

    /// Home's whole painted panel rect (`list_area`) and its selected-row
    /// rect, for the shell to place the context menu over what this component
    /// actually painted rather than the legacy `AppLayout` copies (task 5.3d,
    /// Home menu-placement geometry). `selected_item_rect` is `None` when this
    /// render produced no selection rect.
    pub(in crate::app) fn menu_placement_geometry(&self) -> (Rect, Option<Rect>) {
        (self.list_area, self.selected_item_rect)
    }

    /// The hero panel `view()` painted this render (the single painter's own
    /// geometry, for characterization), `None` when it painted none. Not a
    /// layout mirror — the component owns every Home `view`-painted rect.
    pub(in crate::app) fn hero_area(&self) -> Option<Rect> {
        self.hero_area
    }

    #[cfg(test)]
    pub(crate) fn test_pill_targets(&self) -> &[(Rect, usize)] {
        &self.pill_targets
    }

    fn new_sections(&self) -> Vec<usize> {
        (0..self.latest.len()).map(|idx| idx + 1).collect()
    }

    fn section_is_valid(&self, section_idx: usize) -> bool {
        section_idx == 0 || self.new_sections().contains(&section_idx)
    }

    fn section_range(&self, section_idx: usize) -> Option<(usize, usize)> {
        if section_idx == 0 {
            return Some((0, self.continue_items.len()));
        }
        let mut pos = self.continue_items.len();
        for (idx, (_, _, items)) in self.latest.iter().enumerate() {
            if idx + 1 == section_idx {
                return Some((pos, items.len()));
            }
            pos += items.len();
        }
        None
    }

    fn visible_indices(&self) -> Vec<usize> {
        let selected = if self.section_is_valid(self.section) {
            self.section
        } else {
            self.new_sections().first().copied().unwrap_or(0)
        };
        self.section_range(selected)
            .map(|(start, len)| (start..start + len).collect())
            .unwrap_or_default()
    }

    fn clamp_section_and_cursor(&mut self) {
        if !self.section_is_valid(self.section) {
            self.section = self.new_sections().first().copied().unwrap_or(0);
        }
        let indices = self.visible_indices();
        if let Some(first) = indices.first() {
            if !indices.contains(&self.cursor) {
                self.cursor = *first;
            }
        } else {
            self.cursor = 0;
        }
    }

    /// Move the flat cursor within the currently visible section (clamped to
    /// its bounds), matching `ui_util::move_cursor`. This is the section-local
    /// cursor movement the keyboard navigation and the Model-boundary wheel
    /// scroll both use (task 5.3d, Home wheel-scroll ownership); the shell
    /// calls it with the same delta semantics as keyboard Up/Down.
    pub(in crate::app) fn move_local_cursor(&mut self, delta: i64) {
        let indices = self.visible_indices();
        if indices.is_empty() {
            self.cursor = 0;
            return;
        }
        let pos = indices
            .iter()
            .position(|idx| *idx == self.cursor)
            .unwrap_or(0);
        let next = move_cursor(pos, delta, indices.len());
        self.cursor = indices[next];
    }

    fn select_start(&mut self) {
        if let Some(first) = self.visible_indices().first() {
            self.cursor = *first;
        }
    }

    fn select_end(&mut self) {
        if let Some(last) = self.visible_indices().last() {
            self.cursor = *last;
        }
    }

    /// Select `section_idx` (clamped to the nearest valid section). Returns
    /// `true` when the selection actually changed, so the caller emits the
    /// persist `Msg` only on a real change.
    fn select_section(&mut self, section_idx: usize) -> bool {
        let resolved = if self.section_is_valid(section_idx) {
            section_idx
        } else if let Some(first) = self.new_sections().first() {
            *first
        } else {
            self.section = 0;
            return false;
        };
        if resolved == self.section {
            return false;
        }
        self.section = resolved;
        self.scroll = 0;
        if let Some((start, len)) = self.section_range(resolved) {
            self.cursor = if len == 0 {
                start
            } else {
                self.cursor.clamp(start, start + len - 1)
            };
        }
        true
    }

    fn move_section(&mut self, dir: i64) -> bool {
        let mut sections = vec![0];
        sections.extend(self.new_sections());
        let pos = sections.iter().position(|&s| s == self.section);
        let next_pos = match pos {
            Some(p) => {
                let n = sections.len() as i64;
                (((p as i64 + dir) % n + n) % n) as usize
            }
            None => 0,
        };
        self.select_section(sections[next_pos])
    }

    /// Handle a keyboard event using crossterm types directly. The local
    /// navigation set and the typed effect-key family (Enter, Ctrl+Enter,
    /// Ctrl+A, Ctrl+W, Delete) are Home's to claim. `None` means the key
    /// isn't Home's to handle: the caller falls through to the legacy
    /// top-level dispatch (`App::handle_key`'s `CONTEXT_STACK`) unchanged,
    /// exactly as if Home had never converted (design D11/D13). Global keys
    /// the `CONTEXT_STACK` already claims ahead of Home (`q`, Tab/BackTab,
    /// 1-9, `.`) are deliberately *not* matched here for that reason: `.`
    /// is consumed by `handle_global_view_key` before the browse dispatch (and
    /// the deleted `handle_cw_key`) ever run, so the global context-menu
    /// routing stays at its original precedence — this component reproduces
    /// the reachable set, not the unreachable one.
    ///
    /// The component claims Home's keys only while its Library panel is
    /// focused (`self.focused`, mirrored from the shell's effective panel
    /// focus every tick). With the Queue panel focused, every key returns
    /// `None` and falls through to the legacy dispatch, so queue handling
    /// (`handle_queue_key`) sees it instead of Home mutating local state
    /// that has no focus authority.
    pub(in crate::app) fn handle_crossterm_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<Msg> {
        if !self.focused {
            return None;
        }
        let ctrl = key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL);
        match key.code {
            crossterm::event::KeyCode::Up => {
                self.move_local_cursor(-1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            crossterm::event::KeyCode::Down => {
                self.move_local_cursor(1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            crossterm::event::KeyCode::Char('[') if !ctrl => {
                let changed = self.move_section(-1);
                Some(self.section_msg(changed))
            }
            crossterm::event::KeyCode::Char(']') if !ctrl => {
                let changed = self.move_section(1);
                Some(self.section_msg(changed))
            }
            crossterm::event::KeyCode::PageUp => {
                self.move_local_cursor(-(self.page_size() as i64));
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            crossterm::event::KeyCode::PageDown => {
                self.move_local_cursor(self.page_size() as i64);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            crossterm::event::KeyCode::Home => {
                self.select_start();
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            crossterm::event::KeyCode::End => {
                self.select_end();
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            crossterm::event::KeyCode::Enter if ctrl => {
                Some(Msg::Shell(ShellRequest::HomeEnqueue(self.cursor)))
            }
            crossterm::event::KeyCode::Enter => {
                Some(Msg::Shell(ShellRequest::HomePlay(self.cursor)))
            }
            crossterm::event::KeyCode::Char('a') if ctrl => {
                Some(Msg::Shell(ShellRequest::HomeEnqueue(self.cursor)))
            }
            crossterm::event::KeyCode::Char('w') if ctrl => {
                Some(Msg::Shell(ShellRequest::HomeToggleWatched))
            }
            crossterm::event::KeyCode::Delete => {
                Some(Msg::Shell(ShellRequest::HomeDelete(self.cursor)))
            }
            _ => None,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let crossterm_key = to_crossterm_key_event(key);
        self.handle_crossterm_key(crossterm_key)
            .or(Some(Msg::Legacy(LegacyTerminalEvent::Key(crossterm_key))))
    }

    fn section_msg(&self, changed: bool) -> Msg {
        if changed {
            Msg::Shell(ShellRequest::HomeSectionSelected(self.section))
        } else {
            Msg::Legacy(LegacyTerminalEvent::NoOp)
        }
    }

    fn page_size(&self) -> usize {
        self.panel_area
            .map(|a| a.height as usize)
            .unwrap_or(1)
            .max(1)
    }

    /// Handle a mouse event using crossterm types directly. `None` means
    /// the event isn't Home's to handle (outside Home's own painted
    /// geometry — tab bar, queue panel, playback controls, the hero in
    /// two-column layout, ...); the caller falls through to the legacy
    /// mouse dispatch unchanged.
    ///
    /// The component owns *where* a Home click lands: it hit-tests against
    /// its own painted geometry (`list_area`, `hitmap`, `pill_targets`,
    /// rebuilt every `view`) and emits a typed `Msg::Shell` naming the region.
    /// It holds no double-click or scroll timing — the shell decides *when* a
    /// click counts against `App`'s own timing fields. Wheel scroll over the
    /// list area is claimed as `HomeScroll`: the shell moves the component's
    /// local cursor and, as a preserved pre-existing quirk, the independent
    /// Continue Watching column's cursor (`Model::handle_home_scroll` →
    /// `App::cw_move_cursor`), which this migration preserves rather than
    /// fixes (task 5.3d, Home wheel-scroll ownership).
    pub(in crate::app) fn handle_crossterm_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
    ) -> Option<Msg> {
        let pos: ratatui::layout::Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            crossterm::event::MouseEventKind::ScrollDown
            | crossterm::event::MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, crossterm::event::MouseEventKind::ScrollUp)
                {
                    -1
                } else {
                    1
                };
                if self.list_area.contains(pos) {
                    return Some(Msg::Shell(ShellRequest::HomeScroll { delta }));
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                // Section pills sit above the list area; claim them before
                // the row hit-test. `select_section` keeps the component's
                // own render state (section/cursor) authoritative; the shell
                // arm persists the selected source at the Model boundary.
                if let Some(section_idx) = self
                    .pill_targets
                    .iter()
                    .find(|(rect, _)| rect.contains(pos))
                    .map(|(_, section_idx)| *section_idx)
                {
                    self.select_section(section_idx);
                    return Some(Msg::Shell(ShellRequest::HomeClick {
                        region: HomeHitRegion::Pill(section_idx),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
                if self.list_area.contains(pos) {
                    // Local highlight only: the authoritative cursor set
                    // (row-map resolution, panel focus) stays in
                    // The `HomeClick` shell arm consumes the resolved cursor.
                    if let Some((_, flat_idx)) =
                        self.hitmap.iter().find(|(rect, _)| rect.contains(pos))
                    {
                        self.cursor = *flat_idx;
                    }
                    return Some(Msg::Shell(ShellRequest::HomeClick {
                        region: HomeHitRegion::Row(self.cursor),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                if self.list_area.contains(pos) {
                    // Resolve the row under the click before opening the menu;
                    // a blank/gap click leaves the cursor unchanged (no hitmap
                    // rect matches), so the context menu opens at the current
                    // cursor. Matching the left-click/`BrowserComponent` row
                    // treatment, resolving a row also moves the component-local
                    // cursor so the emitted `ContextMenu` and the cursor agree
                    // on the row under the click.
                    if let Some((_, flat_idx)) =
                        self.hitmap.iter().find(|(rect, _)| rect.contains(pos))
                    {
                        self.cursor = *flat_idx;
                    }
                    return Some(Msg::Shell(ShellRequest::HomeClick {
                        region: HomeHitRegion::ContextMenu(self.cursor),
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            _ => {}
        }
        None
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let crossterm_mouse = to_crossterm_mouse_event(mouse);
        self.handle_crossterm_mouse(crossterm_mouse)
            .or(Some(Msg::Legacy(LegacyTerminalEvent::Mouse(
                crossterm_mouse,
            ))))
    }

    #[cfg(test)]
    pub(crate) fn test_hitmap(&self) -> &[(Rect, usize)] {
        &self.hitmap
    }
}

impl Default for HomeComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for HomeComponent {
    fn view(&mut self, f: &mut Frame, area: Rect) {
        let result = crate::app::render::render_home_content(
            f,
            area,
            self.focused,
            &self.continue_items,
            &self.latest,
            self.section,
            &mut self.cursor,
            &mut self.scroll,
            self.use_nerd_fonts,
        );
        self.section = result.resolved_section;
        self.hitmap = result.hitmap;
        self.pill_targets = result.pill_targets;
        self.list_area = result.left_area;
        self.selected_item_rect = result.selected_item_rect;
        self.image_paint = result.image_paint;
        self.hero_area = result.hero_area;
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

impl AppComponent<Msg, UserEvent> for HomeComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }
}
