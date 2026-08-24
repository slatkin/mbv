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
//! `ShellRequest::Home*` messages below are implemented and tested but not
//! yet the live input path; they are the intended hand-off target for the
//! follow-up task that gives Home real input ownership.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyEvent, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
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
    last_click_time: Option<std::time::Instant>,
    last_click_pos: (u16, u16),
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
            last_click_time: None,
            last_click_pos: (0, 0),
        }
    }

    /// Replace the shell-owned content snapshot. Section/cursor clamp to
    /// the new content, matching `home_select_section`'s bounds handling.
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
    /// exists, mirroring the legacy `home_section_pending` restore in
    /// `render_home_section_pills_row`. Returns `true` once restored (the
    /// shell should stop calling this for the same preference afterward).
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

    fn move_local_cursor(&mut self, delta: i64) {
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

    /// Select `section_idx` (clamped to the nearest valid section, matching
    /// `home_select_section`). Returns `true` when the selection actually
    /// changed, so the caller emits the persist `Msg` only on a real change.
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

    /// Handle a keyboard event using crossterm types directly, matching the
    /// legacy `handle_cw_key`'s key set exactly. `None` means the key isn't
    /// Home's to handle: the caller falls through to the legacy top-level
    /// dispatch (`App::handle_key`'s `CONTEXT_STACK`) unchanged, exactly as
    /// if Home had never converted (design D11/D13). Global keys the
    /// `CONTEXT_STACK` already claims ahead of Home (`q`, Tab/BackTab,
    /// 1-9, `.`) are deliberately *not* matched here for that reason: `.`
    /// in particular is already consumed by `handle_global_view_key` before
    /// `handle_cw_key` ever runs, so its own `.` arm there is unreachable —
    /// this component reproduces the reachable set, not the unreachable one.
    /// Used both by `AppComponent::on` (kept for the mount trait bound) and
    /// by the shell's manual dispatch, which never makes Home the active
    /// TuiRealm component (see `Model::route_home_key`'s doc comment).
    pub(in crate::app) fn handle_crossterm_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<Msg> {
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
    /// the event isn't Home's to handle (outside its own painted geometry,
    /// or a wheel scroll — see the wheel-scroll doc note below); the caller
    /// falls through to the legacy mouse dispatch unchanged.
    pub(in crate::app) fn handle_crossterm_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
    ) -> Option<Msg> {
        let pos: ratatui::layout::Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                let pill_hit = self
                    .pill_targets
                    .iter()
                    .find(|(rect, _)| rect.contains(pos))
                    .map(|(_, section_idx)| *section_idx);
                if let Some(section_idx) = pill_hit {
                    let changed = self.select_section(section_idx);
                    return Some(self.section_msg(changed));
                }
                let row_hit = self
                    .hitmap
                    .iter()
                    .find(|(rect, _)| rect.contains(pos))
                    .map(|(_, flat_idx)| *flat_idx);
                let Some(flat_idx) = row_hit else {
                    // Outside Home's own geometry (tab bar, queue panel,
                    // playback controls, ...): not Home's event.
                    return None;
                };
                let now = std::time::Instant::now();
                let is_double = self
                    .last_click_time
                    .is_some_and(|t| now.duration_since(t) < std::time::Duration::from_millis(400))
                    && self.last_click_pos == (mouse.column, mouse.row);
                self.last_click_time = Some(now);
                self.last_click_pos = (mouse.column, mouse.row);
                self.cursor = flat_idx;
                if is_double {
                    Some(Msg::Shell(ShellRequest::HomePlay(self.cursor)))
                } else {
                    Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
                }
            }
            // Wheel scroll on Home does not move the flat cursor in the
            // legacy path either (it moves the independent Continue
            // Watching column's cursor instead, a pre-existing quirk this
            // migration preserves rather than fixes): not Home's event.
            _ => None,
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let crossterm_mouse = to_crossterm_mouse_event(mouse);
        self.handle_crossterm_mouse(crossterm_mouse)
            .or(Some(Msg::Legacy(LegacyTerminalEvent::Mouse(
                crossterm_mouse,
            ))))
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
        self.image_paint = result.image_paint;
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
